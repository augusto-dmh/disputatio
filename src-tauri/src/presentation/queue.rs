//! Queue command (specs/queue-slice task 5) — translation only
//! (AGENTS.md layering): one service call, small DTOs out. The queue answer
//! is aggregated in Rust; only the picked items cross IPC (payload rule).

use serde::Serialize;
use specta::Type;
use tauri::Manager;

use crate::application::queue::build_daily_queue;
use crate::domain::queue::{AnkiStatus, DailyQueue, QueueItem};
use crate::infrastructure::anki_connect::AnkiConnectGateway;
use crate::infrastructure::pg::Db;
use crate::infrastructure::queue_pg::PgQueueRepo;
use crate::infrastructure::settings_pg::PgSettingsStore;

/// Anki due header. Display-only — the total never gates the queue, and any
/// Anki failure degrades to a note while the queue answers normally
/// (specs/queue-slice/design.md point 3, requirements.md EARS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AnkiHeaderDto {
    /// Total due cards when Anki answered; `null` while offline/unavailable.
    /// (64-bit narrowed at the IPC edge — specta forbids BigInt-style types.)
    pub due_total: Option<u32>,
    /// Human note when Anki did not answer ("Anki offline", or the failure
    /// detail); `null` when `due_total` is present.
    pub note: Option<String>,
}

/// One surfaced queue item. `stale` marks an `in_progress` chunk untouched
/// for 14+ days — surfaced ahead of the track's new chunks (EARS). The DB
/// `BIGINT` id narrows to 32-bit at this edge — ids are tiny in this app
/// (same narrowing as `ReindexResponse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct QueueItemDto {
    pub chunk_id: i32,
    pub track: String,
    pub title: String,
    pub vault_path: String,
    /// `"queued"` or `"in_progress"` (the queue-facing pair).
    pub status: String,
    pub stale: bool,
    pub est_minutes: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct TrackQueueDto {
    pub track: String,
    /// Surfacing order: stale first, then the fresh frontier, then the next
    /// new chunk (specs/queue-slice/design.md).
    pub items: Vec<QueueItemDto>,
}

/// The daily queue: one IPC call answering "what do I study now".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct QueueDto {
    pub anki: AnkiHeaderDto,
    /// Track sections in stable rotation order (fundamentos → system-design
    /// → videos); tracks with nothing to show are omitted.
    pub tracks: Vec<TrackQueueDto>,
}

impl From<AnkiStatus> for AnkiHeaderDto {
    fn from(status: AnkiStatus) -> Self {
        match status {
            AnkiStatus::Due(total) => AnkiHeaderDto {
                due_total: Some(u32::try_from(total).unwrap_or(u32::MAX)),
                note: None,
            },
            AnkiStatus::Offline => AnkiHeaderDto {
                due_total: None,
                note: Some("Anki offline".to_string()),
            },
            AnkiStatus::Unavailable(detail) => AnkiHeaderDto {
                due_total: None,
                note: Some(format!("Anki unavailable: {detail}")),
            },
        }
    }
}

impl From<QueueItem> for QueueItemDto {
    fn from(item: QueueItem) -> Self {
        let chunk = item.chunk;
        QueueItemDto {
            chunk_id: i32::try_from(chunk.chunk_id).unwrap_or(i32::MAX),
            track: chunk.track,
            title: chunk.title,
            vault_path: chunk.vault_path,
            status: chunk.status.as_str().to_string(),
            stale: item.stale,
            est_minutes: chunk.est_minutes,
        }
    }
}

impl From<DailyQueue> for QueueDto {
    fn from(queue: DailyQueue) -> Self {
        QueueDto {
            anki: AnkiHeaderDto::from(queue.anki),
            tracks: queue
                .tracks
                .into_iter()
                .map(|track| TrackQueueDto {
                    track: track.track,
                    items: track.items.into_iter().map(QueueItemDto::from).collect(),
                })
                .collect(),
        }
    }
}

/// The daily queue: per track the next chunk in course order, stale items
/// flagged ahead of new ones, the Anki due total (or the offline note) in
/// the header. Requires the state store — without it the answer cannot be
/// honest, so the error says so (ADR 0003 degradation).
#[tauri::command]
#[specta::specta]
pub async fn get_queue(app: tauri::AppHandle) -> Result<QueueDto, String> {
    let db = app.try_state::<Db>().ok_or_else(|| {
        "database offline — the queue needs the state store (start `make infra`)".to_string()
    })?;
    let queue = build_daily_queue(
        &PgQueueRepo(db.0.clone()),
        &PgSettingsStore(db.0.clone()),
        &AnkiConnectGateway::default(),
        chrono::Utc::now(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(QueueDto::from(queue))
}
