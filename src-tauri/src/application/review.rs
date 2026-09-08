//! Review service (specs/memoria-slice/tasks.md item 4): compose the card
//! repo with the scheduler port. Pure composition — batch reads come from
//! the repo, one grade flows scheduler → single-transaction write.

use std::fmt;

use chrono::{DateTime, Utc};

use crate::domain::repo::RepoError;
use crate::domain::review::{Card, CardRepo, Curation, Grade, Scheduler};

#[derive(Debug)]
pub enum ReviewError {
    Repo(RepoError),
    /// The id is gone — stale UI, or the card was imported over.
    NotFound,
    /// The card exists but is not `kept`: curation is the gate
    /// (requirements.md EARS: grades on draft/killed cards are refused).
    NotReviewable,
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewError::Repo(e) => write!(f, "{e}"),
            ReviewError::NotFound => write!(f, "card not found"),
            ReviewError::NotReviewable => {
                write!(f, "card is not reviewable (curation is not kept)")
            }
        }
    }
}

impl std::error::Error for ReviewError {}

/// The batch of due cards for one review sitting (requirements.md EARS:
/// bounded, oldest due first — the repo's SQL implements [`crate::domain::
/// review::due_batch`]).
pub async fn due_cards<C: CardRepo>(
    repo: &C,
    limit: i64,
    now: DateTime<Utc>,
) -> Result<Vec<Card>, ReviewError> {
    repo.due_cards(limit, now).await.map_err(ReviewError::Repo)
}

