//! Indexing orchestration (specs/queue-slice/design.md: `index.rs` —
//! "orchestrate indexing"): read the vault through the `VaultReader` port,
//! persist it through the `IndexRepo` port. Pure composition — the adapters
//! own the world.

use std::fmt;
use std::path::Path;

use crate::domain::repo::{IndexRepo, IndexReport, RepoError};
use crate::domain::vault::{VaultError, VaultReader};

#[derive(Debug)]
pub enum IndexError {
    Vault(VaultError),
    Repo(RepoError),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::Vault(e) => write!(f, "{e}"),
            IndexError::Repo(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for IndexError {}

/// Re-index the vault at `vault_root`: scan + idempotent upsert on
/// `vault_path`. Running it twice over an unchanged vault is a no-op on the
/// row set (requirements.md: "re-index doesn't duplicate").
pub async fn reindex(
    reader: &dyn VaultReader,
    repo: &dyn IndexRepo,
    vault_root: &Path,
) -> Result<IndexReport, IndexError> {
    let scan = reader.scan(vault_root).map_err(IndexError::Vault)?;
    repo.upsert_scan(&scan).await.map_err(IndexError::Repo)
}
