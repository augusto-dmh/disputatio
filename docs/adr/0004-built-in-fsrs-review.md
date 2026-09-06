# 0004. Built-in FSRS review; Anki demoted to interim source

- Status: Accepted
- Date: 2026-09-06

## Context

Anki currently holds the cards and the review history (pushed via AnkiConnect from the agent flow). Reviews happen exclusively at the desktop — the owner confirmed no mobile review habit — so Anki-mobile compatibility is not a requirement. FSRS is the modern scheduling standard with official implementations per language (rs-fsrs, ts-fsrs, …), and research was blunt about the tar pit: AnkiConnect exposes no FSRS memory state, so two-way sync with Anki is lossy and fragile. Owning the review UI is also a precondition for the narratio and disputatio stages, which need the loop in one place.

## Decision

1. disputatio owns scheduling and review: **rs-fsrs in the Rust backend**, colocated with the database (ADR 0003).
2. AnkiConnect is used only as an **interim due-count reader** (slice 0.1 queue) and as an optional **one-time history import** (slice 0.2). Anki is never required at runtime after 0.2.
3. No `.apkg` export before 1.0, and only if a use case appears — desktop-only reviews removed the original motivation.
4. Whether to import Anki's review history (continuity of FSRS memory state) or start fresh is decided in slice 0.2 and recorded as an addendum here.

## Consequences

- The review UX is ours to build and tune — no borrowing Anki's.
- Review history continuity depends on the import decision; FSRS memory state can be seeded from review logs if imported.
- Cards keep the keep/kill curation gate (never auto-imported), now flowing straight into the internal scheduler instead of Anki.
