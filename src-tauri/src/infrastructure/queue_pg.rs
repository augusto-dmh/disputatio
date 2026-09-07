//! sqlx adapter for the [`QueueRepo`] port: the queue's read model over
//! `chunks` × `sources` (migration 0001, ADR 0003 state store). Runtime
//! queries, same rationale as `index_repo`: the queue's shape is stable and
//! this keeps `.sqlx` churn out of the diff.

use sqlx::PgPool;
use sqlx::Row;

use crate::domain::chunk::ChunkStatus;
use crate::domain::queue::{QueueChunk, QueueRepo};
use crate::domain::repo::RepoError;

/// Read repo backed by the shared pool (`pg::Db` state handle). The queue
/// never writes chunks — status changes go through the session commands.
pub struct PgQueueRepo(pub PgPool);

fn db_err(e: sqlx::Error) -> RepoError {
    RepoError::Database(e.to_string())
}

impl QueueRepo for PgQueueRepo {
    /// Every non-done chunk with its track and course order. `done` rows are
    /// the queue's past — excluded here so the payload stays the daily answer,
    /// not the whole history.
    async fn open_chunks(&self) -> Result<Vec<QueueChunk>, RepoError> {
        let rows = sqlx::query(
            "SELECT c.id, s.track, c.ord, c.title, c.vault_path, c.est_minutes, c.status, c.updated_at
             FROM chunks c
             JOIN sources s ON s.id = c.source_id
             WHERE c.status IN ('queued', 'in_progress')
             ORDER BY s.track, c.ord, c.id",
        )
        .fetch_all(&self.0)
        .await
        .map_err(db_err)?;

        rows.into_iter()
            .map(|row| {
                let status_raw: String = row.try_get("status").map_err(db_err)?;
                let status = ChunkStatus::parse(&status_raw).ok_or_else(|| {
                    RepoError::Database(format!("unexpected chunk status {status_raw:?}"))
                })?;
                Ok(QueueChunk {
                    chunk_id: row.try_get("id").map_err(db_err)?,
                    track: row.try_get("track").map_err(db_err)?,
                    ord: row.try_get("ord").map_err(db_err)?,
                    title: row.try_get("title").map_err(db_err)?,
                    vault_path: row.try_get("vault_path").map_err(db_err)?,
                    est_minutes: row.try_get("est_minutes").map_err(db_err)?,
                    updated_at: row.try_get("updated_at").map_err(db_err)?,
                    status,
                })
            })
            .collect()
    }
}
