# disputatio

A local-first desktop study environment: a daily queue decides what you study, built-in FSRS owns your memory, voice narration makes you tell it back in your own words, and an AI tutor disputes your position before handing over answers.

Classically grounded (Aristotle, Aquinas, Sertillanges, Adler, Mason) and built as the study counterpart of a coding harness: the system decides what's next, captures what happened, and measures whether it's working — so the human spends cognition on studying, not on meta-studying.

Sibling to [learny](https://github.com/augusto-dmh/learny) (books, web). They share ideas and structure deliberately; they share no code and no data.

## Status

Pre-code. The definition lives in [docs/PROJECT_BRIEF.md](docs/PROJECT_BRIEF.md), the load-bearing decisions in [docs/adr/](docs/adr/), and the module map in [docs/architecture.md](docs/architecture.md). Slice 0.1 is the first code drop.

## Stack (decided — see ADRs)

- **Desktop:** Tauri 2 — TypeScript frontend, thin Rust backend ([ADR 0002](docs/adr/0002-desktop-stack-tauri2-typescript-thin-rust.md))
- **State:** PostgreSQL 16 in a Docker volume ([ADR 0003](docs/adr/0003-postgres-state-store-and-vault-as-knowledge-truth.md))
- **Memory:** FSRS via `rs-fsrs`, in the Rust backend, next to the database ([ADR 0004](docs/adr/0004-built-in-fsrs-review.md))
- **Knowledge truth:** the Obsidian vault, markdown. Postgres holds mutable and derived state; `Dashboard.md` is abolished — the queue screen is the dashboard ([ADR 0003](docs/adr/0003-postgres-state-store-and-vault-as-knowledge-truth.md))
- **AI:** direct LLM API first; agent sidecars later; MCP server post-1.0 ([ADR 0007](docs/adr/0007-ai-integration-staging.md))

## The daily loop (proposed — [ADR 0005](docs/adr/0005-classical-session-model.md))

**memoria** (clear due reviews) → **lectio** (ingest one next chunk) → **narratio** (tell it back, voice) → **disputatio** (argue your position before the answer is revealed) → **compositio** (leave an artifact).

The queue decides the quantities each day; the shape never changes.
