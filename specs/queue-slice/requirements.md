# requirements — queue-slice (slice 0.1)

## Context

First code slice: the app exists, runs on Windows, and its only screen — the Daily Queue — answers "what do I study now" from the vault courses, the video folder, and Anki. Everything else (reviews, narratio, disputatio) comes later; this slice delivers the pain-killer decided in the project brief.

## Expectations

(Written before building, per TLC class 22.)

- After `make infra` + `make dev`, the app opens on Windows and shows the queue with zero console errors.
- The queue covers all three v1 sources: fundamentos-enterprise (64 classes), system-design (16 classes), courses/videos (one video = one chunk).
- A queue answer never requires the user to think about ordering: next-in-order per track, stale items marked, Anki as one total.
- Indexing the real vault (~80 classes + videos) completes without errors and is idempotent (re-index doesn't duplicate).
- A session can be started and stopped; the study record (start, stop, duration, chunk) lands in Postgres without any manual dashboard edits.

## Requirements (EARS)

- WHEN the app opens THE SYSTEM SHALL display the daily queue: for each track the next chunk in course order, stale items flagged ahead of new ones, and the Anki due total in the header.
- WHEN a track has an in-progress chunk untouched for 14+ days THE SYSTEM SHALL surface it as stale, ahead of that track's new chunks.
- WHEN AnkiConnect (127.0.0.1:8765) is reachable THE SYSTEM SHALL display the total due count; WHEN it is unreachable THE SYSTEM SHALL show "Anki offline" and continue normally.
- WHEN the user presses start THE SYSTEM SHALL create a session row (started_at) and the study stage.
- WHEN the user presses stop THE SYSTEM SHALL close the session and record duration and the chunk studied.
- WHEN the user marks a chunk complete THE SYSTEM SHALL set its status to done; the next chunk in that track's order becomes "next".
- IF the vault path is changed in settings THE SYSTEM SHALL re-index from the new path on demand.
- WHEN seeding the first index THE SYSTEM SHALL derive per-track position from existing session `scope.md` files, with a manual "set current position" override in settings.

## Done when

- [ ] `make check` passes (fitness + clippy + biome + tsc + tests once activated)
- [ ] Manual smoke on Windows: infra up → index real vault → queue shows correct next class per track, one video chunk, Anki total (or offline note) → start/stop writes a session row visible via SQL
- [ ] Re-index run twice produces identical chunk counts
- [ ] All expectations above verified and recorded in the PR
