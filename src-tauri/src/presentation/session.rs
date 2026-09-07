//! Session commands (specs/queue-slice task 7) — translation only
//! (AGENTS.md layering): small DTOs in, one service call, small DTOs out.

use chrono::Utc;
use serde::Serialize;
use specta::Type;
use tauri::Manager;

use crate::domain::session::{OpenSession, Session, SessionRepo};
use crate::infrastructure::pg::Db;
use crate::infrastructure::session_pg::PgSessionRepo;

/// IPC shape of the study session. The DB `BIGINT` ids and the duration
/// narrow to 32-bit at this edge (specta forbids BigInt-style types; values
/// are tiny in this app).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SessionDto {
    pub session_id: i32,
    /// The chunk being studied (the open session's stage row names it).
    pub chunk_id: i32,
    /// `false` while the session is open.
    pub closed: bool,
    /// Logged seconds, present once the session is stopped.
    pub duration_seconds: Option<u32>,
}

impl From<OpenSession> for SessionDto {
    fn from(open: OpenSession) -> Self {
        SessionDto {
            session_id: i32::try_from(open.session.id).unwrap_or(i32::MAX),
            chunk_id: i32::try_from(open.chunk_id).unwrap_or(i32::MAX),
            closed: false,
            duration_seconds: None,
        }
    }
}

impl From<Session> for SessionDto {
    fn from(session: Session) -> Self {
        SessionDto {
            session_id: i32::try_from(session.id).unwrap_or(i32::MAX),
            // A closed session's chunk lives in its stage rows; callers of
            // the stop command already know which chunk they were on.
            chunk_id: 0,
            closed: true,
            duration_seconds: session
                .duration_seconds()
                .and_then(|s| u32::try_from(s).ok()),
        }
    }
}

/// Builds the repo from the managed pool; without it the app runs without
/// persistence (lib.rs degrades instead of crashing, ADR 0003).
fn repo(app: &tauri::AppHandle) -> Result<PgSessionRepo, String> {
    let db = app.try_state::<Db>().ok_or_else(|| {
        "database offline — sessions need the state store (start `make infra`)".to_string()
    })?;
    Ok(PgSessionRepo(db.0.clone()))
}

/// Opens a study session on the chunk: one session row + the study stage row
/// (migration 0001's `session_stages.stage` enum as-is — ADR 0005, Proposed,
/// may amend it). The chunk flips to `in_progress`; the queue surfaces it.
#[tauri::command]
#[specta::specta]
pub async fn start_session(app: tauri::AppHandle, chunk_id: i32) -> Result<SessionDto, String> {
    let repo = repo(&app)?;
    let open = crate::application::session::start_session(&repo, i64::from(chunk_id), Utc::now())
        .await
        .map_err(|e| e.to_string())?;
    Ok(SessionDto::from(open))
}

/// Stops the open session: records end time + duration and closes the stage
/// row. The chunk stays `in_progress` — it resurfaces in the queue.
#[tauri::command]
#[specta::specta]
pub async fn stop_session(app: tauri::AppHandle) -> Result<SessionDto, String> {
    let repo = repo(&app)?;
    let session = crate::application::session::stop_session(&repo, Utc::now())
        .await
        .map_err(|e| e.to_string())?;
    Ok(SessionDto::from(session))
}

/// The session still open, if any — so the UI can offer resume/stop after a
/// restart.
#[tauri::command]
#[specta::specta]
pub async fn get_active_session(app: tauri::AppHandle) -> Result<Option<SessionDto>, String> {
    let repo = repo(&app)?;
    let open = repo
        .open_session()
        .await
        .map_err(|e| e.to_string())?
        .map(SessionDto::from);
    Ok(open)
}

/// Marks the chunk complete (`done`); the next chunk in that track's order
/// becomes "next" (requirements.md EARS).
#[tauri::command]
#[specta::specta]
pub async fn complete_chunk(app: tauri::AppHandle, chunk_id: i32) -> Result<(), String> {
    let repo = repo(&app)?;
    crate::application::session::complete_chunk(&repo, i64::from(chunk_id), Utc::now())
        .await
        .map_err(|e| e.to_string())
}
