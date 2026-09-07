//! Chunk entity: the unit the queue orders and sessions log
//! (docs/architecture.md: "chunks.status is what the queue reads").

/// Queue-facing chunk states (specs/queue-slice/design.md). The ingest
/// pipeline states (`ingested | narrated | …`) arrive with later slices; the
/// DB check constraint already admits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkStatus {
    Queued,
    InProgress,
    Done,
}

impl ChunkStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ChunkStatus::Queued => "queued",
            ChunkStatus::InProgress => "in_progress",
            ChunkStatus::Done => "done",
        }
    }
}

/// One study unit: a class for markdown tracks, one video for the video
/// track (specs/queue-slice/requirements.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Position within its track's course order (next-in-order rule).
    pub ord: i32,
    pub title: String,
    /// Vault-relative, forward-slash path — the stable key re-index upserts
    /// on (specs/queue-slice/design.md, ADR 0003).
    pub vault_path: String,
    pub est_minutes: Option<i32>,
    /// Derived at scan time by the seeding rules; the repo merges it with
    /// persisted progress on upsert.
    pub status: ChunkStatus,
}
