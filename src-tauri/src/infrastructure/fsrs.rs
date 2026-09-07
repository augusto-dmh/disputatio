//! The [`Scheduler`] port implemented over rs-fsrs 1.2 (ADR 0004; the crate
//! is banned in `domain` by `fitness.py` — this file is its only home).
//! Default FSRS parameters, desired retention 0.90 = the library's
//! `request_retention` default (specs/memoria-slice/design.md).

use chrono::{DateTime, Utc};
use rs_fsrs::{Card as FsrsCard, Rating as FsrsRating, State as FsrsStateLib, FSRS};

#[cfg(test)]
use crate::domain::review::Curation;
use crate::domain::review::{
    Card, CardState, FsrsState, Grade, ReviewLog, ScheduledReview, Scheduler,
};

/// rs-fsrs adapter. One instance serves the whole app: the scheduler is
/// stateless across reviews (memory state lives on the card).
#[derive(Debug, Clone, Default)]
pub struct FsrsScheduler {
    fsrs: FSRS,
}

impl FsrsScheduler {
    /// Replay a chronological review history ((reviewed_at, grade) pairs)
    /// through the scheduler to reconstruct a card's memory state — the
    /// one-time Anki import's seeding path (ADR 0004 addendum draft).
    /// `None` for an empty history: the card imports as new.
    pub fn replay(&self, history: &[(DateTime<Utc>, Grade)]) -> Option<CardState> {
        let mut state: Option<CardState> = None;
        for &(at, grade) in history {
            let card = Self::to_fsrs_card(state.as_ref(), at);
            let info = self.fsrs.next(card, at, Self::to_fsrs_rating(grade));
            state = Some(Self::from_fsrs(info.card));
        }
        state
    }

    /// Domain card → rs-fsrs card. `None` state (never reviewed) maps to a
    /// `New` card; `now` anchors due/last_review so epoch zero never leaks
    /// into the algorithm.
    fn to_fsrs_card(state: Option<&CardState>, now: DateTime<Utc>) -> FsrsCard {
        match state {
            None => FsrsCard {
                due: now,
                last_review: now,
                ..FsrsCard::default()
            },
            Some(state) => FsrsCard {
                due: state.due,
                stability: state.stability,
                difficulty: state.difficulty,
                reps: state.reps,
                lapses: state.lapses,
                state: match state.state {
                    FsrsState::Learning => FsrsStateLib::Learning,
                    FsrsState::Review => FsrsStateLib::Review,
                    FsrsState::Relearning => FsrsStateLib::Relearning,
                },
                last_review: state.last_review_at,
                // rs-fsrs derives elapsed_days from last_review itself when
                // scheduling; the stored field is its own bookkeeping.
                elapsed_days: 0,
                scheduled_days: 0,
            },
        }
    }

    fn to_fsrs_rating(grade: Grade) -> FsrsRating {
        match grade {
            Grade::Again => FsrsRating::Again,
            Grade::Hard => FsrsRating::Hard,
            Grade::Good => FsrsRating::Good,
            Grade::Easy => FsrsRating::Easy,
        }
    }

    fn from_fsrs(card: FsrsCard) -> CardState {
        CardState {
            stability: card.stability,
            difficulty: card.difficulty,
            state: match card.state {
                FsrsStateLib::Learning => FsrsState::Learning,
                FsrsStateLib::Review => FsrsState::Review,
                // A review never leaves a card in New; treat it as Learning.
                FsrsStateLib::New => FsrsState::Learning,
                FsrsStateLib::Relearning => FsrsState::Relearning,
            },
            due: card.due,
            last_review_at: card.last_review,
            reps: card.reps,
            lapses: card.lapses,
        }
    }
}

