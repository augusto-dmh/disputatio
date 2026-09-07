//! Queue integration (specs/queue-slice task 5): the read model and the
//! daily-queue composition over a real Postgres (throwaway container, ADR
//! 0009) — rotation order, stale resurfacing, the `track_position` override
//! honored through the stored settings, and the Anki offline header when the
//! gateway points nowhere.

use chrono::{Duration, Utc};
use disputatio_lib::application::queue::build_daily_queue;
use disputatio_lib::domain::queue::{AnkiStatus, QueueRepo};
use disputatio_lib::domain::settings::SettingsStore;
use disputatio_lib::infrastructure::anki_connect::AnkiConnectGateway;
use disputatio_lib::infrastructure::pg;
use disputatio_lib::infrastructure::queue_pg::PgQueueRepo;
use disputatio_lib::infrastructure::settings_pg::PgSettingsStore;
use sqlx::{PgPool, Row};
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

/// Postgres + migrations; the container must stay alive for the pool to work.
async fn db_on_fresh_postgres() -> (PgPool, testcontainers::ContainerAsync<Postgres>) {
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
    (pool, container)
}

/// One `sources` row; returns its id.
async fn seed_source(pool: &PgPool, track: &str, kind: &str, ord: i32) -> i64 {
    sqlx::query(
        "INSERT INTO sources (kind, track, title, vault_path, ord)
         VALUES ($1, $2, $2, $3, $4) RETURNING id",
    )
    .bind(kind)
    .bind(track)
    .bind(format!("courses/{track}"))
    .bind(ord)
    .fetch_one(pool)
    .await
    .expect("insert source")
    .get::<i64, _>("id")
}

/// One `chunks` row, `days_old` days in the past on `updated_at`.
async fn seed_chunk(
    pool: &PgPool,
    source_id: i64,
    ord: i32,
    vault_path: &str,
    status: &str,
    days_old: i64,
) -> i64 {
    sqlx::query(
        "INSERT INTO chunks (source_id, ord, title, vault_path, status, updated_at)
         VALUES ($1, $2, $2, $3, $4, now() - ($5::int * interval '1 day'))
         RETURNING id",
    )
    .bind(source_id)
    .bind(ord)
    .bind(vault_path)
    .bind(status)
    .bind(days_old as i32)
    .fetch_one(pool)
    .await
    .expect("insert chunk")
    .get::<i64, _>("id")
}

/// The shared fixture: the three v1 tracks plus an extra one, every state the
/// rules must distinguish. `updated_at` is expressed in days before now().
async fn seed_fixture(pool: &PgPool) {
    let fundamentos = seed_source(pool, "fundamentos-enterprise", "course", 0).await;
    seed_chunk(
        pool,
        fundamentos,
        1,
        "courses/fundamentos-enterprise/item-01",
        "done",
        0,
    )
    .await;
    seed_chunk(
        pool,
        fundamentos,
        2,
        "courses/fundamentos-enterprise/item-02",
        "in_progress",
        20, // stale
    )
    .await;
    seed_chunk(
        pool,
        fundamentos,
        3,
        "courses/fundamentos-enterprise/item-03",
        "queued",
        0,
    )
    .await;
    seed_chunk(
        pool,
        fundamentos,
        4,
        "courses/fundamentos-enterprise/item-04",
        "queued",
        0,
    )
    .await;

    let system_design = seed_source(pool, "system-design", "course", 1).await;
    seed_chunk(
        pool,
        system_design,
        1,
        "courses/system-design/item-01",
        "in_progress",
        1, // fresh frontier
    )
    .await;
    seed_chunk(
        pool,
        system_design,
        2,
        "courses/system-design/item-02",
        "queued",
        0,
    )
    .await;

    let videos = seed_source(pool, "videos", "video", 2).await;
    seed_chunk(pool, videos, 1, "courses/videos/a.mp4", "done", 0).await;
    seed_chunk(pool, videos, 2, "courses/videos/b.mp4", "queued", 0).await;

    // An unknown track: sorts after the canonical rotation; its done head is
    // skipped and only the queued tail surfaces.
    let philosophy = seed_source(pool, "philosophy", "course", 3).await;
    seed_chunk(pool, philosophy, 1, "courses/philosophy/item-01", "done", 0).await;
    seed_chunk(
        pool,
        philosophy,
        2,
        "courses/philosophy/item-02",
        "queued",
        0,
    )
    .await;
}

