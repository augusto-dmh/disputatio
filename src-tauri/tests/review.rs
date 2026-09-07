//! Memoria-slice persistence (specs/memoria-slice/tasks.md item 1):
//! migration 0002 on a throwaway Postgres 16, cards + reviews round-trips,
//! the due query the review flow is built on, and the uniqueness contracts
//! the import's idempotency relies on. Runtime queries (no `query!`
//! macros) — new SQL avoids .sqlx churn.

use chrono::SubsecRound;
use disputatio_lib::infrastructure::pg;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

/// Container + pool: the caller must hold the container — testcontainers
/// stops it on drop, and a dropped guard silently orphans the pool.
async fn setup() -> (testcontainers::ContainerAsync<Postgres>, sqlx::PgPool) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container (docker daemon required, ADR 0003)");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port mapped to host");
    let pool = pg::create_pool(&format!(
        "postgres://postgres:postgres@localhost:{port}/postgres"
    ))
    .await
    .expect("connect sqlx pool");
    pg::run_migrations(&pool)
        .await
        .expect("migrations 0001 + 0002 apply cleanly");
    (container, pool)
}

#[tokio::test]
async fn migration_0002_applies_and_due_query_orders_kept_cards() {
    let (_container, pool) = setup().await;

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_all(&pool)
    .await
    .expect("list public tables");
    for expected in ["cards", "reviews"] {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table {expected} (found: {tables:?})"
        );
    }

    let now = chrono::Utc::now();
    for (front, due) in [
        ("old", now - chrono::Duration::hours(5)),
        ("newest", now),
        ("older", now - chrono::Duration::hours(50)),
    ] {
        sqlx::query("INSERT INTO cards (deck, front, back, due) VALUES ($1, $2, $3, $4)")
            .bind("fundamentos")
            .bind(front)
            .bind("the answer")
            .bind(due)
            .execute(&pool)
            .await
            .expect("insert card");
    }
    // Curation gates the review queue: drafts and killed cards never surface.
    for curation in ["draft", "killed"] {
        sqlx::query(
            "INSERT INTO cards (deck, front, back, due, curation) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind("fundamentos")
        .bind(curation)
        .bind("the answer")
        .bind(now - chrono::Duration::hours(99))
        .bind(curation)
        .execute(&pool)
        .await
        .expect("insert gated card");
    }

    let fronts: Vec<String> = sqlx::query_scalar(
        "SELECT front FROM cards \
         WHERE curation = 'kept' AND due <= $1 ORDER BY due",
    )
    .bind(now)
    .fetch_all(&pool)
    .await
    .expect("due query");
    assert_eq!(
        fronts,
        vec!["older", "old", "newest"],
        "kept cards due <= now, oldest due first; draft/killed never surface"
    );
}

#[tokio::test]
async fn grade_round_trip_persists_card_state_and_review_row_together() {
    let (_container, pool) = setup().await;
    // TIMESTAMPTZ stores microseconds; round to millis so the round-trip is exact.
    let now = chrono::Utc::now().trunc_subsecs(3);

    sqlx::query("INSERT INTO cards (deck, front, back) VALUES ('fundamentos', 'front', 'back')")
        .execute(&pool)
        .await
        .expect("insert new card");
    let (card_id,): (i64,) = sqlx::query_as("SELECT id FROM cards WHERE front = 'front'")
        .fetch_one(&pool)
        .await
        .expect("find card");

    // What application/review.rs will do in one transaction (task 4):
    // append the review row and move the card's FSRS state.
    let mut tx = pool.begin().await.expect("open transaction");
    sqlx::query(
        "INSERT INTO reviews (card_id, grade, reviewed_at, state_before, elapsed_days, scheduled_days, duration_ms) \
         VALUES ($1, 3, $2, NULL, 0, 4, 5800)",
    )
    .bind(card_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .expect("insert review row");
    sqlx::query(
        "UPDATE cards SET stability = 3.15, difficulty = 4.9, fsrs_state = 'review', \
         due = $2, last_review_at = $2, reps = reps + 1 WHERE id = $1",
    )
    .bind(card_id)
    .bind(now + chrono::Duration::days(4))
    .execute(&mut *tx)
    .await
    .expect("update card state");
    tx.commit().await.expect("commit grade");

    let (stability, difficulty, state, reps): (Option<f64>, Option<f64>, Option<String>, i32) =
        sqlx::query_as("SELECT stability, difficulty, fsrs_state, reps FROM cards WHERE id = $1")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("read card back");
    let due_after_grade: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT due FROM cards WHERE id = $1")
            .bind(card_id)
            .fetch_one(&pool)
            .await
            .expect("read due");
    assert_eq!(stability, Some(3.15));
    assert_eq!(difficulty, Some(4.9));
    assert_eq!(state.as_deref(), Some("review"));
    assert_eq!(
        due_after_grade,
        Some(now + chrono::Duration::days(4)),
        "the scheduler's next due is what persists"
    );
    assert_eq!(reps, 1);

    let (review_count, grade, state_before): (i64, i32, Option<String>) = sqlx::query_as(
        "SELECT count(*), max(grade), max(state_before) FROM reviews WHERE card_id = $1",
    )
    .bind(card_id)
    .fetch_one(&pool)
    .await
    .expect("read review row");
    assert_eq!(review_count, 1);
    assert_eq!(grade, 3, "good");
    assert_eq!(state_before, None, "first review has no prior state");
}

#[tokio::test]
async fn uniqueness_contracts_make_import_idempotency_enforceable() {
    let (_container, pool) = setup().await;
    let now = chrono::Utc::now().trunc_subsecs(3);

    sqlx::query(
        "INSERT INTO cards (deck, front, back, anki_card_id) VALUES ('d', 'f', 'b', 1492739572063)",
    )
    .execute(&pool)
    .await
    .expect("insert card with anki id");
    sqlx::query("INSERT INTO cards (deck, front, back, anki_card_id) VALUES ('d', 'f2', 'b', 1492739572999)")
        .execute(&pool)
        .await
        .expect("insert second card");

    // Re-importing the same Anki card must collide on the stable id...
    let dup = sqlx::query(
        "INSERT INTO cards (deck, front, back, anki_card_id) VALUES ('d', 'f', 'b', 1492739572063)",
    )
    .execute(&pool)
    .await;
    assert!(
        dup.err()
            .and_then(|e| e.as_database_error().map(|e| e.is_unique_violation()))
            .unwrap_or(false),
        "duplicate anki_card_id must be a unique violation"
    );

    // ...and the same revlog row must collide on (card_id, reviewed_at).
    let (card_id,): (i64,) = sqlx::query_as("SELECT id FROM cards WHERE front = 'f'")
        .fetch_one(&pool)
        .await
        .expect("find card");
    sqlx::query("INSERT INTO reviews (card_id, grade, reviewed_at) VALUES ($1, 3, $2)")
        .bind(card_id)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert first review");
    let dup = sqlx::query("INSERT INTO reviews (card_id, grade, reviewed_at) VALUES ($1, 3, $2)")
        .bind(card_id)
        .bind(now)
        .execute(&pool)
        .await;
    assert!(
        dup.err()
            .and_then(|e| e.as_database_error().map(|e| e.is_unique_violation()))
            .unwrap_or(false),
        "duplicate (card_id, reviewed_at) must be a unique violation"
    );
}
