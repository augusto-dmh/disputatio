//! sqlx implementation of the `IndexRepo` port (ADR 0003 state store).
//! Idempotent upsert on `vault_path` — the stable key with a UNIQUE
//! constraint in migration 0001 (specs/queue-slice/design.md).
//!
//! Upsert semantics (requirements: "re-index doesn't duplicate"; design:
//! seeding derives status):
//! - `queued` rows are re-derived from the scan — a `scope.md` completed
//!   outside the app advances the chunk to `done` on the next re-index.
//! - Rows already past `queued` (`in_progress` or ingest-pipeline states)
//!   keep their status — re-indexing never regresses progress.
//! - `updated_at` moves only when the status actually changes, so the stale
//!   rule (in_progress untouched ≥ 14 days) is not reset by no-op re-indexes.
//! - Removed-from-vault chunks are left in place: sessions reference chunks
//!   by id and the study record is history (ADR 0003) — deletion is not part
//!   of this slice.

use sqlx::PgPool;
use sqlx::Row;

use crate::domain::chunk::ChunkStatus;
use crate::domain::repo::{IndexRepo, IndexReport, RepoError};
use crate::domain::vault::VaultScan;

pub struct PgIndexRepo {
    pool: PgPool,
}

impl PgIndexRepo {
    pub fn new(pool: PgPool) -> Self {
        PgIndexRepo { pool }
    }
}

#[async_trait::async_trait]
impl IndexRepo for PgIndexRepo {
    async fn upsert_scan(&self, scan: &VaultScan) -> Result<IndexReport, RepoError> {
        // One transaction: a failed re-index leaves the previous index intact.
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut report = IndexReport::default();

        for scanned in &scan.sources {
            let source = &scanned.source;
            let source_row = sqlx::query(
                "INSERT INTO sources (kind, track, title, vault_path, ord)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (vault_path) DO UPDATE SET
                     kind = EXCLUDED.kind,
                     track = EXCLUDED.track,
                     title = EXCLUDED.title,
                     ord = EXCLUDED.ord
                 RETURNING id",
            )
            .bind(source.kind.as_str())
            .bind(&source.track)
            .bind(&source.title)
            .bind(&source.vault_path)
            .bind(source.ord)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
            let source_id: i64 = source_row.get("id");

            for chunk in &scanned.chunks {
                sqlx::query(
                    "INSERT INTO chunks (source_id, ord, title, vault_path, est_minutes, status)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT (vault_path) DO UPDATE SET
                         source_id = EXCLUDED.source_id,
                         ord = EXCLUDED.ord,
                         title = EXCLUDED.title,
                         est_minutes = EXCLUDED.est_minutes,
                         status = CASE WHEN chunks.status = 'queued'
                                       THEN EXCLUDED.status
                                       ELSE chunks.status END,
                         updated_at = CASE WHEN (CASE WHEN chunks.status = 'queued'
                                                      THEN EXCLUDED.status
                                                      ELSE chunks.status END) = chunks.status
                                           THEN chunks.updated_at
                                           ELSE now() END",
                )
                .bind(source_id)
                .bind(chunk.ord)
                .bind(&chunk.title)
                .bind(&chunk.vault_path)
                .bind(chunk.est_minutes)
                .bind(chunk.status.as_str())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;

                report.chunks += 1;
                if chunk.status == ChunkStatus::Done {
                    report.done_seeded += 1;
                }
            }
            report.sources += 1;
        }

        tx.commit().await.map_err(db_err)?;
        Ok(report)
    }
}

fn db_err(e: sqlx::Error) -> RepoError {
    RepoError::Database(e.to_string())
}
