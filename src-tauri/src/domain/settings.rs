//! Settings entity and store port (docs/architecture.md: the domain owns the
//! rules; infrastructure implements the world). Key semantics and validation
//! live here — the durable key/value store sits behind `SettingsStore`.

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;

/// Canonical key for the Obsidian vault root (specs/queue-slice/design.md
/// "Data model changes": vault path, per-track position).
pub const VAULT_PATH_KEY: &str = "vault_path";

/// Prefix under which per-track position overrides are stored: `<prefix><track>`.
pub const TRACK_POSITION_PREFIX: &str = "track_position:";

/// The v1 tracks (specs/queue-slice/requirements.md): fundamentos-enterprise
/// (64 classes), system-design (16 classes), videos (one video = one chunk).
pub const TRACKS: [&str; 3] = ["fundamentos-enterprise", "system-design", "videos"];

/// The settings aggregate the rest of the app consumes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    /// Absolute path to the Obsidian vault root (ADR 0003: the vault is
    /// knowledge truth; the app only ever reads it).
    pub vault_path: Option<String>,
    /// Position overrides keyed by track name. The value is the stable
    /// `vault_path` of the chunk the track is currently at (design.md chunk
    /// identity); an absent track means "no override — derive from status".
    pub track_positions: BTreeMap<String, String>,
}

/// Failure modes of the settings service and its store. Presentation maps
/// these to IPC-shaped strings.
#[derive(Debug)]
pub enum SettingsError {
    /// The backing store failed; the message is display-only.
    Store(String),
    /// The caller tried to write something the domain rejects.
    InvalidInput(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Store(message) => write!(f, "settings store error: {message}"),
            SettingsError::InvalidInput(message) => write!(f, "invalid setting: {message}"),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Port: durable key/value storage for app settings. Implemented in
/// infrastructure (the `app_settings` table from migration 0001); services in
/// application compose this port only.
///
/// Async trait methods are desugared to `impl Future + Send` (the form the
/// compiler recommends over bare `async fn` in public traits: the returned
/// futures must stay `Send` for Tauri's async runtime).
pub trait SettingsStore: Send + Sync {
    /// Every stored key/value pair, in stable order.
    fn load_all(&self)
        -> impl Future<Output = Result<Vec<(String, String)>, SettingsError>> + Send;
    /// Insert or overwrite one key.
    fn put(&self, key: &str, value: &str)
        -> impl Future<Output = Result<(), SettingsError>> + Send;
    /// Remove one key; removing a missing key is not an error.
    fn delete(&self, key: &str) -> impl Future<Output = Result<(), SettingsError>> + Send;
}

/// Storage key for a track's position override.
pub fn track_position_key(track: &str) -> String {
    format!("{TRACK_POSITION_PREFIX}{track}")
}

/// The track behind a stored key, if the key is a position override.
pub fn track_from_key(key: &str) -> Option<&str> {
    key.strip_prefix(TRACK_POSITION_PREFIX)
}
