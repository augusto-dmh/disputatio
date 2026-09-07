# Desktop stack and Tauri tooling map (2026-09-06)

Research dossier behind ADR-0002 (stack), ADR-0003 (state store), and ADR-0009 (stack pinning). Dated September 2026; verify versions before upgrades.

## Stack comparison

- **Tauri 2.x** — stable since Oct 2024, ~v2.11 line by mid-2026; official plugins (sql, fs, http, notifications, shell with sidecar spawning); WebView2 on Windows; small binaries; the only option where every requirement of this app is an official plugin or known pattern. https://v2.tauri.app/blog/tauri-20/ · https://github.com/tauri-apps/tauri/releases
- **Wails v3 (Go)** — public beta since Aug 2026, "stable desktop API" claimed; API churn risk, thinner plugin ecosystem; designated runner-up. https://v3.wails.io/blog/wails-v3-beta/
- **Electron** — reliable and boring; right call only when Node native modules are required; ~100-200 MB bundles. https://releases.electronjs.org/
- **Pure Rust/Go GUIs** (Dioxus, Iced, Slint, egui, Fyne, Gio) — Dioxus is webview-based on desktop anyway; markdown rendering, tables, syntax highlighting, and chart polish are all DIY; not credible for a content-heavy app at solo-dev cost.

## Tooling per layer (community-embraced, 2026)

- Scaffold: `pnpm create tauri-app -t react-ts` (official; scaffolds and stops — no Laravel/Rails-grade harness exists for Tauri; every layer borrows its ecosystem's default). https://v2.tauri.app/start/create-project/
- Rust gates: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` (universal convention); plain `cargo test` first, cargo-nextest when the suite is big enough. https://nexte.st/
- Postgres access: sqlx 0.8 (async, compile-time-checked queries) is the community-embraced choice for thin async Postgres; Diesel = max compile-safety/sync; SeaORM = Active-Record on sqlx. Migrations: `sqlx migrate` (consensus default; no down-migrations — forward-fix). https://github.com/launchbadge/sqlx
- DB tests: testcontainers-rs with the Postgres module (real database per suite, no mocks). https://rust.testcontainers.org/
- TS bindings: tauri-specta v2 (de-facto standard, still 2.0.0-rc) generates typed `invoke()` — keep generated files outside hot-reload-watched directories; max 10 command args (use structs). https://github.com/specta-rs/tauri-specta
- Frontend: pnpm (2026 momentum default), Vite 7, React 19, Tailwind v4 via the Vite plugin, shadcn/ui conventions, Biome as single lint+format tool (accepted for new projects; plugin coverage narrower than ESLint's — add missing rules deliberately), Vitest 5. https://biomejs.dev/guides/migrate-eslint-prettier/
- Releases: tauri-action (builds targets, creates the GitHub release, emits signed updater manifest) + tauri-plugin-updater with a signing key in GH secrets. https://github.com/tauri-apps/tauri-action
- E2E: the stack's weakest layer — tauri-driver (WebDriver) is maintenance-mode and macOS-unsupported; Playwright drives WebView2 via CDP on Windows only. Pragmatic call: Vitest component tests plus manual smoke until something breaks. https://v2.tauri.app/develop/tests/

## Decision points where no convention exists

DB library (sqlx chosen) · migrations tool (sqlx migrate chosen) · e2e approach (deferred) · adopting tauri-specta at RC (accepted) · pnpm vs npm (pnpm) · Biome vs ESLint+Prettier (Biome) · where the DB pool lives (Rust-side pool in tauri State).

## Gotchas

- sqlx offline CI: run `cargo sqlx prepare --workspace -- --all-targets` with a live database, commit the `.sqlx/` directory, set `SQLX_OFFLINE=true` in CI only; keep sqlx-cli version matched to sqlx; missing `--all-targets` omits test-only queries.
- The official Tauri sql plugin is frontend-facing — wrong for a thin-Rust app; use sqlx inside Rust commands behind ports.
- Tauri IPC has a bandwidth wall: `invoke` serializes JSON through strings — aggregate and paginate in Rust, stream bulk data via custom protocol, never block the main thread. https://github.com/orgs/tauri-apps/discussions/5690
- Capabilities/ACL (fs scopes, sidecar spawn permissions) are the top friction source; unsigned sidecar binaries trip SmartScreen.
- CRDT sync (Automerge/Yjs): defer — single user, git as the history layer.
- Full-text search: Postgres FTS (tsvector) covers the need; pgvector headroom if semantic search arrives later. https://www.sqlite.org/fts5.html (original SQLite+FTS5 comparison, superseded by the Postgres decision in ADR-0003)
