# 0009. Stack pinning

- Status: Accepted
- Date: 2026-09-06

## Context

ADR 0002 chose Tauri 2 with a TypeScript frontend and thin Rust backend; research for slice 0.1 mapped what the ecosystem has embraced per layer. Finding: **no Laravel/Rails-grade harness exists for Tauri** — the official scaffolder (`create-tauri-app`) generates a starter and stops, with no generators, migrations, or opinionated structure. Each layer borrows its own ecosystem's embraced default, and the repo must pin those choices. The official Tauri `sql` plugin is frontend-facing and therefore wrong for this repo's thin-Rust architecture (ADR 0003/0007).

## Decision

- **Frontend:** pnpm + Vite + React 19 + TypeScript + Tailwind v4 + shadcn/ui (learnly's UI conventions, minus Next) · Biome as the single linter/formatter · Vitest for unit tests.
- **Rust:** rustfmt + clippy with `-D warnings` as the CI gate; plain `cargo test` (nextest deferred until the suite is big enough to feel it).
- **Persistence:** sqlx (async, compile-time-checked queries) + `sqlx migrate` for migrations · testcontainers-rs(Postgres) for integration tests · the committed `.sqlx/` directory + `SQLX_OFFLINE=true` in CI for deterministic builds.
- **IPC:** tauri-specta (2.0.0-rc, accepted knowingly) generating typed `invoke()` bindings — generated files kept outside hot-reload paths.
- **Deferred:** release pipeline (tauri-action + updater plugin) when there's something to distribute; e2e (tauri-driver/Playwright) — Vitest + manual smoke until the stack's weakest layer settles.

## Consequences

- Biome's plugin coverage is narrower than ESLint's; if a needed rule is missing, add it deliberately rather than reintroducing ESLint wholesale.
- tauri-specta may break across Tauri minors while at RC; the port/adapters layering keeps the blast radius at the presentation edge.
- `sqlx` offline state must be regenerated whenever migrations change (`cargo sqlx prepare` with a live DB) — this becomes a checklist item in the ship skill.
- No down-migrations under `sqlx migrate`; recovery is forward-fix migrations, matching the record-store philosophy (ADR 0003).
