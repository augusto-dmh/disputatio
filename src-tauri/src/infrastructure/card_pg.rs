//! sqlx adapter for the [`CardRepo`] port: the review flow's view over
//! `cards` × `reviews` (migration 0002, ADR 0003/0004). Runtime queries —
//! the same `.sqlx`-churn rationale as `queue_pg`. Curation filtering
//! (`kept`) lives in these WHERE clauses, per the domain contract.

use sqlx::PgPool;
use sqlx::Row;

use chrono::{DateTime, Utc};

use crate::domain::repo::RepoError;
use crate::domain::review::{Card, CardRepo, CardState, Curation, FsrsState, ReviewLog};

/// Read/write repo backed by the shared pool (`pg::Db` state handle). The
/// FSRS columns are this repo's to write and nobody else's (architecture.md
/// data-model contract).
pub struct PgCardRepo(pub PgPool);

fn db_err(e: sqlx::Error) -> RepoError {
    RepoError::Database(e.to_string())
}

fn card_from_row(row: sqlx::postgres::PgRow) -> Result<Card, RepoError> {
    let state_raw: Option<String> = row.try_get("fsrs_state").map_err(db_err)?;
    let state = state_raw
        .as_deref()
        .map(|s| {
            FsrsState::parse(s)
                .ok_or_else(|| RepoError::Database(format!("unexpected fsrs_state {s:?}")))
        })
        .transpose()?;
    let curation_raw: String = row.try_get("curation").map_err(db_err)?;
    Ok(Card {
        id: row.try_get("id").map_err(db_err)?,
        deck: row.try_get("deck").map_err(db_err)?,
        front: row.try_get("front").map_err(db_err)?,
        back: row.try_get("back").map_err(db_err)?,
        curation: Curation::parse(&curation_raw)
            .ok_or_else(|| RepoError::Database(format!("unexpected curation {curation_raw:?}")))?,
        due: row.try_get("due").map_err(db_err)?,
        stability: row.try_get("stability").map_err(db_err)?,
        difficulty: row.try_get("difficulty").map_err(db_err)?,
        state,
        last_review_at: row.try_get("last_review_at").map_err(db_err)?,
        reps: row.try_get("reps").map_err(db_err)?,
        lapses: row.try_get("lapses").map_err(db_err)?,
    })
}

const CARD_COLUMNS: &str = "id, deck, front, back, curation, due, stability, difficulty, \
                            fsrs_state, last_review_at, reps, lapses";

impl CardRepo for PgCardRepo {
    async fn due_count(&self, now: DateTime<Utc>) -> Result<u64, RepoError> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM cards WHERE curation = 'kept' AND due <= $1")
                .bind(now)
                .fetch_one(&self.0)
                .await
                .map_err(db_err)?;
        Ok(count.max(0) as u64)
    }

    async fn due_cards(&self, limit: i64, now: DateTime<Utc>) -> Result<Vec<Card>, RepoError> {
        let rows = sqlx::query(&format!(
            "SELECT {CARD_COLUMNS} FROM cards \
             WHERE curation = 'kept' AND due <= $1 \
             ORDER BY due, id LIMIT $2"
        ))
        .bind(now)
        .bind(limit)
        .fetch_all(&self.0)
        .await
        .map_err(db_err)?;
        rows.into_iter().map(card_from_row).collect()
    }

    async fn get(&self, id: i64) -> Result<Option<Card>, RepoError> {
        sqlx::query(&format!("SELECT {CARD_COLUMNS} FROM cards WHERE id = $1"))
            .bind(id)
            .fetch_optional(&self.0)
            .await
            .map_err(db_err)?
            .map(card_from_row)
            .transpose()
    }

    async fn apply_review(
        &self,
        card_id: i64,
        state: &CardState,
        log: &ReviewLog,
    ) -> Result<(), RepoError> {
        let mut tx = self.0.begin().await.map_err(db_err)?;
        sqlx::query(
            "INSERT INTO reviews (card_id, grade, reviewed_at, state_before, elapsed_days, \
             scheduled_days, duration_ms) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(card_id)
        .bind(log.grade.as_rating())
        .bind(log.reviewed_at)
        .bind(log.state_before.map(|s| s.as_str()))
        .bind(log.elapsed_days)
        .bind(log.scheduled_days)
        .bind(log.duration_ms)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query(
            "UPDATE cards SET stability = $2, difficulty = $3, fsrs_state = $4, due = $5, \
             last_review_at = $6, reps = $7, lapses = $8 WHERE id = $1",
        )
        .bind(card_id)
        .bind(state.stability)
        .bind(state.difficulty)
        .bind(state.state.as_str())
        .bind(state.due)
        .bind(state.last_review_at)
        .bind(state.reps)
        .bind(state.lapses)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }
}
