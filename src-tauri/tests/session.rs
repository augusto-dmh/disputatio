//! Session integration (specs/queue-slice task 7): the study loop over a
//! real Postgres (throwaway container, ADR 0009): start writes the session
//! and study-stage rows and flips the chunk `in_progress`; a second start is
//! rejected; stop records duration and closes the stage; and completing a
//! chunk hands "next" to the queue rules. The study stage follows migration
//! 0001's enum as-is (ADR 0005, Proposed, may amend it).

use chrono::{DateTime, Utc};
use disputatio_lib::application::session::{complete_chunk, start_session, stop_session};
use disputatio_lib::domain::chunk::ChunkStatus;
use disputatio_lib::domain::queue::{build_queue, QueueRepo};
use disputatio_lib::domain::session::{SessionError, SessionRepo};
use disputatio_lib::domain::settings::SettingsStore;
use disputatio_lib::infrastructure::pg;
use disputatio_lib::infrastructure::queue_pg::PgQueueRepo;
use disputatio_lib::infrastructure::session_pg::PgSessionRepo;
use disputatio_lib::infrastructure::settings_pg::PgSettingsStore;
use sqlx::{PgPool, Row};
use std::collections::BTreeMap;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

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

/// One track, three queued chunks; returns the chunk ids in course order.
async fn seed_track(pool: &PgPool) -> Vec<i64> {
    let source_id = sqlx::query(
        "INSERT INTO sources (kind, track, title, vault_path, ord)
         VALUES ('course', 'system-design', 'System Design', 'courses/system-design', 0)
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("insert source")
    .get::<i64, _>("id");

    let mut ids = Vec::new();
    for ord in 1..=3 {
        let id = sqlx::query(
            "INSERT INTO chunks (source_id, ord, title, vault_path)
             VALUES ($1, $2, $2, $3) RETURNING id",
        )
        .bind(source_id)
        .bind(ord)
        .bind(format!("courses/system-design/item-{ord:02}"))
        .fetch_one(pool)
        .await
        .expect("insert chunk")
        .get::<i64, _>("id");
        ids.push(id);
    }
    ids
}

async fn chunk_status(pool: &PgPool, chunk_id: i64) -> String {
    sqlx::query("SELECT status FROM chunks WHERE id = $1")
        .bind(chunk_id)
        .fetch_one(pool)
        .await
        .expect("chunk row")
        .get("status")
}

#[tokio::test]
async fn the_study_loop_writes_honest_rows_end_to_end() {
    let (pool, _container) = db_on_fresh_postgres().await;
    let chunks = seed_track(&pool).await;
    let repo = PgSessionRepo::new(pool.clone());

    // START: session row + study stage row + chunk flips in_progress.
    let now: DateTime<Utc> = Utc::now();
    let open = start_session(&repo, chunks[0], now)
        .await
        .expect("session starts");
    assert_eq!(open.chunk_id, chunks[0]);
    assert_eq!(chunk_status(&pool, chunks[0]).await, "in_progress");

    let stages: Vec<(String, Option<i64>, Option<String>)> =
        sqlx::query("SELECT stage, chunk_id, ended_at FROM session_stages")
            .fetch_all(&pool)
            .await
            .expect("stage rows")
            .into_iter()
            .map(|row| (row.get("stage"), row.get("chunk_id"), row.get("ended_at")))
            .collect();
    assert_eq!(
        stages,
        vec![("lectio".to_string(), Some(chunks[0]), None)],
        "one study-stage row, open, bound to the chunk"
    );

    // A second start is rejected while the session is open.
    let err = start_session(&repo, chunks[1], Utc::now())
        .await
        .expect_err("one session at a time");
    assert!(matches!(err, SessionError::InvalidInput(_)));

    // The open session survives a "restart": the UI can find it again.
    let resumed = repo.open_session().await.expect("read open session");
    assert_eq!(resumed.map(|o| o.chunk_id), Some(chunks[0]));

    // STOP: end time + duration land on the session, the stage row closes.
    let stopped = stop_session(&repo, Utc::now())
        .await
        .expect("session stops");
    assert!(stopped.duration_seconds().is_some());
    let stage_open: i64 =
        sqlx::query("SELECT count(*) AS n FROM session_stages WHERE ended_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("stage count")
            .get("n");
    assert_eq!(stage_open, 0, "the study stage closed with the session");
    assert!(repo.open_session().await.expect("read").is_none());

    // COMPLETE: status flips done, the queue's next-in-line takes over.
    complete_chunk(&repo, chunks[0], Utc::now())
        .await
        .expect("chunk completes");
    assert_eq!(chunk_status(&pool, chunks[0]).await, "done");

    let open_paths = PgQueueRepo(pool.clone())
        .open_chunks()
        .await
        .expect("read model");
    let paths: Vec<&str> = open_paths.iter().map(|c| c.vault_path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "courses/system-design/item-02",
            "courses/system-design/item-03"
        ],
        "the done chunk left the queue; items 02 and 03 are next"
    );
}

#[tokio::test]
async fn starting_a_missing_or_done_chunk_is_rejected_honestly() {
    let (pool, _container) = db_on_fresh_postgres().await;
    let chunks = seed_track(&pool).await;
    let repo = PgSessionRepo::new(pool.clone());

    let err = start_session(&repo, 99_999, Utc::now())
        .await
        .expect_err("unknown chunk");
    assert!(matches!(err, SessionError::InvalidInput(_)));

    complete_chunk(&repo, chunks[0], Utc::now())
        .await
        .expect("chunk completes");
    let err = start_session(&repo, chunks[0], Utc::now())
        .await
        .expect_err("done chunks are past — no restart");
    assert!(
        matches!(err, SessionError::InvalidInput(_)),
        "completing is one-way in this slice"
    );

    // Completing an unknown chunk reports the same honesty.
    let err = complete_chunk(&repo, 99_999, Utc::now())
        .await
        .expect_err("unknown chunk");
    assert!(matches!(err, SessionError::InvalidInput(_)));
}

/// A stopped-midway chunk (still `in_progress`) leads its track's section as
/// the fresh frontier — started work never vanishes from the queue.
#[tokio::test]
async fn the_queue_surfaces_a_stopped_midway_chunk_as_the_frontier() {
    let (pool, _container) = db_on_fresh_postgres().await;
    let chunks = seed_track(&pool).await;
    let repo = PgSessionRepo::new(pool.clone());

    start_session(&repo, chunks[0], Utc::now())
        .await
        .expect("session starts");
    stop_session(&repo, Utc::now())
        .await
        .expect("session stops");

    let queue = build_queue(
        PgQueueRepo(pool.clone()).open_chunks().await.expect("rows"),
        &BTreeMap::new(),
        0,
        Utc::now(),
    );
    let first = &queue.tracks[0].items[0];
    assert_eq!(first.chunk.chunk_id, chunks[0], "the midway chunk leads");
    assert_eq!(first.chunk.status, ChunkStatus::InProgress);
    assert!(!first.stale, "fresh activity is not stale");

    // The settings store round-trips beside it (wiring sanity for the
    // command's ports — the override reaches the queue through task 5).
    let store = PgSettingsStore(pool.clone());
    store
        .put(
            "track_position:system-design",
            "courses/system-design/item-02",
        )
        .await
        .expect("override stored");
    let loaded = store.load_all().await.expect("loaded");
    assert_eq!(
        loaded,
        vec![(
            "track_position:system-design".to_string(),
            "courses/system-design/item-02".to_string()
        )]
    );
}
