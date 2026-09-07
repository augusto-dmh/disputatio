# tasks — queue-slice (slice 0.1)

Ordered; each task is one PR-able commit/branch. If this list exceeds ~8 tasks, split the feature.

1. [x] Scaffold: `pnpm create tauri-app` (react-ts), pnpm + Biome + Vitest + Tailwind 4 wiring, `.nvmrc`, `make dev` target, CI rust/frontend jobs + dependabot cargo/npm activated (ADR 0009)
2. [ ] Persistence: sqlx pool, migration 0001 (`sources`, `chunks`, `sessions`, `session_stages`, `app_settings`), testcontainers first integration test, committed `.sqlx/` for offline CI
3. [ ] Vault indexer: walk fundamentos + system-design + videos (one video = one chunk), `scope.md` seeding, idempotent upsert on `vault_path`, `reindex` command
4. [ ] Settings: `app_settings`-backed vault path + per-track position override; Settings screen
5. [ ] Queue service: ordo next-per-track, stale (14 days), track rotation; `get_queue` command; tauri-specta bindings
6. [ ] AnkiGateway port + AnkiConnect adapter: due total, graceful "Anki offline"
7. [ ] Home screen: queue list (next per track, stale flags, due header), start/stop session logging, complete chunk
8. [ ] Session logging close-out: duration, stage rows; verify expectations; update README screenshots; run full `make check` + manual smoke and record results in the PR
