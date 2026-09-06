# AGENTS.md — disputatio

Single always-on context for humans and AI agents in this repo. Claude Code imports it via `CLAUDE.md`; every other tool reads it natively. Keep it lean: a rule earns its place by preventing a repeated mistake, and rules nobody obeys get pruned (see `docs/PROCESS.md` → Growth rule).

## What this is

Local-first desktop study environment: Tauri 2 (TypeScript frontend, thin Rust backend), Postgres 16 in a Docker volume, Obsidian vault as knowledge truth.

- Definition: [docs/PROJECT_BRIEF.md](docs/PROJECT_BRIEF.md) · Decisions: [docs/adr/](docs/adr/) (the **only** decision log) · Process: [docs/PROCESS.md](docs/PROCESS.md)

## Verification vocabulary

Same commands for human, agent, and CI. If you can't verify it, don't ship it.

- `make setup` — one-time: wire git hooks (`core.hooksPath`)
- `make infra` — start Postgres (Docker must be running; host port 5433)
- `make dev` — run the app (arrives with slice 0.1)
- `make lint` — fitness functions + linters
- `make test` — unit/integration tests (arrives with slice 0.1)
- `make check` — **the gate**: lint + test, run before every PR
- `make fitness` — architecture boundaries only
- `make eval` — AI-output evals (arrives with slice 0.3)

## Layout

`docs/` (brief, ADRs, process) · `docker/` (Postgres) · `scripts/` (fitness) · `specs/<feature>/` (Lane-2 triples, 3-file cap) · `evals/` · `src-tauri/` + `src/` (slice 0.1).

## Layering — enforced by `scripts/fitness.py` via `make fitness`

- `domain` imports nothing from `application`/`infrastructure`/`presentation` and no framework (`tauri`, `sqlx`, `reqwest`, `rusqlite`, …)
- `presentation` (Tauri commands) is translation only: small DTOs in, small DTOs out
- TypeScript never imports Rust paths; the UI talks through `invoke` only
- IPC payload rule: aggregate and paginate in Rust; never ship full markdown over `invoke`
- A `specs/<feature>/` directory contains at most `requirements.md`, `design.md`, `tasks.md`

## Work lanes (full rules: docs/PROCESS.md)

- **Lane 0 — Direct**: small fix. Branch, change, `make check`, PR. No plan, no spec.
- **Lane 1 — Planned**: bounded feature. Ephemeral plan first — "done when" in one sentence — then execute. Nothing stored beyond the PR.
- **Lane 2 — Specified**: ambiguous or cross-boundary work (and every roadmap slice). `specs/<feature>/` triple + recorded expectations + ADR if load-bearing.

Escalate a lane when: you can't state "done when" in one sentence; the change spans `src/` and `src-tauri/`; a migration touches the record schema. De-escalate when the spec turns out to describe a one-file change.

## Decisions

ADRs are the only decision log — no STATE.md, no duplication. Write an ADR when a choice is hard to reverse, shapes the architecture, or closes an open question. Expectations for Lane-2 work are recorded inside the spec: "sem expectativa registrada, a decisão não pode ser avaliada depois" (TLC, class 22).

## PR rules

Conventional Commits for commits and PR title (enforced: `.githooks/commit-msg` + CI). One PR per change; small PRs. Template filled in: lane declared, `make check` green, ADR linked when applicable. Agents never push to main and never merge — propose; the human ships.

## Machine notes

- git/gh run in WSL (`~/projects/disputatio`); Docker runs Windows-side; Postgres on host port 5433.
- `.env` is gitignored: copy `.env.example`, set `POSTGRES_PASSWORD` (compose fails fast without it).
- No network → `make infra` fails fast. Surface the failure; don't retry blindly.

## Growth rule

Add a rule here only after the same mistake happens twice. Prune quarterly. This file stays under 160 lines — `make fitness` enforces it.
