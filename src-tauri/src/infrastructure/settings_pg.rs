//! Postgres adapter for the `SettingsStore` domain port: the `app_settings`
//! key/value table from migration 0001 (specs/queue-slice/design.md).

use sqlx::PgPool;

use crate::domain::settings::{SettingsError, SettingsStore};

/// Store backed by the shared pool (`pg::Db` state handle).
pub struct PgSettingsStore(pub PgPool);

fn store_error(error: sqlx::Error) -> SettingsError {
    SettingsError::Store(error.to_string())
}

impl SettingsStore for PgSettingsStore {
    async fn load_all(&self) -> Result<Vec<(String, String)>, SettingsError> {
        let rows = sqlx::query!("SELECT key, value FROM app_settings ORDER BY key")
            .fetch_all(&self.0)
            .await
            .map_err(store_error)?;
        Ok(rows.into_iter().map(|row| (row.key, row.value)).collect())
    }

    async fn put(&self, key: &str, value: &str) -> Result<(), SettingsError> {
        sqlx::query!(
            "INSERT INTO app_settings (key, value) VALUES ($1, $2)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            key,
            value
        )
        .execute(&self.0)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), SettingsError> {
        sqlx::query!("DELETE FROM app_settings WHERE key = $1", key)
            .execute(&self.0)
            .await
            .map_err(store_error)?;
        Ok(())
    }
}
