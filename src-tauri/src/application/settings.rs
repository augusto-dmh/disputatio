//! Settings service (specs/queue-slice task 4): reads and writes the settings
//! aggregate through the domain port, owning validation and key semantics.

use std::collections::BTreeMap;

use crate::domain::settings::{
    track_from_key, track_position_key, Settings, SettingsError, SettingsStore, TRACKS,
    VAULT_PATH_KEY,
};

/// Stateless service over a `SettingsStore`. Generic because the store is only
/// ever reached through its port (hexagonal, docs/architecture.md layering).
pub struct SettingsService<S: SettingsStore> {
    store: S,
}

impl<S: SettingsStore> SettingsService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Hands back the underlying store (tests, diagnostics).
    pub fn into_store(self) -> S {
        self.store
    }

    /// Load the aggregate, keeping only keys this version understands —
    /// position overrides of unknown tracks stay in storage but invisible
    /// here (symmetric with the strict write path, forward compatible).
    pub async fn get(&self) -> Result<Settings, SettingsError> {
        let mut settings = Settings::default();
        for (key, value) in self.store.load_all().await? {
            if key == VAULT_PATH_KEY {
                settings.vault_path = Some(value);
            } else if let Some(track) = track_from_key(&key) {
                if TRACKS.contains(&track) {
                    settings.track_positions.insert(track.to_string(), value);
                }
            }
        }
        Ok(settings)
    }

    /// Set the vault root. The path is trimmed; blank paths are rejected
    /// (a missing vault path is simply the unset case, not an empty one).
    pub async fn set_vault_path(&self, path: &str) -> Result<(), SettingsError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(SettingsError::InvalidInput(
                "vault path cannot be empty".to_string(),
            ));
        }
        self.store.put(VAULT_PATH_KEY, path).await
    }

    /// Set (or, with a blank position, clear) a track's position override.
    /// Only the known v1 tracks are accepted.
    pub async fn set_track_position(
        &self,
        track: &str,
        position: &str,
    ) -> Result<(), SettingsError> {
        if !TRACKS.contains(&track) {
            return Err(SettingsError::InvalidInput(format!(
                "unknown track: {track} (known: {TRACKS:?})"
            )));
        }
        let key = track_position_key(track);
        let position = position.trim();
        if position.is_empty() {
            return self.store.delete(&key).await;
        }
        self.store.put(&key, position).await
    }

    /// Apply a partial update: a `Some` vault path is set (unchanged when
    /// `None`); each position entry is set, a blank one clearing the override.
    pub async fn apply(
        &self,
        vault_path: Option<String>,
        track_positions: BTreeMap<String, String>,
    ) -> Result<(), SettingsError> {
        if let Some(path) = vault_path {
            self.set_vault_path(&path).await?;
        }
        for (track, position) in &track_positions {
            self.set_track_position(track, position).await?;
        }
        Ok(())
    }
}
