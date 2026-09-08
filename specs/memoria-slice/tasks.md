# tasks — memoria-slice (slice 0.2)

Ordered; each task is one PR-able commit/branch. If this list exceeds ~8 tasks, split the feature.

1. [x] Migration 0002: `cards` + `reviews` (design.md draft — CHECKs, TIMESTAMPTZ, `anki_card_id` unique for import idempotency) + testcontainers `tests/review.rs` persistence test (apply, grade round-trip, due ordering)
2. [x] Domain: `domain/review.rs` — `ReviewCard`, `Grade`, review-log entry, `Scheduler` port, due rules (due ≤ now, kept only, oldest first); `fitness.py` bans `rs_fsrs` in domain; pure unit tests
3. [x] Infrastructure: `fsrs.rs` (`FsrsScheduler` over rs-fsrs 1.2 — direct dep, Cargo.lock committed; default parameters, retention 0.90, replay-based memory-state reconstruction) + `card_pg.rs` (runtime queries); integration tests
4. [x] Application + presentation: review service (batch, single-transaction grade) + `list_due_cards` / `grade_card` commands + regenerated `bindings.ts`; typed-error path for stale/absent card ids
5. [x] Queue due switch: `DailyQueue.due` from the internal count; `AnkiStatus` + `AnkiGateway` leave the queue path; header renders internal due; queue service + frontend tests updated
6. [x] Frontend reviewer: Home Memoria block — due header, "Review now", reveal + four grades, i/n progress, "nothing due" state; Vitest
7. [ ] Anki import (executes the ADR 0004 addendum once the owner decides; default plan assumes import approved): `findCards`/`cardsInfo`/`getReviewsOfCards` adapter, idempotent import service (replay → memory state), `import_anki` command + Settings button + done-flag. If start-fresh wins: delete `anki_connect.rs` + `AnkiGateway` + tests instead, and record that here
8. [ ] Close-out: verify every expectation in requirements.md; manual smoke (real DB, review with Anki closed; real-Anki import when running); screenshots → `docs/screenshots/`; full `make check`; results recorded in the PR
