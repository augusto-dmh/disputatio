//! Manual smoke for slice 0.1 (specs/queue-slice/requirements.md "Done
//! when"), updated for slice 0.2's due source (specs/memoria-slice): the
//! full loop against a REAL vault — index twice (idempotency), build the
//! daily queue (rotation, stale flags, internal due header), start/stop a
//! session, and read the study record back via SQL.
//!
//! Not part of CI. Run with:
//!
//! ```text
//! DISPUTATIO_VAULT_PATH=/path/to/vault \
//!   cargo test --test smoke -- --ignored --nocapture
//! ```
//!
//! Uses `DATABASE_URL` when set (your running `make infra` Postgres — rows
//! persist), otherwise a throwaway Postgres 16 container (ADR 0009). The
//! due header counts the internal memoria backlog; Anki is not part of the
//! queue anymore (ADR 0004: never required at runtime after 0.2).

use std::path::PathBuf;

use chrono::Utc;
use disputatio_lib::application::queue::build_daily_queue;
use disputatio_lib::application::session::{start_session, stop_session};
use disputatio_lib::infrastructure::card_pg::PgCardRepo;
use disputatio_lib::infrastructure::index_repo::PgIndexRepo;
use disputatio_lib::infrastructure::pg;
use disputatio_lib::infrastructure::queue_pg::PgQueueRepo;
use disputatio_lib::infrastructure::session_pg::PgSessionRepo;
use disputatio_lib::infrastructure::settings_pg::PgSettingsStore;
use disputatio_lib::infrastructure::vault_fs::FsVaultReader;
use sqlx::Row;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
#[ignore = "manual smoke: needs DISPUTATIO_VAULT_PATH (a real vault); run with --ignored --nocapture"]
async fn slice_0_1_smoke_real_vault_end_to_end() {
    let vault_root = PathBuf::from(
        std::env::var("DISPUTATIO_VAULT_PATH")
            .expect("set DISPUTATIO_VAULT_PATH to the Obsidian vault root"),
    );
    assert!(
        vault_root.is_dir(),
        "DISPUTATIO_VAULT_PATH is not a directory: {}",
        vault_root.display()
    );

    // Real shared Postgres when DATABASE_URL is set; throwaway otherwise.
    let container: Option<testcontainers::ContainerAsync<Postgres>>;
    let pool = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            container = None;
            pg::create_pool(&url)
                .await
                .expect("connect to DATABASE_URL")
        }
        Err(_) => {
            let c = Postgres::default()
                .with_tag("16-alpine")
                .start()
                .await
                .expect("start throwaway postgres");
            let port = c.get_host_port_ipv4(5432).await.expect("port mapped");
            let pool = pg::create_pool(&format!(
                "postgres://postgres:postgres@localhost:{port}/postgres"
            ))
            .await
            .expect("connect sqlx pool");
            container = Some(c);
            pool
        }
    };
    pg::run_migrations(&pool).await.expect("migrations apply");

    // 1. Index the real vault twice: no duplicates, no drift.
    let repo = PgIndexRepo::new(pool.clone());
    let first = disputatio_lib::application::index::reindex(&FsVaultReader, &repo, &vault_root)
        .await
        .expect("first reindex");
    let second = disputatio_lib::application::index::reindex(&FsVaultReader, &repo, &vault_root)
        .await
        .expect("second reindex");
    assert_eq!(first, second, "re-index must be idempotent (requirements)");
    let rows: i64 = sqlx::query("SELECT count(*) AS n FROM chunks")
        .fetch_one(&pool)
        .await
        .expect("count")
        .get("n");
    assert_eq!(rows as usize, second.chunks, "row count matches the report");
    println!("[smoke] index report: {first:?}");

    // 2. The daily queue: rotation, stale flags, the internal due header.
    let queue = build_daily_queue(
        &PgQueueRepo(pool.clone()),
        &PgSettingsStore(pool.clone()),
        &PgCardRepo(pool.clone()),
        Utc::now(),
    )
    .await
    .expect("queue builds");
    println!("[smoke] internal due reviews: {}", queue.due);
    assert!(
        !queue.tracks.is_empty(),
        "the real vault produced at least one queue section"
    );
    for track in &queue.tracks {
        for item in &track.items {
            println!(
                "[smoke] queue · {:<24} {} {} — {}",
                track.track,
                if item.stale { "[stale] " } else { "" },
                item.chunk.status.as_str(),
                item.chunk.title
            );
        }
    }

    // 3. Start/stop a session on the first surfaced chunk; the record must
    //    land in Postgres without any manual dashboard edits.
    let first_chunk = queue.tracks[0].items[0].chunk.chunk_id;
    let opened = start_session(&PgSessionRepo(pool.clone()), first_chunk, Utc::now())
        .await
        .expect("session starts");
    println!(
        "[smoke] session {} started on chunk {first_chunk}",
        opened.session.id
    );
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let stopped = stop_session(&PgSessionRepo(pool.clone()), Utc::now())
        .await
        .expect("session stops");
    let duration = stopped.duration_seconds().expect("stop records duration");
    println!("[smoke] session stopped — {duration}s logged");

    // 4. The study record, visible via SQL: closed session, closed lectio
    //    stage row bound to the chunk, duration derivable from the row.
    let row = sqlx::query(
        "SELECT s.ended_at, st.stage, st.chunk_id, st.ended_at AS stage_ended
         FROM sessions s
         JOIN session_stages st ON st.session_id = s.id
         WHERE s.id = $1",
    )
    .bind(opened.session.id)
    .fetch_one(&pool)
    .await
    .expect("session row visible via SQL");
    let stage: String = row.get("stage");
    let chunk_via_sql: i64 = row.get("chunk_id");
    assert!(row
        .get::<Option<chrono::DateTime<Utc>>, _>("ended_at")
        .is_some());
    assert_eq!(
        stage, "lectio",
        "the study stage row (migration 0001 enum as-is)"
    );
    assert_eq!(chunk_via_sql, first_chunk);
    assert!(row
        .get::<Option<chrono::DateTime<Utc>>, _>("stage_ended")
        .is_some());
    println!("[smoke] study record verified via SQL: session + lectio stage + duration");

    drop(container); // keep a throwaway container alive until here
}
