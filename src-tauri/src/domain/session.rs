//! Session entities and port (docs/architecture.md data model sketch,
//! migration 0001 `sessions`/`session_stages`). A session is the study
//! record: it opens on one chunk, logs its stage row, and closes with an
//! end time — no manual dashboard edits (specs/queue-slice/requirements.md).

use std::fmt;
use std::future::Future;

use chrono::{DateTime, Utc};

use super::repo::RepoError;

/// The canonical study stages, mirroring the `session_stages.stage` CHECK
/// constraint (migration 0001). ADR 0005 proposes the five-stage daily loop
/// and may amend this — the enum follows the migration as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Memoria,
    Lectio,
    Narratio,
    Disputatio,
    Compositio,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Memoria => "memoria",
            Stage::Lectio => "lectio",
            Stage::Narratio => "narratio",
            Stage::Disputatio => "disputatio",
            Stage::Compositio => "compositio",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "memoria" => Some(Stage::Memoria),
            "lectio" => Some(Stage::Lectio),
            "narratio" => Some(Stage::Narratio),
            "disputatio" => Some(Stage::Disputatio),
            "compositio" => Some(Stage::Compositio),
            _ => None,
        }
    }
}

/// The study record: one sitting with one chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    /// Set when the session closes; `None` while it is open.
    pub ended_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Logged duration once the session is closed (requirements.md: stop
    /// records duration).
    pub fn duration_seconds(&self) -> Option<i64> {
        Some((self.ended_at? - self.started_at).num_seconds())
    }
}

/// The open session with the chunk it is studying — what the UI needs to
/// offer "resume/stop" after a restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSession {
    pub session: Session,
    /// The chunk behind the session's open study stage.
    pub chunk_id: i64,
}

#[derive(Debug)]
pub enum SessionError {
    Repo(RepoError),
    /// The caller asked for something the study loop forbids: starting a
    /// second session, stopping a closed one, completing a missing chunk.
    InvalidInput(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Repo(e) => write!(f, "{e}"),
            SessionError::InvalidInput(msg) => write!(f, "invalid session request: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// Port: writes for the study loop (ADR 0003 state store). The queue reads;
/// this port records what actually happened.
pub trait SessionRepo {
    /// Opens a session on `chunk_id` with its first study-stage row
    /// (`started_at = now`). The chunk must exist — sessions log against it.
    fn insert_open_session(
        &self,
        chunk_id: i64,
        stage: Stage,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<OpenSession, SessionError>> + Send;
    /// Closes the session (and its open stage row) at `now`.
    fn close_session(
        &self,
        session_id: i64,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Session, SessionError>> + Send;
    /// The session still open, if any. More than one open session is a
    /// state this slice never creates; the latest wins.
    fn open_session(
        &self,
    ) -> impl Future<Output = Result<Option<OpenSession>, SessionError>> + Send;
    /// Marks the chunk done — the next chunk in course order becomes "next"
    /// (requirements.md EARS).
    fn mark_chunk_done(
        &self,
        chunk_id: i64,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), SessionError>> + Send;
}
