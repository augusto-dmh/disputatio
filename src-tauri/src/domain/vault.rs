//! `VaultReader` port: reads the Obsidian vault into a pure scan
//! (specs/queue-slice/design.md). Implementations own all IO; the domain
//! stays pure (architecture.md layering rule 1).

use std::fmt;
use std::path::Path;

use super::chunk::Chunk;
use super::source::Source;

/// One scanned track: a source plus its course-ordered chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedSource {
    pub source: Source,
    pub chunks: Vec<Chunk>,
}

/// The whole vault, scanned. Pure data — no filesystem handles.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VaultScan {
    pub sources: Vec<ScannedSource>,
}

impl VaultScan {
    pub fn chunk_count(&self) -> usize {
        self.sources.iter().map(|s| s.chunks.len()).sum()
    }

    /// Chunks the seeding rules derived as already studied.
    pub fn done_count(&self) -> usize {
        self.sources
            .iter()
            .flat_map(|s| s.chunks.iter())
            .filter(|c| c.status == super::chunk::ChunkStatus::Done)
            .count()
    }
}

#[derive(Debug)]
pub enum VaultError {
    /// The configured vault root does not exist.
    NotFound(String),
    Io(std::io::Error),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VaultError::NotFound(path) => write!(f, "vault path not found: {path}"),
            VaultError::Io(e) => write!(f, "vault read failed: {e}"),
        }
    }
}

impl std::error::Error for VaultError {}

/// Port over the vault filesystem. Sync on purpose: walking a local vault is
/// quick bounded IO, and the application service awaits the repo, not the FS.
pub trait VaultReader: Send + Sync {
    fn scan(&self, vault_root: &Path) -> Result<VaultScan, VaultError>;
}
