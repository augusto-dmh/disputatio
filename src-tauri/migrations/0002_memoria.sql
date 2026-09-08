-- Migration 0002: memoria-slice schema (ADR 0004 built-in FSRS review;
-- specs/memoria-slice/design.md "Data model changes"). Cards carry their
-- FSRS memory state; reviews are the append-only log the scheduler owns.
-- Column types follow rs-fsrs: f64 stability/difficulty, i64 day counts.

CREATE TABLE cards (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- nullable: Anki-imported cards predate the vault index; narratio drafts (0.3) set it
    chunk_id BIGINT REFERENCES chunks (id) ON DELETE SET NULL,
    -- stable external key: the one-time import upserts on it (idempotent, like vault_path)
    anki_card_id BIGINT UNIQUE,
    deck TEXT,
    front TEXT NOT NULL,
    back TEXT NOT NULL,
    -- curation gate (architecture.md sketch): imports arrive pre-curated = kept;
    -- drafts (0.3) start as draft; killed cards never surface for review
    curation TEXT NOT NULL DEFAULT 'kept' CHECK (curation IN ('draft', 'kept', 'killed')),
    -- FSRS state — written only by the scheduler path (application/review.rs);
    -- f64/DOUBLE PRECISION because rs-fsrs Card carries stability/difficulty as f64
    stability DOUBLE PRECISION,
    difficulty DOUBLE PRECISION,
    -- learning | review | relearning (rs-fsrs State; NULL = never reviewed = new)
    fsrs_state TEXT CHECK (fsrs_state IN ('learning', 'review', 'relearning')),
    due TIMESTAMPTZ,
    last_review_at TIMESTAMPTZ,
    reps INT NOT NULL DEFAULT 0,
    lapses INT NOT NULL DEFAULT 0
);

CREATE TABLE reviews (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    card_id BIGINT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    -- 1=again 2=hard 3=good 4=easy (FSRS ratings)
    grade INT NOT NULL CHECK (grade BETWEEN 1 AND 4),
    reviewed_at TIMESTAMPTZ NOT NULL,
    -- fsrs_state of the card BEFORE this review; NULL = first review
    state_before TEXT,
    -- rs-fsrs carries day counts as i64
    elapsed_days BIGINT NOT NULL DEFAULT 0,
    scheduled_days BIGINT NOT NULL DEFAULT 0,
    duration_ms INT,
    -- review-log import dedup: one Anki revlog row per card per timestamp
    UNIQUE (card_id, reviewed_at)
);

CREATE INDEX cards_due_idx ON cards (due);
