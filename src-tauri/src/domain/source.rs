//! Source entity: a track-level study source in the vault
//! (docs/architecture.md data model sketch, migration 0001 `sources`).

/// The kind of a source, mirroring the `sources.kind` check constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Course,
    Video,
    AnkiImport,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Course => "course",
            SourceKind::Video => "video",
            SourceKind::AnkiImport => "anki_import",
        }
    }
}

/// One study source (a course track or the video folder). `vault_path` is the
/// stable identity re-index upserts on (specs/queue-slice/design.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: SourceKind,
    /// Track name the queue rotates over (design.md: fundamentos → system-design → video).
    pub track: String,
    pub title: String,
    /// Vault-relative, forward-slash path — stable across machines (ADR 0003).
    pub vault_path: String,
    pub ord: i32,
}
