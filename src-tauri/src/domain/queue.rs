//! Queue read model and the daily-queue rules (specs/queue-slice/design.md):
//! per track, stale items first, then the fresh frontier, then the next new
//! chunk; tracks merged by stable rotation. Pure — `infrastructure` supplies
//! the rows through [`QueueRepo`], `application` composes the pieces.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use chrono::{DateTime, Duration, Utc};

use super::chunk::ChunkStatus;
use super::repo::RepoError;
use super::settings::TRACKS;

/// An `in_progress` chunk untouched for at least this long is stale
/// (requirements.md EARS: 14+ days → surfaced ahead of the track's new chunks).
pub const STALE_AFTER_DAYS: i64 = 14;

/// The queue-facing read model of one chunk: what the rules need to pick the
/// daily answer. Rows come from the state store (`chunks` × `sources`,
/// migration 0001) — never from the vault, whose truth was indexed already.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueChunk {
    /// Stable row id — sessions and completion reference it.
    pub chunk_id: i64,
    /// Track the queue rotates over (fundamentos → system-design → videos).
    pub track: String,
    /// Position within the track's course order (next-in-order rule).
    pub ord: i32,
    pub title: String,
    /// Vault-relative stable key (ADR 0003) — position overrides point at it.
    pub vault_path: String,
    pub status: ChunkStatus,
    pub est_minutes: Option<i32>,
    /// Last status change — drives the 14-day stale rule.
    pub updated_at: DateTime<Utc>,
}

/// One surfaced item: the chunk plus the flag the queue computed for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub chunk: QueueChunk,
    /// `in_progress` untouched for ≥ [`STALE_AFTER_DAYS`] — surfaced ahead of
    /// the track's new chunks (requirements.md EARS).
    pub stale: bool,
}

/// A track's section of the daily queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackQueue {
    pub track: String,
    /// Surfacing order: stale first (oldest activity first), then the fresh
    /// frontier, then the next new chunk (design.md).
    pub items: Vec<QueueItem>,
}

/// The daily queue: one screen answering "what do I study now". The due
/// header counts the app's own review backlog (kept cards due ≤ now,
/// migration 0002) — AnkiConnect left the queue path in slice 0.2 (ADR 0004:
/// Anki is never required at runtime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyQueue {
    /// Kept cards with `due <= now` — the memoria backlog.
    pub due: u64,
    /// Track sections in stable rotation order; tracks with nothing to show
    /// are omitted.
    pub tracks: Vec<TrackQueue>,
}

/// Port: the queue's read model over the state store (ADR 0003). Rows come
/// back as stored; the rules here do the picking.
pub trait QueueRepo {
    fn open_chunks(&self) -> impl Future<Output = Result<Vec<QueueChunk>, RepoError>> + Send;
}

/// A chunk is stale when `in_progress` and untouched for ≥ 14 days
/// (requirements.md EARS: "in-progress chunk untouched for 14+ days").
pub fn is_stale(chunk: &QueueChunk, now: DateTime<Utc>) -> bool {
    chunk.status == ChunkStatus::InProgress
        && now - chunk.updated_at >= Duration::days(STALE_AFTER_DAYS)
}

/// Stable rotation order (design.md: fundamentos → system-design → video, no
/// track starves): the canonical v1 tracks first, then any other indexed
/// tracks alphabetically.
pub fn rotation_order(tracks: impl IntoIterator<Item = String>) -> Vec<String> {
    let seen: BTreeSet<String> = tracks.into_iter().collect();
    let mut order: Vec<String> = TRACKS
        .into_iter()
        .filter(|t| seen.contains(*t))
        .map(|t| t.to_string())
        .collect();
    order.extend(seen.into_iter().filter(|t| !TRACKS.contains(&t.as_str())));
    order
}

