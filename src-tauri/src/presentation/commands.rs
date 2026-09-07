//! Queue-slice commands. `reindex` per specs/queue-slice/tasks.md item 3.
//!
//! Vault path resolution (kept minimal until task 4 wires the Settings
//! screen): explicit argument wins; otherwise the `DISPUTATIO_VAULT_PATH`
//! environment variable. Task 4 will pass the `app_settings` vault path as
//! the argument from the frontend.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::application::index;
use crate::infrastructure::index_repo::PgIndexRepo;
use crate::infrastructure::pg::Db;
use crate::infrastructure::vault_fs::FsVaultReader;

#[derive(Serialize)]
pub struct ReindexResponse {
    pub sources: u32,
    pub chunks: u32,
    /// Chunks seeded done by the `scope.md` rules in this pass.
    pub done_seeded: u32,
}

/// Resolve the vault root: explicit argument, then `DISPUTATIO_VAULT_PATH`.
fn resolve_vault_root(vault_path: Option<String>) -> Result<std::path::PathBuf, String> {
    if let Some(p) = vault_path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
    {
        return Ok(std::path::PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("DISPUTATIO_VAULT_PATH") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return Ok(std::path::PathBuf::from(p));
        }
    }
    Err("no vault path: pass vaultPath or set DISPUTATIO_VAULT_PATH (settings screen lands with task 4)".to_string())
}

/// Re-index the vault: walk tracks + videos, seed from `scope.md`, upsert
/// idempotently on `vault_path`. Never duplicates rows; never regresses
/// chunk progress.
#[tauri::command]
pub async fn reindex(
    app: AppHandle,
    vault_path: Option<String>,
) -> Result<ReindexResponse, String> {
    // try_state (not State extraction) so a down database degrades to an
    // honest error instead of a missing-state panic (ADR 0003 degradation).
    let Some(db) = app.try_state::<Db>() else {
        return Err("database offline — persistence unavailable (ADR 0003)".to_string());
    };
    let vault_root = resolve_vault_root(vault_path)?;
    let repo = PgIndexRepo::new(db.0.clone());
    let report = index::reindex(&FsVaultReader, &repo, &vault_root)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ReindexResponse {
        sources: report.sources as u32,
        chunks: report.chunks as u32,
        done_seeded: report.done_seeded as u32,
    })
}
