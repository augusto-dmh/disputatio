# Study systems landscape (2026-09-06)

Research dossier behind the v1 definition (see `docs/PROJECT_BRIEF.md`, ADRs 0003-0005). Headline finding: the queue is the product — every serious system converges on removing the "what do I study now" decision, and no existing tool composes the whole loop.

## Taxonomy

Functions every serious study system needs: capture → process/extract → schedule → review/rehearse → assess → log/reflect → decide-what-next. Most tools cover one or two; none compose all seven.

## What exists

- **SuperMemo** — the only full-pipeline system: import everything, assign priority 0-100%, and the outstanding queue interleaves reading topics with clozes, eliminating the "what next" decision and neutralizing priority bias. Closed Windows app, proprietary database, hostile UX, no markdown or AI integration. https://help.supermemo.org/wiki/Priority_queue
- **Anki + FSRS + AnkiConnect** — best-in-class scheduling and assessment. FSRS is built into Anki 23.10+; embeddable implementations exist per language (open-spaced-repetition). AnkiConnect exposes HTTP on port 8765 (addNote, findCards, answerCards, setDueDate, insertReviews) but no FSRS memory state — scheduling truth stays locked in collection.db. https://git.sr.ht/~foosoft/anki-connect · https://github.com/open-spaced-repetition
- **Obsidian ecosystem** — Spaced Repetition plugin (FSRS and SM-2, deck-by-tag, whole-note review) but no ingest pipeline; Incremental Writing ports SuperMemo queues to notes but is unmaintained; Incremental Reading Toolkit is the most harness-like (topics, extracts, priority-aware queue, progress-aware scheduling, three card backends) but young and desktop-only. https://github.com/st3v3nmw/obsidian-spaced-repetition · https://github.com/bjsi/incremental-writing · https://community.obsidian.md/plugins/incremental-reading-toolkit
- **Andy Matuschak's line of work** — mnemonic medium (prompts embedded in the book, review in context, zero separate practice), Orbit, Quantum Country, "How to write good prompts" (prompts create understanding, not trivia), evergreen notes. https://andymatuschak.org · https://quantum.country · https://withorbit.com
- **AI-native tools (2024-2026)** — converged pattern: upload source → AI generates notes/cards/quizzes → built-in SRS (Knowt, AnkiDecks, NotebookLM flashcards, GPT-based Anki addons). Recurring critique: generated cards are low quality and content gets lost — generation works, curation does not. https://knowt.com/flashcards · https://www.remnote.com/blog/best-flashcard-maker

## Source-of-truth patterns

Markdown-files-as-truth (portable, greppable, git-friendly; scheduling metadata is the weak spot) · tool's own database (powerful scheduling, siloed, exit means losing history) · bridge markdown → AnkiConnect (creation works, sync-back one-way and fragile) · emerging hybrid: markdown canonical for knowledge, local database for mutable scheduling state, joined by stable UIDs — never let the vault and the scheduler fight.

## Gaps that justify a custom app

1. No tool unifies decide-what-next across courses, notes, and cards. 2. Scheduling truth is split between a card database and plugin state. 3. Course-position awareness (progress-aware scheduling from remaining material) exists only in one young plugin. 4. AI is bolt-on everywhere: generation without queueing, prioritization, or curation-in-the-loop. 5. Session logging is scattered with no reflective view.

## Design lessons adopted

The queue is the product; interleave acquisition and retention (review extracts before the next segment); split the source of truth (markdown canonical, database for state); review in context (cards anchored to their source); AI generates drafts, humans curate; log every session and let the logs re-rank the queue.
