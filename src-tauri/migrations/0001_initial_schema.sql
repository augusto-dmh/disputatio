-- Migration 0001: initial queue-slice schema (ADR 0003 sketch made real;
-- specs/queue-slice/design.md "Data model changes"). Minimal for v1 queue
-- logic — full-text search over chunk titles is deferred (design.md).

CREATE TABLE sources (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- course | video | anki_import (docs/architecture.md data model sketch)
    kind TEXT NOT NULL CHECK (kind IN ('course', 'video', 'anki_import')),
    -- track name the queue rotates over (design.md: fundamentos → system-design → video)
    track TEXT NOT NULL,
    title TEXT NOT NULL,
    -- stable identity: re-index upserts sources on it (requirements: idempotent indexing)
    vault_path TEXT NOT NULL UNIQUE,
    ord INT NOT NULL DEFAULT 0
);

CREATE TABLE chunks (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_id BIGINT NOT NULL REFERENCES sources (id) ON DELETE CASCADE,
    ord INT NOT NULL,
    title TEXT NOT NULL,
    -- the stable key: the queue upserts chunks on it when re-indexing (design.md)
    vault_path TEXT NOT NULL UNIQUE,
    est_minutes INT,
    -- queue states (queued | in_progress | done — design.md) plus the ingest
    -- pipeline states (ingested | narrated | disputed | composed — architecture.md sketch)
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN (
        'queued', 'in_progress', 'done',
        'ingested', 'narrated', 'disputed', 'composed'
    )),
    -- drives the stale rule: in_progress untouched ≥ 14 days (requirements.md)
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (source_id, ord)
);

CREATE TABLE sessions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ
);

CREATE TABLE session_stages (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    session_id BIGINT NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    -- memoria | lectio | narratio | disputatio | compositio (architecture.md)
    stage TEXT NOT NULL CHECK (stage IN ('memoria', 'lectio', 'narratio', 'disputatio', 'compositio')),
    chunk_id BIGINT REFERENCES chunks (id),
    payload JSONB,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ
);

-- key/value store: vault path, per-track position (design.md "Data model changes")
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
