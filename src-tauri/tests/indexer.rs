//! Indexer integration (specs/queue-slice/tasks.md item 3): walk a fixture
//! vault, upsert through PgIndexRepo, and prove the re-index contract —
//! idempotent on `vault_path` (no duplicate rows) and progress-preserving
//! (seeding derives `queued` rows; anything past `queued` is kept).
//! Uses a throwaway Postgres 16 (ADR 0009) and a synthetic vault in a
//! tempdir — the real Obsidian vault is not in this repo.

use std::fs;
use std::path::Path;

use disputatio_lib::application::index;
use disputatio_lib::domain::repo::IndexReport;
use disputatio_lib::infrastructure::index_repo::PgIndexRepo;
use disputatio_lib::infrastructure::pg;
use disputatio_lib::infrastructure::vault_fs::FsVaultReader;
use sqlx::PgPool;
use sqlx::Row;
use tempfile::tempdir;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;

/// Fixture vault: two class tracks + the video track, with every seeding
/// shape represented (fully checked, partially checked, no checkboxes).
fn write_fixture_vault(root: &Path) {
    let scope = |rel: &str, body: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    };
    scope(
        "courses/fundamentos-enterprise/sessions/01-empresa/scope.md",
        "# Empresa\n\n- [x] Ler as notas\n- [x] Narrar o resumo\n",
    );
    scope(
        "courses/fundamentos-enterprise/sessions/02-modelos/scope.md",
        "# Modelos\n\n- [x] Ler as notas\n- [ ] Narrar o resumo\n",
    );
    scope(
        "courses/fundamentos-enterprise/sessions/03-proposito/scope.md",
        "- [x] Ler\n",
    );
    scope(
        "courses/system-design/sessions/01-intro/scope.md",
        "# Intro\n\nSem checklist aqui.\n",
    );
    let vid = |rel: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "video bytes").unwrap();
    };
    vid("courses/videos/modulo-a/aula.mp4");
    vid("courses/videos/b.mkv");
}

async fn upsert(pool: &PgPool, vault_root: &Path) -> IndexReport {
    let repo = PgIndexRepo::new(pool.clone());
    index::reindex(&FsVaultReader, &repo, vault_root)
        .await
        .expect("scan + upsert succeed")
}

async fn count(pool: &PgPool, table: &str) -> i64 {
    let sql = format!("SELECT count(*) AS n FROM {table}");
    sqlx::query(&sql)
        .fetch_one(pool)
        .await
        .unwrap()
        .get::<i64, _>("n")
}

async fn chunk_status(pool: &PgPool, vault_path: &str) -> String {
    sqlx::query("SELECT status FROM chunks WHERE vault_path = $1")
        .bind(vault_path)
        .fetch_one(pool)
        .await
        .unwrap()
        .get::<String, _>("status")
}

async fn chunk_updated_epoch(pool: &PgPool, vault_path: &str) -> i64 {
    sqlx::query(
        "SELECT EXTRACT(EPOCH FROM updated_at)::bigint AS updated_epoch
         FROM chunks WHERE vault_path = $1",
    )
    .bind(vault_path)
    .fetch_one(pool)
    .await
    .unwrap()
    .get::<i64, _>("updated_epoch")
}

#[tokio::test]
async fn reindex_is_idempotent_and_preserves_progress() {
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
        .expect("migrations apply cleanly");

    let vault = tempdir().expect("fixture vault tempdir");
    write_fixture_vault(vault.path());

    // First index: 2 class tracks + videos, 6 chunks, 2 seeded done.
    let first = upsert(&pool, vault.path()).await;
    assert_eq!(
        first,
        IndexReport {
            sources: 3,
            chunks: 6,
            done_seeded: 2
        },
        "walk found the fixture's tracks, classes and videos"
    );
    assert_eq!(count(&pool, "sources").await, 3);
    assert_eq!(count(&pool, "chunks").await, 6);
    assert_eq!(
        chunk_status(&pool, "courses/fundamentos-enterprise/sessions/01-empresa").await,
        "done",
        "fully checked scope.md seeds done"
    );
    assert_eq!(
        chunk_status(&pool, "courses/system-design/sessions/01-intro").await,
        "queued",
        "scope.md without checkboxes stays queued"
    );

    // Second index over the unchanged vault: identical report, identical rows.
    let second = upsert(&pool, vault.path()).await;
    assert_eq!(
        second, first,
        "re-index of the same vault writes the same rows"
    );
    assert_eq!(count(&pool, "sources").await, 3, "no duplicate sources");
    assert_eq!(count(&pool, "chunks").await, 6, "no duplicate chunks");

    // Progress beats seeding: in_progress stays in_progress even after its
    // scope.md is fully checked, and its untouched clock is not reset.
    sqlx::query(
        "UPDATE chunks SET status = 'in_progress', updated_at = now() - interval '20 days'
         WHERE vault_path = $1",
    )
    .bind("courses/fundamentos-enterprise/sessions/02-modelos")
    .execute(&pool)
    .await
    .unwrap();
    // Pipeline states (architecture.md sketch) are past `queued` too.
    sqlx::query("UPDATE chunks SET status = 'narrated' WHERE vault_path = $1")
        .bind("courses/fundamentos-enterprise/sessions/03-proposito")
        .execute(&pool)
        .await
        .unwrap();
    fs::write(
        vault
            .path()
            .join("courses/fundamentos-enterprise/sessions/02-modelos/scope.md"),
        "# Modelos\n\n- [x] Ler as notas\n- [x] Narrar o resumo\n",
    )
    .unwrap();
    let epoch_before =
        chunk_updated_epoch(&pool, "courses/fundamentos-enterprise/sessions/02-modelos").await;

    let third = upsert(&pool, vault.path()).await;
    assert_eq!(third.chunks, 6, "still no duplicates after re-index");
    assert_eq!(
        chunk_status(&pool, "courses/fundamentos-enterprise/sessions/02-modelos").await,
        "in_progress",
        "seeding must not regress progress"
    );
    assert_eq!(
        chunk_status(
            &pool,
            "courses/fundamentos-enterprise/sessions/03-proposito"
        )
        .await,
        "narrated",
        "pipeline states are preserved"
    );
    assert_eq!(
        chunk_updated_epoch(&pool, "courses/fundamentos-enterprise/sessions/02-modelos").await,
        epoch_before,
        "no-op re-index must not bump updated_at (the stale rule depends on it)"
    );

    // One video = one chunk, all queued (videos carry no scope.md).
    let row = sqlx::query(
        "SELECT count(*) AS n, count(*) FILTER (WHERE status = 'queued') AS queued
         FROM chunks WHERE vault_path LIKE 'courses/videos/%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("n"), 2, "one video = one chunk");
    assert_eq!(row.get::<i64, _>("queued"), 2, "video chunks start queued");
}