/// An AnkiConnect endpoint where nothing listens (ephemeral port, released).
async fn dead_anki_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn open_chunks_returns_only_open_rows_in_stable_order() {
    let (pool, _container) = db_on_fresh_postgres().await;
    seed_fixture(&pool).await;

    let chunks = PgQueueRepo(pool.clone())
        .open_chunks()
        .await
        .expect("read model loads");

    let paths: Vec<&str> = chunks.iter().map(|c| c.vault_path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "courses/fundamentos-enterprise/item-02",
            "courses/fundamentos-enterprise/item-03",
            "courses/fundamentos-enterprise/item-04",
            // SQL order is alphabetical by track — the rotation reorders in
            // the domain, not here.
            "courses/philosophy/item-02",
            "courses/system-design/item-01",
            "courses/system-design/item-02",
            "courses/videos/b.mp4",
        ],
        "done rows excluded; track + course order"
    );
    assert!(
        chunks
            .iter()
            .all(|c| c.status != disputatio_lib::domain::chunk::ChunkStatus::Done),
        "no done rows cross the port"
    );
}

#[tokio::test]
async fn daily_queue_rotates_surfaces_stale_and_degrades_without_anki() {
    let (pool, _container) = db_on_fresh_postgres().await;
    seed_fixture(&pool).await;

    let anki = AnkiConnectGateway::new(dead_anki_url().await);
    let queue = build_daily_queue(
        &PgQueueRepo(pool.clone()),
        &PgSettingsStore(pool.clone()),
        &anki,
        Utc::now(),
    )
    .await
    .expect("queue builds");

    assert_eq!(
        queue.anki,
        AnkiStatus::Offline,
        "dead gateway degrades gracefully"
    );

    let sections: Vec<(&str, Vec<(&str, bool)>)> = queue
        .tracks
        .iter()
        .map(|t| {
            (
                t.track.as_str(),
                t.items
                    .iter()
                    .map(|i| (i.chunk.vault_path.as_str(), i.stale))
                    .collect(),
            )
        })
        .collect();

    assert_eq!(
        sections,
        vec![
            (
                "fundamentos-enterprise",
                vec![
                    ("courses/fundamentos-enterprise/item-02", true),
                    ("courses/fundamentos-enterprise/item-03", false),
                ],
            ),
            (
                "system-design",
                vec![
                    ("courses/system-design/item-01", false),
                    ("courses/system-design/item-02", false),
                ],
            ),
            ("videos", vec![("courses/videos/b.mp4", false)]),
            ("philosophy", vec![("courses/philosophy/item-02", false)]),
        ],
        "canonical rotation first, extras after; stale ahead of the track's new chunks"
    );
}

#[tokio::test]
async fn stored_position_override_reaches_the_daily_queue() {
    let (pool, _container) = db_on_fresh_postgres().await;
    seed_fixture(&pool).await;

    // "fundamentos is at item-03": the stale item-02 sits before the
    // position, so the override skips it too.
    PgSettingsStore(pool.clone())
        .put(
            "track_position:fundamentos-enterprise",
            "courses/fundamentos-enterprise/item-03",
        )
        .await
        .expect("store the override");

    let anki = AnkiConnectGateway::new(dead_anki_url().await);
    let queue = build_daily_queue(
        &PgQueueRepo(pool.clone()),
        &PgSettingsStore(pool.clone()),
        &anki,
        Utc::now(),
    )
    .await
    .expect("queue builds");

    let fundamentos = queue
        .tracks
        .iter()
        .find(|t| t.track == "fundamentos-enterprise")
        .expect("fundamentos section present");
    let paths: Vec<&str> = fundamentos
        .items
        .iter()
        .map(|i| i.chunk.vault_path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["courses/fundamentos-enterprise/item-03"],
        "the override makes everything before it past, stale included"
    );
}

/// `updated_at` moves with the seeded ages — the stale rule reads real
/// timestamps, so a fresh `in_progress` row must not resurface as stale even
/// a hair under the threshold.
#[tokio::test]
async fn thirteen_day_old_in_progress_is_not_stale() {
    let (pool, _container) = db_on_fresh_postgres().await;
    let fundamentos = seed_source(&pool, "fundamentos-enterprise", "course", 0).await;
    let id = seed_chunk(
        &pool,
        fundamentos,
        1,
        "courses/fundamentos-enterprise/item-01",
        "in_progress",
        13,
    )
    .await;
    let stored: chrono::DateTime<Utc> = sqlx::query("SELECT updated_at FROM chunks WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .expect("chunk row")
        .get("updated_at");
    assert!(
        (Utc::now() - stored) < Duration::days(14),
        "fixture sanity: 13-day-old row reads back under the threshold"
    );

    let anki = AnkiConnectGateway::new(dead_anki_url().await);
    let queue = build_daily_queue(
        &PgQueueRepo(pool.clone()),
        &PgSettingsStore(pool.clone()),
        &anki,
        Utc::now(),
    )
    .await
    .expect("queue builds");

    let items = &queue.tracks[0].items;
    assert_eq!(items.len(), 1);
    assert!(!items[0].stale, "13 days is not yet stale");
}
