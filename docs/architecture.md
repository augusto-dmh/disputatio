# Architecture

Hexagonal, in the learny tradition: the domain owns the rules, infrastructure owns the world, and nothing in the domain imports Tauri, SQL, HTTP, or AI SDKs. This document is the module map slice 0.1 implements; the data model is a sketch until the first migration PR lands.

## Repo layout (target)

```
disputatio/
├── docs/                     # brief, ADRs, this map
├── docker/                   # postgres compose (ADR 0003)
├── src-tauri/                # Rust backend
│   └── src/
│       ├── domain/           # entities + ports (no framework imports)
│       ├── application/      # services: queue, review, narration, tutor, ingest
│       ├── infrastructure/   # pg.rs, vault_fs.rs, anki_connect.rs, llm.rs, asr.rs
│       └── presentation/     # tauri commands: thin, IPC-shaped
├── src/                      # TypeScript frontend (React 19 + Vite + Tailwind)
└── package.json
```

## Layering rules

1. `domain` defines ports (traits); `infrastructure` implements them. Services in `application` compose ports only.
2. `presentation` (Tauri commands) is translation only: take small DTOs, call one service, return small DTOs. **IPC payload rule:** aggregate and paginate in Rust — never ship full markdown documents or unbounded result sets over `invoke`; stream bulk data through a custom protocol handler instead. This is the one way Tauri apps get slow.
3. Providers (LLM, ASR) sit behind ports so switching vendors never touches the domain (learny's deterministic-offline-adapter pattern, reused in spirit).

## Data model sketch

```sql
sources         (id, kind [course|video|anki_import], track, title, vault_path, ord)
chunks          (id, source_id, ord, title, vault_path, est_minutes,
                 status [queued|ingested|narrated|disputed|composed])
sessions        (id, started_at, ended_at)
session_stages  (id, session_id, stage [memoria|lectio|narratio|disputatio|compositio],
                 chunk_id, payload jsonb, started_at, ended_at)
cards           (id, chunk_id, front, back, state [draft|kept|killed],
                 fsrs_stability, fsrs_difficulty, due, last_review_at)
reviews         (id, card_id, reviewed_at, grade, elapsed_s, scheduled_days)
artifacts       (id, chunk_id, session_id, kind [note|code|decision|explainer],
                 vault_export_path)
```

- `cards.fsrs_*` + `reviews` are owned by the Rust FSRS service (rs-fsrs); nothing else writes them.
- `chunks.status` is what the queue reads to compute *ordo* progression and stale backlog.
- Full-text search over chunk content: Postgres `tsvector` column maintained by the ingest indexer.

## The vault contract

- **Reads:** `courses/**` markdown (class notes, transcriptions, summaries) and any indexed video metadata. The vault path is configured once, in the app.
- **Writes:** only generated exports (session summaries, artifacts) into a dedicated exports folder, on demand. Class notes are never modified. Vault git stays for knowledge; the app never runs git commands.
- The study record lives in Postgres (ADR 0003). If disputatio vanished tomorrow, the vault loses nothing.

## Process discipline

- One PR per change, even solo — feature branches into `main`, conventional commits (`docs:`, `feat:`, `chore:`).
- Every load-bearing decision gets an ADR in `docs/adr/` before or with the code that implements it.
- Every slice ends shippable: merged = usable (PROJECT_BRIEF → Roadmap).
