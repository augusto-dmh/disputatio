# disputatio — Project Brief

> A local-first desktop study environment where the queue decides what you study, memory is scheduled by FSRS, understanding is earned by narration and disputation, and every session leaves a trace in Postgres.

Defined 2026-09-06 in a zero-based session ("orçamento base zero"): research subagents mapped the current agent-driven study workflow in the Obsidian vault, the learny sibling project, the study-tool landscape, the classical education canon (Aristotle, Aquinas, Sertillanges, Adler, Mason, the trivium, lectio divina, ars memoriae), and the desktop-stack options. Every load-bearing decision below was grilled and answered explicitly — nothing is copied from the current workflow just because it exists.

## Problem

Studying today runs through AI coding agents driving the Obsidian vault (the `/study` skill): ingest class → tutoring dialogue → harvest session → draft Anki cards → approve keep/kill in chat → import via AnkiConnect → regenerate Dashboard → branch and PR. The flow works, but the manual residue is pure cognitive load: deciding what to study, curating cards in a chat window, merging PRs for study records, maintaining a dashboard that only exists because agents are stateless, and reconstructing "what was I doing?" at the start of every session.

## Definition

The vault stays the source of truth for knowledge (markdown). PostgreSQL in a Docker volume holds the record: queue, sessions, scheduling state, review history. AI is a worker the app orchestrates, not a chat you live in. The core of the product is the **daily queue** — the screen that answers "what do I study now" so the human never burns cognition on meta-studying. Reviews are built-in FSRS; understanding is earned through voice narration and tutor disputation; sessions follow the classical sequence. Destination is a **full study environment** — studying happens inside the app — reached in shippable steps.

## Decisions (locked)

| # | Decision | Call |
|---|----------|------|
| 1 | Destination | Full study environment, built incrementally — PR-driven, every slice shippable, fakeflix-grade quality |
| 2 | Reviews | Built-in FSRS; the app owns scheduling; reviews are desktop-only, so no `.apkg` export before 1.0 |
| 3 | learny | Sibling apps, unconnected — shared ideas and structure, shared no code and no data |
| 4 | State store | PostgreSQL 16 in a Docker volume (explicitly chosen over SQLite) |
| 5 | Knowledge truth | Obsidian vault markdown; app reads the vault, writes only generated exports; `Dashboard.md` abolished — the queue screen replaces it |
| 6 | Narratio | Voice narration in scope: tell it back → ASR → fidelity scoring → omissions become card drafts → human curation |
| 7 | v1 queue sources | `fundamentos-enterprise` + `system-design` + existing Anki cards (interim, via AnkiConnect) + `courses/videos` |
| 8 | Identity | `github.com/augusto-dmh/disputatio` — the name is the method: the scholastic disputation the tutor embodies |

Still open: session shape ([ADR 0005](adr/0005-classical-session-model.md), *Proposed*), ASR engine ([ADR 0006](adr/0006-voice-narration.md)), whether to import Anki review history ([ADR 0004](adr/0004-built-in-fsrs-review.md)).

## Keep / kill ledger (zero-based outcomes)

**Keep — proven good, carried forward on merit:**
- Markdown class notes in the vault as knowledge truth, versioned in git.
- The tutoring dialogue with a hint ladder and withheld answers — now evidence-backed (PNAS 2025: unguarded AI answers hurt later exam performance by ~17%; a hint-only tutor avoided the harm).
- Human curation of AI-drafted cards (keep/kill) — generation without curation is the documented failure mode of AI study tools.
- learny's engineering discipline: hexagonal boundaries, ADRs, PRs per slice.

**Kill — agent-era workarounds and modern-tool violations:**
- `Dashboard.md` and the PR ceremony for study records (both exist because agents are stateless and need safety rails; an app with its own database needs neither). Vault PRs remain for knowledge changes only.
- Anki as the review surface. AnkiConnect is demoted to an interim due-count reader plus an optional one-time history import.
- Session-folder sprawl (five markdown files per session) → Postgres rows, with markdown export on demand.
- Novelty-ordered "what should I do next" → *ordo* (order of the subject matter) + FSRS due pressure.
- Recognition-based review and answers-first AI → designed out by narratio and disputatio.

## Classical grounding (why the loop looks like this)

- **The classical day is a queue**: lectio → meditatio → memoria → disputatio → compositio. No existing product composes these stages into one sequence; every tool does one stage. The composition is the niche.
- **Virtues are plural** (Aristotle): *episteme*, *techne*, *phronesis* are acquired by different activities — so the queue rotates recall, build tasks, and judgment scenarios. Not everything is a flashcard.
- **Disputatio precedes determinatio** (Aquinas): the learner argues objections before the settled answer is revealed.
- **Narration before notation** (Mason): recall in your own words after one attentive reading; notes are written only after recall.
- **Capture now, incubate later; rest is work** (Sertillanges): frictionless capture, reviews scheduled across sleep, sessions end before exhaustion.
- **Memoria feeds composition** (ars memoriae): every topic issues an artifact — an explainer, code, an argument. Notes exist to become works.

## Roadmap

Each slice is a shippable, PR-reviewable release. Nothing merges that can't be used.

### 0.1 — Queue
Tauri 2 skeleton (React + Vite + Tailwind frontend, thin Rust backend, hexagonal module layout). Docker-compose Postgres started and health-checked by the app. Vault indexer: `fundamentos-enterprise`, `system-design`, and a chunker for `courses/videos` into Postgres + full-text search. Daily Queue v0: *ordo* next-chunks + stale backlog + due counts read live from Anki via AnkiConnect. Start/Stop session button writing the first session rows.
*Accepts when:* the queue screen correctly answers "what now" from all four sources and a session can be logged.

### 0.2 — Memoria
Built-in FSRS review UI (`rs-fsrs`). Optional one-time import of Anki review history. Queue's due source switches from AnkiConnect to internal scheduling.
*Accepts when:* reviews run entirely in-app and the queue reflects internal due state.

### 0.3 — Narratio
Voice tell-back per chunk: MediaRecorder → ASR → LLM fidelity scoring against the source → omissions/confusions become card drafts → mandatory human keep/kill → into FSRS.
*Accepts when:* a narrated session produces curated cards with zero chat windows involved.

### 0.4 — Disputatio
In-app tutoring dialogue: argues against the learner's stated position before revealing the determinatio, hint ladder throughout, transcript and outcome logged to the session.
*Accepts when:* a full tutoring session happens natively with answers withheld by default.

### 1.0 — Compositio
Artifact stage (notes → works), stats and reflection (streaks, retention, time-on-task), queue re-ranked by retention + ordo, optional MCP server so coding agents can read and write the queue.
*Accepts when:* the loop closes: logs change future priorities without manual intervention.

## Non-goals

- No note editor — Obsidian remains the place knowledge is written.
- No web version, no cloud sync, no mobile client (reviews are desktop-only by decision).
- No connection to learny — siblings, not a suite.
- No auto-imported cards — curation is mandatory, forever.
- No gamification of the competitive kind (the debate-app failure mode).
