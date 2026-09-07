//! The memoria model (specs/memoria-slice/design.md): cards under FSRS
//! scheduling, the grading vocabulary, and the [`Scheduler`] port. Pure —
//! the port's implementation lives in `infrastructure` (rs-fsrs, banned
//! here by `fitness.py`), services in `application` compose it.

use std::future::Future;

use chrono::{DateTime, Utc};

use super::repo::RepoError;

/// The four FSRS ratings (reviews.grade stores 1–4, migration 0002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    Again,
    Hard,
    Good,
    Easy,
}

impl Grade {
    /// Wire/DB spelling (`grade_card` receives this string over IPC).
    pub fn as_str(&self) -> &'static str {
        match self {
            Grade::Again => "again",
            Grade::Hard => "hard",
            Grade::Good => "good",
            Grade::Easy => "easy",
        }
    }

    /// The numeric FSRS rating (reviews.grade, 1=again … 4=easy).
    pub fn as_rating(&self) -> i32 {
        match self {
            Grade::Again => 1,
            Grade::Hard => 2,
            Grade::Good => 3,
            Grade::Easy => 4,
        }
    }

    pub fn parse(s: &str) -> Option<Grade> {
        match s {
            "again" => Some(Grade::Again),
            "hard" => Some(Grade::Hard),
            "good" => Some(Grade::Good),
            "easy" => Some(Grade::Easy),
            _ => None,
        }
    }
}

/// The three FSRS learning states (cards.fsrs_state, migration 0002).
/// A card with `None` has never been reviewed — new.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsrsState {
    Learning,
    Review,
    Relearning,
}

impl FsrsState {
    pub fn as_str(&self) -> &'static str {
        match self {
            FsrsState::Learning => "learning",
            FsrsState::Review => "review",
            FsrsState::Relearning => "relearning",
        }
    }

    pub fn parse(s: &str) -> Option<FsrsState> {
        match s {
            "learning" => Some(FsrsState::Learning),
            "review" => Some(FsrsState::Review),
            "relearning" => Some(FsrsState::Relearning),
            _ => None,
        }
    }
}

/// The curation gate (cards.curation, migration 0002): only `kept` cards
/// review. Imports arrive pre-curated; narratio drafts (0.3) start here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curation {
    Draft,
    Kept,
    Killed,
}

impl Curation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Curation::Draft => "draft",
            Curation::Kept => "kept",
            Curation::Killed => "killed",
        }
    }

    pub fn parse(s: &str) -> Option<Curation> {
        match s {
            "draft" => Some(Curation::Draft),
            "kept" => Some(Curation::Kept),
            "killed" => Some(Curation::Killed),
            _ => None,
        }
    }
}

/// A card as the review flow sees it. `None` scheduling fields mean the
/// card was never reviewed (new); the repo's WHERE clauses filter batches
/// to `kept`, while the field rides along so a direct fetch can refuse
/// grades on non-reviewable cards (requirements.md EARS).
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub id: i64,
    pub deck: Option<String>,
    pub front: String,
    pub back: String,
    pub curation: Curation,
    pub due: Option<DateTime<Utc>>,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub state: Option<FsrsState>,
    pub last_review_at: Option<DateTime<Utc>>,
    pub reps: i32,
    pub lapses: i32,
}

/// The scheduling state a [`Scheduler`] produces for one grade — everything
/// the review write persists back onto the card (migration 0002 columns).
#[derive(Debug, Clone, PartialEq)]
pub struct CardState {
    pub stability: f64,
    pub difficulty: f64,
    pub state: FsrsState,
    pub due: DateTime<Utc>,
    pub last_review_at: DateTime<Utc>,
    pub reps: i32,
    pub lapses: i32,
}

/// One graded review — appended to `reviews` in the same write as the new
/// [`CardState`] (single transaction, requirements.md EARS).
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewLog {
    pub grade: Grade,
    pub reviewed_at: DateTime<Utc>,
    /// The card's FSRS state before this review; `None` on first review.
    pub state_before: Option<FsrsState>,
    pub elapsed_days: i64,
    pub scheduled_days: i64,
    pub duration_ms: Option<i32>,
}

/// What one grade yields: the card's next state plus the log row.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledReview {
    pub state: CardState,
    pub log: ReviewLog,
}

