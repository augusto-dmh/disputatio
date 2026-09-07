//! Postgres access via sqlx (ADR 0003 state store, ADR 0009 stack pinning):
//! pool creation, embedded migrations, and the Tauri state handle.
//! Repository logic lands with the queue-slice tasks that need it.

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Tauri state handle: managed only while the pool is connected, so a down
/// database degrades to "app without persistence" instead of a crash.
pub struct Db(pub PgPool);

/// Connect with bounded patience so an unreachable DB fails fast.
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

/// Apply the embedded migrations (sqlx migrate, ADR 0009).
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Database URL from the environment: `DATABASE_URL` wins; otherwise the
/// compose variables from `.env` (see `.env.example`, ADR 0003) are assembled.
pub fn database_url_from_env() -> Option<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Some(url);
    }
    let password = std::env::var("POSTGRES_PASSWORD").ok()?;
    let user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "disputatio".to_string());
    let db = std::env::var("POSTGRES_DB").unwrap_or_else(|_| user.clone());
    let port = std::env::var("PGPORT").unwrap_or_else(|_| "5433".to_string());
    Some(format!(
        "postgres://{user}:{password}@localhost:{port}/{db}"
    ))
}