impl Scheduler for FsrsScheduler {
    fn schedule(
        &self,
        card: &Card,
        grade: Grade,
        duration_ms: Option<i32>,
        now: DateTime<Utc>,
    ) -> ScheduledReview {
        let state_before = card.state;
        let fsrs_card = Self::to_fsrs_card(
            state_before
                .map(|state| CardState {
                    stability: card.stability.unwrap_or(0.0),
                    difficulty: card.difficulty.unwrap_or(0.0),
                    state,
                    due: card.due.unwrap_or(now),
                    last_review_at: card.last_review_at.unwrap_or(now),
                    reps: card.reps,
                    lapses: card.lapses,
                })
                .as_ref(),
            now,
        );
        let info = self.fsrs.next(fsrs_card, now, Self::to_fsrs_rating(grade));
        ScheduledReview {
            state: Self::from_fsrs(info.card),
            log: ReviewLog {
                grade,
                reviewed_at: now,
                state_before,
                elapsed_days: info.review_log.elapsed_days,
                scheduled_days: info.review_log.scheduled_days,
                duration_ms,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_card() -> Card {
        Card {
            id: 1,
            deck: Some("fundamentos".into()),
            front: "front".into(),
            back: "back".into(),
            curation: Curation::Kept,
            due: None,
            stability: None,
            difficulty: None,
            state: None,
            last_review_at: None,
            reps: 0,
            lapses: 0,
        }
    }

    #[test]
    fn first_good_review_seeds_memory_state_and_schedules_ahead() {
        let now = Utc::now();
        let review = FsrsScheduler::default().schedule(&new_card(), Grade::Good, Some(4200), now);

        assert_eq!(review.log.grade, Grade::Good);
        assert_eq!(review.log.state_before, None, "first review");
        assert_eq!(review.log.duration_ms, Some(4200));
        assert!(review.state.stability > 0.0, "memory state is born");
        assert!(review.state.difficulty > 0.0);
        assert_eq!(
            review.state.state,
            FsrsState::Learning,
            "short-term first step"
        );
        assert!(review.state.due > now, "scheduled into the future");
        assert_eq!(review.state.reps, 1);
    }

    #[test]
    fn grades_differ_and_again_lapses_a_mature_card() {
        let now = Utc::now();
        let scheduler = FsrsScheduler::default();

        let easy = scheduler.schedule(&new_card(), Grade::Easy, None, now);
        let hard = scheduler.schedule(&new_card(), Grade::Hard, None, now);
        assert!(
            easy.state.due > hard.state.due,
            "easy schedules further ahead than hard"
        );

        // A mature card failed: it lapses and comes back soon.
        let mature = CardState {
            stability: 30.0,
            difficulty: 5.0,
            state: FsrsState::Review,
            due: now,
            last_review_at: now - chrono::Duration::days(30),
            reps: 5,
            lapses: 0,
        };
        let card = Card {
            due: Some(mature.due),
            stability: Some(mature.stability),
            difficulty: Some(mature.difficulty),
            state: Some(mature.state),
            last_review_at: Some(mature.last_review_at),
            reps: mature.reps,
            lapses: mature.lapses,
            ..new_card()
        };
        let again = scheduler.schedule(&card, Grade::Again, None, now);
        assert_eq!(again.log.state_before, Some(FsrsState::Review));
        assert_eq!(again.state.lapses, 1, "a lapsed card counts its lapse");
        assert!(again.state.due < easy.state.due, "back soon, not far ahead");
    }

    #[test]
    fn replay_reconstructs_memory_state_from_history() {
        let now = Utc::now();
        let history = vec![
            (now - chrono::Duration::days(30), Grade::Again),
            (now - chrono::Duration::days(29), Grade::Hard),
            (now - chrono::Duration::days(20), Grade::Good),
            (now - chrono::Duration::days(2), Grade::Good),
        ];
        let state = FsrsScheduler::default()
            .replay(&history)
            .expect("history yields a state");

        assert_eq!(state.reps, 4);
        assert_eq!(
            state.lapses, 0,
            "Again on a learning card is not a lapse in rs-fsrs"
        );
        assert!(state.stability > 0.0);
        assert_eq!(state.state, FsrsState::Review, "mature after the replay");

        assert_eq!(
            FsrsScheduler::default().replay(&[]),
            None,
            "no history = new"
        );
    }
}
