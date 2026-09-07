//! Session service (specs/queue-slice/design.md: `session.rs` — start/stop/
//! complete): the study-loop rules over the [`SessionRepo`] port. Pure
//! composition — the adapter owns the world.

use chrono::{DateTime, Utc};

use crate::domain::session::{OpenSession, Session, SessionError, SessionRepo, Stage};

/// The stage a queue-driven study session opens with: ingesting one next
/// chunk (ADR 0005's lectio — Proposed; migration 0001's enum is followed
/// as-is and may be amended by that ADR).
pub const STUDY_STAGE: Stage = Stage::Lectio;

/// Starts a session on `chunk_id`: one session row, one study-stage row, and
/// the chunk flips to `in_progress` so the queue surfaces it as the frontier.
/// One session at a time — a second start is rejected, not silently merged.
pub async fn start_session<R: SessionRepo>(
    repo: &R,
    chunk_id: i64,
    now: DateTime<Utc>,
) -> Result<OpenSession, SessionError> {
    if repo.open_session().await?.is_some() {
        return Err(SessionError::InvalidInput(
            "a session is already open — stop it first".to_string(),
        ));
    }
    repo.insert_open_session(chunk_id, STUDY_STAGE, now).await
}

/// Stops the open session: closes the session row and its stage row at `now`
/// (duration becomes derivable), the chunk stays `in_progress` — mid-way
/// work resurfaces in the queue, and goes stale after 14 days.
pub async fn stop_session<R: SessionRepo>(
    repo: &R,
    now: DateTime<Utc>,
) -> Result<Session, SessionError> {
    let open = repo
        .open_session()
        .await?
        .ok_or_else(|| SessionError::InvalidInput("no session is open".to_string()))?;
    repo.close_session(open.session.id, now).await
}

/// Marks the chunk done; the queue's next-in-order rule takes over from
/// there (requirements.md EARS).
pub async fn complete_chunk<R: SessionRepo>(
    repo: &R,
    chunk_id: i64,
    now: DateTime<Utc>,
) -> Result<(), SessionError> {
    repo.mark_chunk_done(chunk_id, now).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeRepo {
        open: Mutex<Option<OpenSession>>,
        done: Mutex<Vec<i64>>,
        closed: Mutex<Vec<i64>>,
        fail_done: bool,
    }

    impl FakeRepo {
        fn empty() -> Self {
            Self {
                open: Mutex::new(None),
                done: Mutex::new(Vec::new()),
                closed: Mutex::new(Vec::new()),
                fail_done: false,
            }
        }

        fn with_open(chunk_id: i64) -> Self {
            let repo = Self::empty();
            *repo.open.lock().unwrap() = Some(OpenSession {
                session: Session {
                    id: 7,
                    started_at: Utc::now() - chrono::Duration::minutes(25),
                    ended_at: None,
                },
                chunk_id,
            });
            repo
        }
    }

    impl SessionRepo for FakeRepo {
        async fn insert_open_session(
            &self,
            chunk_id: i64,
            stage: Stage,
            now: DateTime<Utc>,
        ) -> Result<OpenSession, SessionError> {
            assert_eq!(stage, STUDY_STAGE, "queue sessions open on the study stage");
            let open = OpenSession {
                session: Session {
                    id: 42,
                    started_at: now,
                    ended_at: None,
                },
                chunk_id,
            };
            *self.open.lock().unwrap() = Some(open.clone());
            Ok(open)
        }

        async fn close_session(
            &self,
            session_id: i64,
            now: DateTime<Utc>,
        ) -> Result<Session, SessionError> {
            self.closed.lock().unwrap().push(session_id);
            *self.open.lock().unwrap() = None;
            Ok(Session {
                id: session_id,
                started_at: now - chrono::Duration::minutes(25),
                ended_at: Some(now),
            })
        }

        async fn open_session(&self) -> Result<Option<OpenSession>, SessionError> {
            Ok(self.open.lock().unwrap().clone())
        }

        async fn mark_chunk_done(
            &self,
            chunk_id: i64,
            _now: DateTime<Utc>,
        ) -> Result<(), SessionError> {
            if self.fail_done {
                return Err(SessionError::InvalidInput("chunk not found".into()));
            }
            self.done.lock().unwrap().push(chunk_id);
            Ok(())
        }
    }

    #[tokio::test]
    async fn start_creates_the_session_with_the_study_stage() {
        let repo = FakeRepo::empty();
        let open = start_session(&repo, 5, Utc::now())
            .await
            .expect("session starts");
        assert_eq!(open.chunk_id, 5);
        assert_eq!(open.session.id, 42);
        assert!(repo.open.lock().unwrap().is_some());
    }

    #[tokio::test]
    async fn a_second_start_is_rejected_while_one_session_is_open() {
        let repo = FakeRepo::with_open(5);
        let err = start_session(&repo, 6, Utc::now())
            .await
            .expect_err("one session at a time");
        assert!(matches!(err, SessionError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn stop_closes_the_open_session_and_reports_its_duration() {
        let repo = FakeRepo::with_open(5);
        let session = stop_session(&repo, Utc::now())
            .await
            .expect("session stops");
        assert_eq!(session.id, 7);
        assert!(session.duration_seconds().is_some());
        assert_eq!(*repo.closed.lock().unwrap(), vec![7]);
        assert!(
            repo.open.lock().unwrap().is_none(),
            "the open slot is freed"
        );
    }

    #[tokio::test]
    async fn stop_without_an_open_session_is_rejected() {
        let repo = FakeRepo::empty();
        let err = stop_session(&repo, Utc::now())
            .await
            .expect_err("nothing to stop");
        assert!(matches!(err, SessionError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn complete_marks_the_chunk_done_and_surfaces_failures() {
        let repo = FakeRepo::empty();
        complete_chunk(&repo, 9, Utc::now())
            .await
            .expect("chunk completes");
        assert_eq!(*repo.done.lock().unwrap(), vec![9]);

        let mut broken = FakeRepo::empty();
        broken.fail_done = true;
        let err = complete_chunk(&broken, 999, Utc::now())
            .await
            .expect_err("unknown chunk surfaces");
        assert!(matches!(err, SessionError::InvalidInput(_)));
    }
}