/// The track's items, in surfacing order (design.md):
///
/// 1. stale `in_progress` chunks first, most neglected first;
/// 2. the freshest non-stale `in_progress` chunk — the active frontier, so a
///    started-but-unfinished chunk never vanishes from the answer;
/// 3. the first `queued` chunk at/after the track's position override — the
///    next new work ("first chunk with status `queued` = next").
///
/// The override is the chunk the track is at (design.md chunk identity):
/// everything strictly before it is past even when its status says otherwise.
/// An override pointing nowhere is ignored — vaults move.
pub fn track_items(
    candidates: &[QueueChunk],
    override_path: Option<&str>,
    now: DateTime<Utc>,
) -> Vec<QueueItem> {
    let floor = override_path
        .and_then(|path| candidates.iter().find(|c| c.vault_path == path))
        .map(|c| c.ord);
    let eligible: Vec<&QueueChunk> = candidates
        .iter()
        .filter(|c| floor.is_none_or(|min| c.ord >= min))
        .collect();

    let mut stale: Vec<QueueItem> = eligible
        .iter()
        .filter(|c| is_stale(c, now))
        .map(|c| QueueItem {
            chunk: (*c).clone(),
            stale: true,
        })
        .collect();
    stale.sort_by_key(|item| item.chunk.updated_at);

    let current = eligible
        .iter()
        .filter(|c| c.status == ChunkStatus::InProgress && !is_stale(c, now))
        .max_by_key(|c| c.updated_at)
        .map(|c| QueueItem {
            chunk: (*c).clone(),
            stale: false,
        });

    let next = eligible
        .iter()
        .filter(|c| c.status == ChunkStatus::Queued)
        .min_by_key(|c| (c.ord, c.chunk_id))
        .map(|c| QueueItem {
            chunk: (*c).clone(),
            stale: false,
        });

    let mut items = stale;
    items.extend(current);
    items.extend(next);
    items
}

