# 0003. Postgres in a Docker volume as the state store; the vault as knowledge truth

- Status: Accepted
- Date: 2026-09-06

## Context

The app needs one home for mutable state — queue, sessions, scheduling, review history — and a policy for the Obsidian vault it studies from. The desktop default would be SQLite, but the owner explicitly chose **PostgreSQL in a Docker volume**: it matches learny's structures (migrations, full-text search, pgvector headroom), the operational habits already exist (compose, backups, monitoring), and one engine serves both projects' knowledge. In the zero-based review it also became clear that `Dashboard.md` and the branch/PR ceremony for study records exist only because coding agents are stateless and need file-based state plus write safety rails. An app with its own database needs neither.

## Decision

1. **PostgreSQL 16 in a Docker volume** is the only mutable state store. `docker/docker-compose.yml` defines it (host port 5433 to avoid clashing with other local instances); the app starts the compose project and health-checks it before connecting, and shows a clear error when the Docker daemon is down.
2. **The vault is knowledge truth.** The app reads `courses/**` markdown; it writes only generated exports into a dedicated exports folder, on demand. Class notes are never modified, and the app never runs git commands. Vault PRs remain for knowledge changes made by humans or agents.
3. **`Dashboard.md` is abolished.** The queue screen is the dashboard. The study record is the database, exportable to markdown on demand.
4. Migrations are versioned in the repository; the concrete tool is picked in the slice 0.1 migration PR.

## Consequences

- Docker must be running to study; accepted, and surfaced honestly in the UI when it isn't.
- Backups are pg_dump + the volume; cross-machine means dump/restore — no sync story by design.
- The research warning "never let the vault and the scheduler fight" is resolved by ownership: vault owns knowledge, Postgres owns state, joined by vault paths stored on sources/chunks.
