//! Daily queue service (specs/queue-slice/design.md: `queue.rs` —
//! `build_daily_queue`): compose the queue read model, the position
//! overrides and the Anki due total into one answer. Pure composition —
//! the adapters own the world.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};

use crate::domain::anki::{AnkiError, AnkiGateway};
use crate::domain::queue::{AnkiStatus, DailyQueue, QueueChunk, QueueRepo};
use crate::domain::repo::RepoError;
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
/// over the repo's read model, with the Anki due total in the header. Anki
/// is display-only — a failure degrades to a header note, never an error
/// (specs/queue-slice/design.md point 3, requirements.md EARS).
pub async fn build_daily_queue<R, S, A>(
    repo: &R,
    store: &S,
    anki: &A,
    now: DateTime<Utc>,
) -> Result<DailyQueue, QueueError>
where
    R: QueueRepo,
    S: SettingsStore,
    A: AnkiGateway,
{
    let chunks: Vec<QueueChunk> = repo.open_chunks().await.map_err(QueueError::Repo)?;
    let mut overrides = BTreeMap::new();
    for (key, value) in store.load_all().await.map_err(QueueError::Settings)? {
        if let Some(track) = track_from_key(&key) {
            overrides.insert(track.to_string(), value);
        }
    }
    let anki_status = match anki.due_total().await {
        Ok(total) => AnkiStatus::Due(total),
        Err(AnkiError::Offline) => AnkiStatus::Offline,
        Err(AnkiError::BadResponse(detail)) => AnkiStatus::Unavailable(detail),
    };
    Ok(crate::domain::queue::build_queue(
        chunks,
        &overrides,
        anki_status,
        now,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::chunk::ChunkStatus;

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

    struct FakeAnki(Result<u64, AnkiError>);

    impl AnkiGateway for FakeAnki {
        async fn due_total(&self) -> Result<u64, AnkiError> {
            self.0.clone()
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
    async fn anki_outcomes_degrade_to_header_states_never_errors() {
        let repo = FakeRepo(vec![]);
        let store = FakeStore(vec![]);

        let queue = build_daily_queue(&repo, &store, &FakeAnki(Ok(12)), Utc::now())
            .await
            .expect("anki ok");
        assert_eq!(queue.anki, AnkiStatus::Due(12));

        let queue = build_daily_queue(
            &repo,
            &store,
            &FakeAnki(Err(AnkiError::Offline)),
            Utc::now(),
        )
        .await
        .expect("anki offline degrades");
        assert_eq!(queue.anki, AnkiStatus::Offline);

        let queue = build_daily_queue(
            &repo,
            &store,
            &FakeAnki(Err(AnkiError::BadResponse("weird payload".into()))),
            Utc::now(),
        )
        .await
        .expect("anki bad response degrades");
        assert_eq!(queue.anki, AnkiStatus::Unavailable("weird payload".into()));
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

        let queue = build_daily_queue(&repo, &store, &FakeAnki(Ok(0)), Utc::now())
            .await
            .expect("queue builds");
        let items = &queue.tracks[0].items;
        assert_eq!(items.len(), 1, "everything before the override is past");
        assert_eq!(items[0].chunk.vault_path, "courses/videos/item-02");
    }

    #[tokio::test]
    async fn repo_and_settings_failures_surface_honestly() {
        let store = FakeStore(vec![]);
        let anki = FakeAnki(Ok(0));

        let err = build_daily_queue(&FailingRepo, &store, &anki, Utc::now())
            .await
            .expect_err("repo failure surfaces");
        assert!(matches!(err, QueueError::Repo(_)));

        let repo = FakeRepo(vec![]);
        let err = build_daily_queue(&repo, &FailingStore, &anki, Utc::now())
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

        let queue = build_daily_queue(&repo, &store, &FakeAnki(Ok(0)), Utc::now())
            .await
            .expect("queue builds");
        let order: Vec<&str> = queue.tracks.iter().map(|t| t.track.as_str()).collect();
        assert_eq!(
            order,
            vec!["fundamentos-enterprise", "system-design", "videos"]
        );
    }
}
