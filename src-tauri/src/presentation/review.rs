//! Review commands (specs/memoria-slice tasks.md item 4) — translation only
//! (AGENTS.md layering): small DTOs in, one service call, small DTOs out.
//! The batch is bounded in Rust; only the fetched cards cross IPC (payload
//! rule). Grades travel as wire strings and parse into the domain enum at
//! this edge — the domain stays free of specta.

use chrono::Utc;
use serde::Serialize;
use specta::Type;
use tauri::Manager;

use crate::domain::review::{Card, FsrsState, Grade};
use crate::infrastructure::card_pg::PgCardRepo;
use crate::infrastructure::fsrs::FsrsScheduler;
use crate::infrastructure::pg::Db;

/// Default and ceiling for one review batch (requirements.md EARS: bounded).
const DEFAULT_BATCH: i64 = 20;
const MAX_BATCH: i64 = 100;

/// IPC shape of a card. The DB `BIGINT` id narrows to 32-bit at this edge
/// (specta forbids BigInt-style types; ids are tiny in this app); the due
/// date crosses as RFC 3339 — the UI only displays it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct CardDto {
    pub id: i32,
    pub deck: Option<String>,
    pub front: String,
    pub back: String,
    /// RFC 3339; `null` only for a card that never entered scheduling.
    pub due: Option<String>,
    /// `"learning" | "review" | "relearning"`; `null` = never reviewed (new).
    pub state: Option<String>,
    pub reps: i32,
}

impl From<Card> for CardDto {
    fn from(card: Card) -> Self {
        CardDto {
            id: i32::try_from(card.id).unwrap_or(i32::MAX),
            deck: card.deck,
            front: card.front,
            back: card.back,
            due: card.due.map(|d| d.to_rfc3339()),
            state: card
                .state
                .map(|s| match s {
                    FsrsState::Learning => "learning",
                    FsrsState::Review => "review",
                    FsrsState::Relearning => "relearning",
                })
                .map(str::to_string),
            reps: card.reps,
        }
    }
}

/// Builds the repo from the managed pool; without it the app runs without
/// persistence (lib.rs degrades instead of crashing, ADR 0003).
fn repo(app: &tauri::AppHandle) -> Result<PgCardRepo, String> {
    let db = app.try_state::<Db>().ok_or_else(|| {
        "database offline — reviews need the state store (start `make infra`)".to_string()
    })?;
    Ok(PgCardRepo(db.0.clone()))
}

/// The review batch: kept cards due now, oldest first. The UI shows "nothing
/// due" for an empty batch — an empty cards table is a normal state, not an
/// error (requirements.md EARS).
#[tauri::command]
#[specta::specta]
pub async fn list_due_cards(
    app: tauri::AppHandle,
    limit: Option<i32>,
) -> Result<Vec<CardDto>, String> {
    let repo = repo(&app)?;
    let limit = limit
        .unwrap_or(DEFAULT_BATCH as i32)
        .clamp(1, MAX_BATCH as i32) as i64;
    let cards = crate::application::review::due_cards(&repo, limit, Utc::now())
        .await
        .map_err(|e| e.to_string())?;
    Ok(cards.into_iter().map(CardDto::from).collect())
}

/// Grades a card: schedules via FSRS, persists card state + review row in
/// one transaction, returns the card as it now reads (the UI shows the next
/// due). Unknown grade spellings are rejected here, not in the domain.
#[tauri::command]
#[specta::specta]
pub async fn grade_card(
    app: tauri::AppHandle,
    card_id: i32,
    grade: String,
    duration_ms: Option<i32>,
) -> Result<CardDto, String> {
    let parsed = Grade::parse(&grade)
        .ok_or_else(|| format!("unknown grade {grade:?} (again | hard | good | easy)"))?;
    let repo = repo(&app)?;
    let card = crate::application::review::grade_card(
        &repo,
        &FsrsScheduler::default(),
        i64::from(card_id),
        parsed,
        duration_ms,
        chrono::Utc::now(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(CardDto::from(card))
}