/// Port: the FSRS scheduler. Pure and synchronous — no IO, no async; the
/// adapter (`infrastructure/fsrs.rs`, rs-fsrs) owns the algorithm and its
/// parameters (default weights, desired retention 0.90 — design.md).
pub trait Scheduler {
    fn schedule(
        &self,
        card: &Card,
        grade: Grade,
        duration_ms: Option<i32>,
        now: DateTime<Utc>,
    ) -> ScheduledReview;
}

/// A card is due when its due date has arrived (`due <= now`). Curation is
/// the repo's filter; this is the rule the SQL implements and the
/// integration tests assert against real rows.
pub fn is_due(card: &Card, now: DateTime<Utc>) -> bool {
    card.due.is_some_and(|due| due <= now)
}

/// The review batch rule (requirements.md EARS): oldest due first, ties by
/// id for a stable order, bounded by the batch limit. The repo implements
/// it in SQL (`ORDER BY due, id LIMIT n`); this pure form is the contract
/// and the unit-test target.
pub fn due_batch(mut cards: Vec<Card>, limit: usize, now: DateTime<Utc>) -> Vec<Card> {
    cards.retain(|c| is_due(c, now));
    cards.sort_by_key(|c| (c.due, c.id));
    cards.truncate(limit);
    cards
}

/// Port: the review flow's read/write view over the state store (ADR 0003),
/// same shape as [`super::queue::QueueRepo`]. The SQL side filters curation
/// to `kept` — killed and draft cards never surface for review.
pub trait CardRepo {
    /// Kept cards with `due <= now` — the queue header's number.
    fn due_count(&self, now: DateTime<Utc>) -> impl Future<Output = Result<u64, RepoError>> + Send;
    /// The review batch: kept, due, oldest first ([`due_batch`] in SQL).
    fn due_cards(
        &self,
        limit: i64,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Card>, RepoError>> + Send;
    /// One card by id — `Ok(None)` when gone (stale UI, killed, imported over).
    fn get(&self, id: i64) -> impl Future<Output = Result<Option<Card>, RepoError>> + Send;
    /// Persist one grade: append the review row and move the card's FSRS
    /// state in a single transaction (requirements.md EARS).
    fn apply_review(
        &self,
        card_id: i64,
        state: &CardState,
        log: &ReviewLog,
    ) -> impl Future<Output = Result<(), RepoError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(due: Option<DateTime<Utc>>, id: i64) -> Card {
        Card {
            id,
            deck: Some("fundamentos".into()),
            front: format!("front {id}"),
            back: "back".into(),
            curation: Curation::Kept,
            due,
            stability: None,
            difficulty: None,
            state: None,
            last_review_at: None,
            reps: 0,
            lapses: 0,
        }
    }

    #[test]
    fn grade_vocabulary_round_trips_through_wire_spelling() {
        for grade in [Grade::Again, Grade::Hard, Grade::Good, Grade::Easy] {
            assert_eq!(Grade::parse(grade.as_str()), Some(grade));
        }
        assert_eq!(Grade::parse("bogus"), None);
        assert_eq!(
            [Grade::Again, Grade::Hard, Grade::Good, Grade::Easy].map(|g| g.as_rating()),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn fsrs_state_vocabulary_round_trips_through_db_spelling() {
        for state in [
            FsrsState::Learning,
            FsrsState::Review,
            FsrsState::Relearning,
        ] {
            assert_eq!(FsrsState::parse(state.as_str()), Some(state));
        }
        assert_eq!(FsrsState::parse("bogus"), None);
    }

    #[test]
    fn is_due_when_due_date_has_arrived() {
        let now = Utc::now();
        assert!(is_due(&card(Some(now), 1), now));
        assert!(is_due(
            &card(Some(now - chrono::Duration::hours(1)), 1),
            now
        ));
        assert!(!is_due(
            &card(Some(now + chrono::Duration::hours(1)), 1),
            now
        ));
        assert!(!is_due(&card(None, 1), now), "no due date is not due");
    }

    #[test]
    fn due_batch_orders_oldest_first_and_respects_the_limit() {
        let now = Utc::now();
        let cards = vec![
            card(Some(now - chrono::Duration::hours(2)), 3),
            card(Some(now + chrono::Duration::days(2)), 4),
            card(Some(now - chrono::Duration::hours(2)), 2),
            card(Some(now - chrono::Duration::hours(9)), 1),
        ];
        let batch = due_batch(cards, 2, now);
        let ids: Vec<i64> = batch.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![1, 2], "oldest due first, ties by id, capped");
    }
}