/// Build the daily queue: pick per-track items, then merge the sections in
/// stable rotation order. `position_overrides` carries the raw
/// `track_position:<track>` settings values (chunk `vault_path`s); `due` is
/// the internal memoria backlog (kept cards due ≤ now).
pub fn build_queue(
    chunks: Vec<QueueChunk>,
    position_overrides: &BTreeMap<String, String>,
    due: u64,
    now: DateTime<Utc>,
) -> DailyQueue {
    let mut by_track: BTreeMap<String, Vec<QueueChunk>> = BTreeMap::new();
    for chunk in chunks {
        by_track.entry(chunk.track.clone()).or_default().push(chunk);
    }
    let mut tracks = Vec::new();
    for track in rotation_order(by_track.keys().cloned()) {
        let candidates = by_track.remove(&track).unwrap_or_default();
        let items = track_items(
            &candidates,
            position_overrides.get(&track).map(String::as_str),
            now,
        );
        if !items.is_empty() {
            tracks.push(TrackQueue { track, items });
        }
    }
    DailyQueue { due, tracks }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(track: &str, ord: i32, status: ChunkStatus, days_since_update: i64) -> QueueChunk {
        QueueChunk {
            chunk_id: ord as i64,
            track: track.to_string(),
            ord,
            title: format!("{track} {ord:02}"),
            vault_path: format!("courses/{track}/item-{ord:02}"),
            status,
            est_minutes: None,
            updated_at: Utc::now() - Duration::days(days_since_update),
        }
    }

    #[test]
    fn stale_is_in_progress_untouched_for_fourteen_days() {
        let now = Utc::now();
        let mut c = chunk("t", 1, ChunkStatus::InProgress, 13);
        assert!(!is_stale(&c, now), "13 days is not yet stale");
        c.updated_at = now - Duration::days(14);
        assert!(is_stale(&c, now), "exactly 14 days is stale");
        c.status = ChunkStatus::Queued;
        assert!(!is_stale(&c, now), "queued chunks are never stale");
        c.status = ChunkStatus::Done;
        assert!(!is_stale(&c, now), "done chunks are never stale");
    }

    #[test]
    fn rotation_is_canonical_tracks_first_then_extras_alphabetically() {
        assert_eq!(rotation_order(vec![]), Vec::<String>::new());
        assert_eq!(
            rotation_order([
                "videos".into(),
                "system-design".into(),
                "aardvark".into(),
                "fundamentos-enterprise".into()
            ]),
            vec![
                "fundamentos-enterprise",
                "system-design",
                "videos",
                "aardvark"
            ]
        );
        assert_eq!(
            rotation_order(["videos".into(), "videos".into()]),
            vec!["videos"],
            "duplicates collapse"
        );
    }

    #[test]
    fn track_items_stale_first_then_next_queued() {
        let now = Utc::now();
        let candidates = vec![
            chunk("fundamentos-enterprise", 1, ChunkStatus::Done, 0),
            chunk("fundamentos-enterprise", 2, ChunkStatus::InProgress, 20),
            chunk("fundamentos-enterprise", 3, ChunkStatus::Queued, 0),
            chunk("fundamentos-enterprise", 4, ChunkStatus::Queued, 0),
        ];
        let items = track_items(&candidates, None, now);
        let ords: Vec<i32> = items.iter().map(|i| i.chunk.ord).collect();
        assert_eq!(
            ords,
            vec![2, 3],
            "stale in_progress first, then first queued"
        );
        assert!(items[0].stale);
        assert!(!items[1].stale);
    }

    #[test]
    fn track_items_fresh_frontier_is_surfaced_so_work_never_vanishes() {
        let now = Utc::now();
        let candidates = vec![
            chunk("system-design", 1, ChunkStatus::InProgress, 1),
            chunk("system-design", 2, ChunkStatus::Queued, 0),
        ];
        let items = track_items(&candidates, None, now);
        let ords: Vec<i32> = items.iter().map(|i| i.chunk.ord).collect();
        assert_eq!(
            ords,
            vec![1, 2],
            "fresh in_progress resumes before the next queued"
        );
        assert!(items.iter().all(|i| !i.stale));
    }

    #[test]
    fn position_override_skips_everything_before_it() {
        let now = Utc::now();
        let candidates = vec![
            chunk("videos", 1, ChunkStatus::Queued, 0),
            chunk("videos", 2, ChunkStatus::InProgress, 30),
            chunk("videos", 3, ChunkStatus::Queued, 0),
        ];
        // "I am at item-03": item-01 stays queued but is past; the stale
        // item-02 is before the position, so it does not resurface either.
        let items = track_items(&candidates, Some("courses/videos/item-03"), now);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].chunk.vault_path, "courses/videos/item-03");

        // An override pointing nowhere is ignored — vaults move. The stale
        // chunk resurfaces and the next queued chunk follows it.
        let items = track_items(&candidates, Some("courses/videos/gone"), now);
        let paths: Vec<&str> = items.iter().map(|i| i.chunk.vault_path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["courses/videos/item-02", "courses/videos/item-01"]
        );
    }

    #[test]
    fn build_queue_rotates_sections_and_omits_empty_tracks() {
        let now = Utc::now();
        let chunks = vec![
            chunk("fundamentos-enterprise", 1, ChunkStatus::Queued, 0),
            chunk("videos", 1, ChunkStatus::Queued, 0),
            chunk("philosophy", 1, ChunkStatus::Done, 0),
            chunk("philosophy", 2, ChunkStatus::Queued, 0),
        ];
        let overrides = BTreeMap::new();
        let queue = build_queue(chunks, &overrides, 7, now);

        assert_eq!(queue.due, 7);
        let section: Vec<&str> = queue.tracks.iter().map(|t| t.track.as_str()).collect();
        assert_eq!(
            section,
            vec!["fundamentos-enterprise", "videos", "philosophy"],
            "canonical rotation first, extras after; the done-only track is omitted"
        );
    }
}
