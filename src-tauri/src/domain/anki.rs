//! Anki port — a read-only view over the user's Anki collection.
//!
//! ADR 0004: AnkiConnect serves as an interim due-count reader until the
//! built-in FSRS scheduler lands (slice 0.2). The domain only knows Anki
//! through this port; how the questions travel is an infrastructure detail.

use std::fmt;
use std::future::Future;

/// Failure modes when reading from Anki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnkiError {
    /// Anki is not reachable: not running, AnkiConnect not listening, or the
    /// request timed out. The queue must degrade gracefully — surface
    /// "Anki offline" and continue normally.
    Offline,
    /// Anki answered, but not with what was asked: an AnkiConnect error
    /// payload, an unexpected response shape, or a non-success HTTP status.
    BadResponse(String),
}

impl fmt::Display for AnkiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnkiError::Offline => write!(f, "Anki offline (AnkiConnect unreachable)"),
            AnkiError::BadResponse(msg) => write!(f, "unexpected response from Anki: {msg}"),
        }
    }
}

impl std::error::Error for AnkiError {}

/// Port: read-only view of the user's Anki collection (ADR 0004).
///
/// Display-only in slice 0.1 — the due total never gates the queue.
pub trait AnkiGateway {
    /// Total number of cards currently due (`is:due`, all decks).
    fn due_total(&self) -> impl Future<Output = Result<u64, AnkiError>> + Send;
}
