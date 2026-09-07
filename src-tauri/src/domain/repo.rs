//! `IndexRepo` port: persistence for scanned vault data (specs/queue-slice/
//! design.md ports list). The Postgres implementation lives in
//! `infrastructure::index_repo`; `application` composes this port only.

use std::fmt;

use async_trait::async_trait;

use super::vault::VaultScan;

/// What one index pass wrote. Counts are per-pass row writes, so two runs of
/// the same vault produce equal reports (idempotency contract, requirements).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexReport {
    pub sources: usize,
    pub chunks: usize,
    /// Chunks whose scan-time status was seeded done (`scope.md` seeding).
    pub done_seeded: usize,
}

#[derive(Debug)]
pub enum RepoError {
    Database(String),
}

impl fmt::Display for RepoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepoError::Database(msg) => write!(f, "repo error: {msg}"),
        }
    }
}

impl std::error::Error for RepoError {}

/// Port over the state store for indexing (ADR 0003).
#[async_trait]
pub trait IndexRepo: Send + Sync {
    /// Upsert one scan idempotently on `vault_path` (the stable key): a
    /// re-index must not duplicate rows and must not regress progress —
    /// chunks already past `queued` keep their status, `queued` rows are
    /// re-derived from the scan (seeding).
    async fn upsert_scan(&self, scan: &VaultScan) -> Result<IndexReport, RepoError>;
}
