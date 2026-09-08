//! Daily queue service (specs/queue-slice/design.md: `queue.rs` —
//! `build_daily_queue`): compose the queue read model, the position
//! overrides and the internal due count into one answer. Pure composition —
//! the adapters own the world.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};

use crate::domain::queue::{DailyQueue, QueueChunk, QueueRepo};
use crate::domain::repo::RepoError;
use crate::domain::review::CardRepo;
use crate::domain::settings::{track_from_key, SettingsError, SettingsStore};

#[derive(Debug)]
pub enum QueueError {
    Repo(RepoError),
    Settings(SettingsError),
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueueError::Repo(e) => write!(f, "{e}"),
            QueueError::Settings(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for QueueError {}

/// Build the daily queue: the domain's selection rules (stale first, fresh
/// frontier, next queued at/after the `track_position:<track>` override)
/// over the repo's read model, with the internal memoria backlog (kept cards
/// due ≤ now) in the header. ADR 0004: AnkiConnect left the queue path in
/// slice 0.2 — nothing here reaches outside the state store.
pub async fn build_daily_queue<R, S, C>(
    repo: &R,
    store: &S,
    cards: &C,
    now: DateTime<Utc>,
) -> Result<DailyQueue, QueueError>
where
    R: QueueRepo,
    S: SettingsStore,
    C: CardRepo,
{
    let chunks: Vec<QueueChunk> = repo.open_chunks().await.map_err(QueueError::Repo)?;
    let mut overrides = BTreeMap::new();
    for (key, value) in store.load_all().await.map_err(QueueError::Settings)? {
        if let Some(track) = track_from_key(&key) {
            overrides.insert(track.to_string(), value);
        }
    }
    let due = cards.due_count(now).await.map_err(QueueError::Repo)?;
    Ok(crate::domain::queue::build_queue(
        chunks, &overrides, due, now,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::chunk::ChunkStatus;
    use crate::domain::review::{Card, CardState, ReviewLog};

    struct FakeRepo(Vec<QueueChunk>);

    impl QueueRepo for FakeRepo {
        async fn open_chunks(&self) -> Result<Vec<QueueChunk>, RepoError> {
            Ok(self.0.clone())
        }
    }

    struct FailingRepo;

    impl QueueRepo for FailingRepo {
        async fn open_chunks(&self) -> Result<Vec<QueueChunk>, RepoError> {
            Err(RepoError::Database("connection refused".into()))
        }
    }

    struct FakeStore(Vec<(String, String)>);

    impl SettingsStore for FakeStore {
        async fn load_all(&self) -> Result<Vec<(String, String)>, SettingsError> {
            Ok(self.0.clone())
        }
        async fn put(&self, _key: &str, _value: &str) -> Result<(), SettingsError> {
            unimplemented!("queue reads never write settings")
        }
        async fn delete(&self, _key: &str) -> Result<(), SettingsError> {
            unimplemented!("queue reads never write settings")
        }
    }

    struct FailingStore;

    impl SettingsStore for FailingStore {
        async fn load_all(&self) -> Result<Vec<(String, String)>, SettingsError> {
            Err(SettingsError::Store("boom".into()))
        }
        async fn put(&self, _key: &str, _value: &str) -> Result<(), SettingsError> {
            unimplemented!("queue reads never write settings")
        }
        async fn delete(&self, _key: &str) -> Result<(), SettingsError> {
            unimplemented!("queue reads never write settings")
        }
    }

    /// The cards store double: a fixed due count, so the header contract is
    /// observable without real scheduling.
    struct FakeCards(u64);

    impl CardRepo for FakeCards {
        async fn due_count(&self, _now: DateTime<Utc>) -> Result<u64, RepoError> {
            Ok(self.0)
        }
        async fn due_cards(
            &self,
            _limit: i64,
            _now: DateTime<Utc>,
        ) -> Result<Vec<Card>, RepoError> {
            unimplemented!("the queue only reads the count")
        }
        async fn get(&self, _id: i64) -> Result<Option<Card>, RepoError> {
            unimplemented!("the queue only reads the count")
        }
        async fn apply_review(
            &self,
            _card_id: i64,
            _state: &CardState,
            _log: &ReviewLog,
        ) -> Result<(), RepoError> {
            unimplemented!("the queue only reads the count")
        }
    }

    fn chunk(track: &str, ord: i32, status: ChunkStatus, days_since_update: i64) -> QueueChunk {
        QueueChunk {
            chunk_id: ord as i64,
            track: track.to_string(),
            ord,
            title: format!("{track} {ord:02}"),
            vault_path: format!("courses/{track}/item-{ord:02}"),
            status,
            est_minutes: None,
            updated_at: Utc::now() - chrono::Duration::days(days_since_update),
        }
    }

    #[tokio::test]
    async fn the_header_counts_the_internal_memoria_backlog() {
        let repo = FakeRepo(vec![]);
        let store = FakeStore(vec![]);

        let queue = build_daily_queue(&repo, &store, &FakeCards(12), Utc::now())
            .await
            .expect("queue builds");
        assert_eq!(queue.due, 12);

        let queue = build_daily_queue(&repo, &store, &FakeCards(0), Utc::now())
            .await
            .expect("empty backlog is a normal state, not an error");
        assert_eq!(queue.due, 0);
    }

    #[tokio::test]
    async fn position_overrides_reach_the_selection_rules() {
        let repo = FakeRepo(vec![
            chunk("videos", 1, ChunkStatus::Queued, 0),
            chunk("videos", 2, ChunkStatus::Queued, 0),
        ]);
        let store = FakeStore(vec![(
            "track_position:videos".to_string(),
            "courses/videos/item-02".to_string(),
        )]);

        let queue = build_daily_queue(&repo, &store, &FakeCards(0), Utc::now())
            .await
            .expect("queue builds");
        let items = &queue.tracks[0].items;
        assert_eq!(items.len(), 1, "everything before the override is past");
        assert_eq!(items[0].chunk.vault_path, "courses/videos/item-02");
    }

    #[tokio::test]
    async fn repo_and_settings_failures_surface_honestly() {
        let store = FakeStore(vec![]);
        let cards = FakeCards(0);

        let err = build_daily_queue(&FailingRepo, &store, &cards, Utc::now())
            .await
            .expect_err("repo failure surfaces");
        assert!(matches!(err, QueueError::Repo(_)));

        let repo = FakeRepo(vec![]);
        let err = build_daily_queue(&repo, &FailingStore, &cards, Utc::now())
            .await
            .expect_err("settings failure surfaces");
        assert!(matches!(err, QueueError::Settings(_)));
    }

    #[tokio::test]
    async fn sections_come_back_in_rotation_order() {
        let repo = FakeRepo(vec![
            chunk("videos", 1, ChunkStatus::Queued, 0),
            chunk("system-design", 1, ChunkStatus::Queued, 0),
            chunk("fundamentos-enterprise", 1, ChunkStatus::Queued, 0),
        ]);
        let store = FakeStore(vec![]);

        let queue = build_daily_queue(&repo, &store, &FakeCards(0), Utc::now())
            .await
            .expect("queue builds");
        let order: Vec<&str> = queue.tracks.iter().map(|t| t.track.as_str()).collect();
        assert_eq!(
            order,
            vec!["fundamentos-enterprise", "system-design", "videos"]
        );
    }
}
