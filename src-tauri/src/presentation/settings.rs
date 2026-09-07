//! Settings commands (specs/queue-slice task 4) — translation only
//! (AGENTS.md layering): small DTOs in, one service call, small DTOs out.

use std::collections::BTreeMap;

use specta::Type;
use tauri::Manager;

use crate::application::settings::SettingsService;
use crate::domain::settings::{Settings, SettingsError};
use crate::infrastructure::pg::Db;
use crate::infrastructure::settings_pg::PgSettingsStore;

/// IPC shape of the settings aggregate. `track_positions` only carries tracks
/// with an explicit override; keys are track names, values are the chunk
/// `vault_path` the track is at (domain::Settings semantics).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Type)]
pub struct SettingsDto {
    pub vault_path: Option<String>,
    pub track_positions: BTreeMap<String, String>,
}

impl From<Settings> for SettingsDto {
    fn from(settings: Settings) -> Self {
        Self {
            vault_path: settings.vault_path,
            track_positions: settings.track_positions,
        }
    }
}

/// Builds the service from the managed pool. When the pool is absent the app
/// is running without persistence (lib.rs degrades instead of crashing).
fn service(app: &tauri::AppHandle) -> Result<SettingsService<PgSettingsStore>, String> {
    let db = app.try_state::<Db>().ok_or_else(|| {
        "database unavailable — settings cannot be read or written (start `make infra`)".to_string()
    })?;
    Ok(SettingsService::new(PgSettingsStore(db.0.clone())))
}

fn ipc_error(error: SettingsError) -> String {
    error.to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(app: tauri::AppHandle) -> Result<SettingsDto, String> {
    let service = service(&app)?;
    service
        .get()
        .await
        .map(SettingsDto::from)
        .map_err(ipc_error)
}

/// Partial update: `vault_path: null` leaves it unchanged (the screen only
/// sends what the user edits); a blank position clears that track's override.
/// Returns the stored state so the UI syncs to what actually persisted.
#[tauri::command]
#[specta::specta]
pub async fn set_settings(
    app: tauri::AppHandle,
    settings: SettingsDto,
) -> Result<SettingsDto, String> {
    let service = service(&app)?;
    service
        .apply(settings.vault_path, settings.track_positions)
        .await
        .map_err(ipc_error)?;
    service
        .get()
        .await
        .map(SettingsDto::from)
        .map_err(ipc_error)
}