/// Grade one card: schedule the next state, persist it with the review row
/// in a single transaction, return the card as it now reads. Refuses
/// missing or non-`kept` cards honestly.
pub async fn grade_card<C: CardRepo, S: Scheduler>(
    repo: &C,
    scheduler: &S,
    card_id: i64,
    grade: Grade,
    duration_ms: Option<i32>,
    now: DateTime<Utc>,
) -> Result<Card, ReviewError> {
    let card = repo
        .get(card_id)
        .await
        .map_err(ReviewError::Repo)?
        .ok_or(ReviewError::NotFound)?;
    if card.curation != Curation::Kept {
        return Err(ReviewError::NotReviewable);
    }
    let review = scheduler.schedule(&card, grade, duration_ms, now);
    repo.apply_review(card.id, &review.state, &review.log)
        .await
        .map_err(ReviewError::Repo)?;
    Ok(Card {
        due: Some(review.state.due),
        stability: Some(review.state.stability),
        difficulty: Some(review.state.difficulty),
        state: Some(review.state.state),
        last_review_at: Some(review.state.last_review_at),
        reps: review.state.reps,
        lapses: review.state.lapses,
        ..card
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::review::{CardState, Curation, FsrsState, ReviewLog as DomainReviewLog};

    struct FakeRepo {
        cards: Vec<Card>,
        applied: std::sync::Mutex<Vec<(i64, CardState)>>,
    }

    impl FakeRepo {
        fn with(cards: Vec<Card>) -> Self {
            Self {
                cards,
                applied: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl CardRepo for FakeRepo {
        async fn due_count(&self, _now: DateTime<Utc>) -> Result<u64, RepoError> {
            Ok(self.cards.len() as u64)
        }
        async fn due_cards(
            &self,
            _limit: i64,
            _now: DateTime<Utc>,
        ) -> Result<Vec<Card>, RepoError> {
            Ok(self.cards.clone())
        }
        async fn get(&self, id: i64) -> Result<Option<Card>, RepoError> {
            Ok(self.cards.iter().find(|c| c.id == id).cloned())
        }
        async fn apply_review(
            &self,
            card_id: i64,
            state: &CardState,
            _log: &DomainReviewLog,
        ) -> Result<(), RepoError> {
            self.applied.lock().unwrap().push((card_id, state.clone()));
            Ok(())
        }
    }

    /// Deterministic scheduler double: the state moves, the grade is logged.
    struct FakeScheduler;

    impl Scheduler for FakeScheduler {
        fn schedule(
            &self,
            card: &Card,
            grade: Grade,
            duration_ms: Option<i32>,
            now: DateTime<Utc>,
        ) -> crate::domain::review::ScheduledReview {
            crate::domain::review::ScheduledReview {
                state: CardState {
                    stability: 1.0,
                    difficulty: 5.0,
                    state: FsrsState::Review,
                    due: now + chrono::Duration::days(i64::from(grade.as_rating())),
                    last_review_at: now,
                    reps: card.reps + 1,
                    lapses: card.lapses,
                },
                log: DomainReviewLog {
                    grade,
                    reviewed_at: now,
                    state_before: card.state,
                    elapsed_days: 0,
                    scheduled_days: 1,
                    duration_ms,
                },
            }
        }
    }

    fn kept_card(id: i64) -> Card {
        Card {
            id,
            deck: Some("fundamentos".into()),
            front: format!("front {id}"),
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

    #[tokio::test]
    async fn grading_persists_the_scheduled_state_and_returns_the_updated_card() {
        let now = Utc::now();
        let repo = FakeRepo::with(vec![kept_card(7)]);
        let card = grade_card(&repo, &FakeScheduler, 7, Grade::Good, Some(3210), now)
            .await
            .expect("grade");

        assert_eq!(card.reps, 1);
        assert_eq!(card.state, Some(FsrsState::Review));
        assert!(card.due.is_some_and(|d| d > now), "scheduled ahead");
        let applied = repo.applied.lock().unwrap();
        assert_eq!(applied.len(), 1, "one transactional write");
        assert_eq!(applied[0].0, 7);
    }

    #[tokio::test]
    async fn grades_refuse_missing_and_non_kept_cards() {
        let now = Utc::now();
        let mut killed = kept_card(2);
        killed.curation = Curation::Killed;
        let mut draft = kept_card(3);
        draft.curation = Curation::Draft;
        let repo = FakeRepo::with(vec![killed, draft]);

        let err = grade_card(&repo, &FakeScheduler, 1, Grade::Good, None, now)
            .await
            .expect_err("missing card");
        assert!(matches!(err, ReviewError::NotFound));

        let err = grade_card(&repo, &FakeScheduler, 2, Grade::Good, None, now)
            .await
            .expect_err("killed card");
        assert!(matches!(err, ReviewError::NotReviewable));

        let err = grade_card(&repo, &FakeScheduler, 3, Grade::Good, None, now)
            .await
            .expect_err("draft card");
        assert!(matches!(err, ReviewError::NotReviewable));
    }

    #[tokio::test]
    async fn repo_failures_surface_honestly() {
        struct FailingRepo;
        impl CardRepo for FailingRepo {
            async fn due_count(&self, _: DateTime<Utc>) -> Result<u64, RepoError> {
                Err(RepoError::Database("down".into()))
            }
            async fn due_cards(&self, _: i64, _: DateTime<Utc>) -> Result<Vec<Card>, RepoError> {
                Err(RepoError::Database("down".into()))
            }
            async fn get(&self, _: i64) -> Result<Option<Card>, RepoError> {
                Err(RepoError::Database("down".into()))
            }
            async fn apply_review(
                &self,
                _: i64,
                _: &CardState,
                _: &DomainReviewLog,
            ) -> Result<(), RepoError> {
                Err(RepoError::Database("down".into()))
            }
        }

        let err = due_cards(&FailingRepo, 10, Utc::now())
            .await
            .expect_err("repo failure surfaces");
        assert!(matches!(err, ReviewError::Repo(_)));

        let err = grade_card(
            &FailingRepo,
            &FakeScheduler,
            1,
            Grade::Good,
            None,
            Utc::now(),
        )
        .await
        .expect_err("repo failure surfaces");
        assert!(matches!(err, ReviewError::Repo(_)));
    }
}
