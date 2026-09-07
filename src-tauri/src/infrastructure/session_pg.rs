//! sqlx adapter for the [`SessionRepo`] port: `sessions` + `session_stages`
//! from migration 0001 (ADR 0003 state store). Runtime queries, like the
//! other write-path adapters — no `.sqlx` churn.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::Row;

use crate::domain::repo::RepoError;
use crate::domain::session::{OpenSession, Session, SessionError, SessionRepo, Stage};

/// Write repo backed by the shared pool (`pg::Db` state handle).
pub struct PgSessionRepo(pub PgPool);

impl PgSessionRepo {
    pub fn new(pool: PgPool) -> Self {
        PgSessionRepo(pool)
    }
}

fn repo_err(e: sqlx::Error) -> SessionError {
    SessionError::Repo(RepoError::Database(e.to_string()))
}

fn session_row(row: sqlx::postgres::PgRow) -> Session {
    Session {
        id: row.get("id"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
    }
}

impl SessionRepo for PgSessionRepo {
    async fn insert_open_session(
        &self,
        chunk_id: i64,
        stage: Stage,
        now: DateTime<Utc>,
    ) -> Result<OpenSession, SessionError> {
        let mut tx = self.0.begin().await.map_err(repo_err)?;

        // The session logs against the chunk and flips it `in_progress` (the
        // queue's frontier + stale rule read this). Zero rows updated means
        // the chunk is missing or already done — a caller mistake, not a
        // database failure.
        let started = sqlx::query(
            "UPDATE chunks SET status = 'in_progress', updated_at = $2
             WHERE id = $1 AND status <> 'done'",
        )
        .bind(chunk_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(repo_err)?;
        if started.rows_affected() == 0 {
            return Err(SessionError::InvalidInput(format!(
                "chunk {chunk_id} not found or already done — index the vault first"
            )));
        }

        let row = sqlx::query(
            "INSERT INTO sessions (started_at) VALUES ($1) RETURNING id, started_at, ended_at",
        )
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(repo_err)?;
        let session = session_row(row);

        sqlx::query(
            "INSERT INTO session_stages (session_id, stage, chunk_id, started_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(session.id)
        .bind(stage.as_str())
        .bind(chunk_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(repo_err)?;

        tx.commit().await.map_err(repo_err)?;
        Ok(OpenSession { session, chunk_id })
    }

    async fn close_session(
        &self,
        session_id: i64,
        now: DateTime<Utc>,
    ) -> Result<Session, SessionError> {
        let mut tx = self.0.begin().await.map_err(repo_err)?;

        let row = sqlx::query(
            "UPDATE sessions SET ended_at = $2
             WHERE id = $1 AND ended_at IS NULL
             RETURNING id, started_at, ended_at",
        )
        .bind(session_id)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(repo_err)?
        .ok_or_else(|| SessionError::InvalidInput(format!("session {session_id} is not open")))?;

        // Close the session's still-open stage row alongside it.
        sqlx::query(
            "UPDATE session_stages SET ended_at = $2 WHERE session_id = $1 AND ended_at IS NULL",
        )
        .bind(session_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(repo_err)?;

        tx.commit().await.map_err(repo_err)?;
        Ok(session_row(row))
    }

    async fn open_session(&self) -> Result<Option<OpenSession>, SessionError> {
        // The session's open study stage names the chunk being studied. This
        // slice creates at most one open session; if more ever exist, the
        // latest wins.
        let row = sqlx::query(
            "SELECT s.id, s.started_at, s.ended_at, st.chunk_id
             FROM sessions s
             JOIN session_stages st ON st.session_id = s.id AND st.ended_at IS NULL
             WHERE s.ended_at IS NULL
             ORDER BY s.id DESC
             LIMIT 1",
        )
        .fetch_optional(&self.0)
        .await
        .map_err(repo_err)?;

        row.map(|row| {
            let chunk_id: i64 = row.get("chunk_id");
            Ok(OpenSession {
                session: session_row(row),
                chunk_id,
            })
        })
        .transpose()
    }

    async fn mark_chunk_done(&self, chunk_id: i64, now: DateTime<Utc>) -> Result<(), SessionError> {
        let result = sqlx::query(
            "UPDATE chunks SET status = 'done', updated_at = $2
             WHERE id = $1",
        )
        .bind(chunk_id)
        .bind(now)
        .execute(&self.0)
        .await
        .map_err(repo_err)?;

        if result.rows_affected() == 0 {
            return Err(SessionError::InvalidInput(format!(
                "chunk {chunk_id} not found — index the vault first"
            )));
        }
        Ok(())
    }
}
