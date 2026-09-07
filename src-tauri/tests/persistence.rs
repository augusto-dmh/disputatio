//! First integration test (ADR 0009: testcontainers-rs + Postgres).
//! Applies migration 0001 on a throwaway Postgres 16 (same tag as
//! docker/docker-compose.yml) and smoke-tests the queue-slice schema:
//! the five tables exist, and a chunk round-trips on its stable
//! `vault_path` key with idempotent upsert (specs/queue-slice/design.md).

use disputatio_lib::infrastructure::pg;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn migration_0001_applies_and_chunks_round_trip_on_vault_path() {
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
        .expect("migration 0001 applies cleanly");

    // The five queue-slice tables exist (tasks.md item 2).
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_all(&pool)
    .await
    .expect("list public tables");
    for expected in [
        "app_settings",
        "chunks",
        "session_stages",
        "sessions",
        "sources",
    ] {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table {expected} (found: {tables:?})"
        );
    }

    // Insert/select round-trip: a chunk keyed by vault_path (design.md).
    let source_id = sqlx::query!(
        "INSERT INTO sources (kind, track, title, vault_path, ord)
         VALUES ('course', 'fundamentos', 'Fundamentos Enterprise', 'courses/fundamentos-enterprise', 0)
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .expect("insert source")
    .id;

    let vault_path = "courses/fundamentos-enterprise/classes/class-001.md";
    let chunk_id = sqlx::query!(
        "INSERT INTO chunks (source_id, ord, title, vault_path, est_minutes)
         VALUES ($1, 1, 'Class 001', $2, 25)
         RETURNING id",
        source_id,
        vault_path
    )
    .fetch_one(&pool)
    .await
    .expect("insert chunk")
    .id;

    let row = sqlx::query!(
        "SELECT id, title, status, est_minutes FROM chunks WHERE vault_path = $1",
        vault_path
    )
    .fetch_one(&pool)
    .await
    .expect("select chunk by vault_path");
    assert_eq!(row.id, chunk_id);
    assert_eq!(row.title, "Class 001");
    assert_eq!(row.status, "queued");
    assert_eq!(row.est_minutes, Some(25));

    // Re-index upserts on vault_path — no duplicate row (requirements: idempotent).
    sqlx::query!(
        "INSERT INTO chunks (source_id, ord, title, vault_path, est_minutes)
         VALUES ($1, 1, 'Class 001', $2, 30)
         ON CONFLICT (vault_path) DO UPDATE SET est_minutes = EXCLUDED.est_minutes",
        source_id,
        vault_path
    )
    .execute(&pool)
    .await
    .expect("upsert chunk on vault_path");

    let (chunk_count, est_minutes) = sqlx::query!(
        "SELECT count(*) AS chunk_count, max(est_minutes) AS est_minutes FROM chunks WHERE vault_path = $1",
        vault_path
    )
    .fetch_one(&pool)
    .await
    .map(|r| (r.chunk_count, r.est_minutes))
    .expect("count chunks after upsert");
    assert_eq!(chunk_count, Some(1), "re-index must not duplicate chunks");
    assert_eq!(est_minutes, Some(30), "upsert updated the existing row");
}
