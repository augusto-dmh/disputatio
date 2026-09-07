# design — queue-slice (slice 0.1)

## Approach

Thin vertical slice, hexagonal per `docs/architecture.md`. Rust owns everything real (indexing, queue logic, DB, AnkiConnect); React renders one Home screen from a single `get_queue` command; tauri-specta generates the TS bindings. Sessions log as rows; the dashboard is the screen (ADR 0003 — no markdown regeneration).

Queue logic (application service, pure domain rules):

1. Per track: first chunk with status `queued` = next; an `in_progress` chunk untouched ≥14 days = stale, surfaced first.
2. Merge tracks by stable rotation (fundamentos → system-design → video) so no track starves; the user can always pick any visible item.
3. Anki due total is display-only in this slice — it never gates the queue (built-in FSRS replaces it in 0.2).

Chunk identity: `vault_path` is the stable key (ADR 0003); re-index upserts on it. Seeding: parse `courses/*/sessions/*/scope.md` to mark already-studied classes done; "set current position" in settings bulk-marks chunks before the chosen one done.

## Modules touched

- `src-tauri/src/domain` — Chunk, Source, Session, Stage entities; ports: `VaultReader`, `AnkiGateway`, `Repo`
- `src-tauri/src/application` — `queue.rs` (build_daily_queue), `session.rs` (start/stop/complete), `index.rs` (orchestrate indexing)
- `src-tauri/src/infrastructure` — `pg.rs` (sqlx pool in `tauri::State`), `vault_fs.rs` (walks cursos + videos + scope.md), `anki_connect.rs` (HTTP, graceful offline), `specta.rs` (bindings emission)
- `src-tauri/src/presentation` — commands: `get_queue`, `start_session`, `stop_session`, `complete_chunk`, `get_settings`, `set_settings`, `reindex`
- `src/` — Home (queue list + due header + start/stop), Settings (vault path, per-track position)

## Data model changes

Initial migrations (ADR 0003 sketch made real): `sources`, `chunks`, `sessions`, `session_stages`, `app_settings` (key/value; vault path, per-track position). FTS5/tsvector over chunk titles arrives later — not needed for v1 queue logic.

## Alternatives considered

- Read the vault directly from TypeScript — rejected: breaks the layering and IPC payload rules (ADR 0002/0008).
- Keep generating Dashboard.md — rejected in ADR 0003 (agents' stateless-file workaround).
- Mark studied classes manually via a first-run wizard — rejected for v1: `scope.md` seeding + a position override achieves it with less friction; revisit if seeding proves unreliable.
- Hand-written TS command types — rejected: tauri-specta (ADR 0009) removes drift even at RC.

## ADR

[0009 — stack pinning](../../docs/adr/0009-stack-pinning.md): pnpm + Vite + React 19 + Tailwind 4 + shadcn/ui + Biome + Vitest; sqlx + `sqlx migrate` + testcontainers; tauri-specta (RC, accepted); e2e deferred (Vitest + manual smoke).
