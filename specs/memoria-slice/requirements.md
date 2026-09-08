# requirements — memoria-slice (slice 0.2)

## Context

Slice 0.2 of the roadmap (PROJECT_BRIEF §Roadmap): reviews move into the app. `rs-fsrs` owns scheduling in the Rust backend (ADR 0004, *Accepted*); cards enter through the one-time Anki import (they were already curated by the keep/kill gate); the queue's due header switches from the AnkiConnect reader to the internal due state. Card authoring is NOT here — narratio drafts arrive in 0.3. Lane 2: new slice, cross-boundary (migration + `src-tauri` + `src/`).

## Expectations

(Written before building, per TLC class 22.)

- Reviews run entirely in-app: from the Home screen the user takes a due card, sees the front, reveals the back, grades again/hard/good/easy, and the card's next due date moves accordingly — with Anki closed the whole time.
- A grade persists in one transaction: the card's FSRS state and one review row land in Postgres; the same grade applied twice (stale UI, retry) does not double-apply.
- The queue header shows the internal due count (kept cards with due ≤ now). After this slice no queue path calls AnkiConnect; the app answers "what do I study now" with Anki uninstalled.
- An empty cards table degrades gracefully: due count 0, "nothing due" in the review block, queue unchanged (ADR 0003 degradation discipline).
- If the owner approves the history import (ADR 0004 addendum, draft attached to the spec PR): after import, imported cards carry reconstructed memory state from their Anki review history — not reset-to-new — and re-running the import duplicates nothing (idempotent on stable Anki ids).
- Review↔session linkage is explicitly out of scope: `session_stages` rows for memoria wait for ADR 0005 acceptance (the stage model is still *Proposed*).

## Requirements (EARS)

- WHEN the queue loads THE SYSTEM SHALL display the internal due count (cards with curation `kept` and due ≤ now); no AnkiConnect call is made anywhere in the queue path.
- WHEN the user opens the review flow THE SYSTEM SHALL fetch a bounded batch of due cards (limit parameter, default 20, oldest due first).
- WHEN the user grades a card THE SYSTEM SHALL compute the next state through the scheduler port (rs-fsrs adapter, desired retention 0.90) and persist the card update plus the review row in a single transaction, returning the scheduled next due.
- WHEN nothing is due or the cards table is empty THE SYSTEM SHALL show the "nothing due" state and the queue continues normally.
- WHEN a grade targets a card that is gone or not `kept` THE SYSTEM SHALL return a typed error, not a crash.
- WHEN the Anki import runs (owner decision permitting) THE SYSTEM SHALL import cards (front, back, deck, `anki_card_id` for idempotency, curation `kept`) and — if history import is approved — replay each card's Anki review history through the scheduler to reconstruct its memory state in one honest transaction; WHEN AnkiConnect is unreachable THE SYSTEM SHALL fail with a clear message and no partial import.
- IF the database is offline THE SYSTEM SHALL degrade per ADR 0003: the queue and review flows say the state store is needed instead of crashing.

## Done when

(Verification commands + observable outcomes that close this spec. `make check` is always implied.)

- [ ] `make check` passes
- [ ] testcontainers integration test: migration 0002 applies; grade round-trip (card state + review row), due query ordering, and (if approved) idempotent import verified against real Postgres
- [ ] Manual smoke: infra up → import from running Anki (or seeded rows) → review a card end-to-end in the GUI with Anki closed → header reflects internal due count; screenshots in `docs/screenshots/`
- [ ] Owner decision recorded as the ADR 0004 addendum (import vs start fresh) — required before the import task merges
- [ ] All expectations above verified and recorded in the PRs
