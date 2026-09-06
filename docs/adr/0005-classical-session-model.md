# 0005. Classical session model: the five-stage daily loop

- Status: **Proposed** (open decision — the owner was asked twice and deferred; interaction design for slices 0.3–0.4 is gated on accepting this)
- Date: 2026-09-06

## Context

The classical tradition converges on study as a *sequence*: lectio (read) → meditatio (ruminate) → memoria (review) → disputatio (argue) → compositio (produce). Aquinas: disputatio precedes determinatio — the learner argues objections before the settled answer is revealed. Mason: recall in your own words after one attentive reading. Aristotle: the intellectual virtues are acquired by *different* activities (demonstration, making, judgment), by habituation. Sertillanges: capture now, incubate later, end sessions before exhaustion. Ordo (order of the subject matter), not novelty, orders the queue. Modern evidence aligns: a 2025 PNAS study found unguarded LLM answers *hurt* later exam performance (~17%), while a hint-only tutor avoided the harm. Landscape research found no product composes these stages into one daily sequence — every tool does one stage.

## Decision (proposed)

Every session runs the same canonical five stages, with quantities decided daily by the queue:

1. **memoria** — clear due reviews (retention before acquisition).
2. **lectio** — ingest ONE next chunk from the queue (ordo order).
3. **narratio** — tell it back in your own words, voice-first (ADR 0006).
4. **disputatio** — the tutor argues against your stated position; the determinatio is revealed only after the exchange, behind a hint ladder.
5. **compositio** — end by producing a small artifact: a note, code, or a decision.

The shape is invariant; time boxes and quantities vary with what's due and what's stale. The alternative shapes considered: **reviews + tutor chunk** (closest to the current `/study` flow, least new interaction to build) and **free-composed blocks** (maximum flexibility — but it keeps the meta-cognitive load of deciding the session's shape, the exact cost this harness exists to remove).

## Consequences

- Predictable, scriptable pedagogy — "script the pedagogy, not just the model" is the documented success pattern (Synthesis Tutor) and a feature, not a constraint.
- Sessions become directly comparable rows in Postgres (same stages, same payload shape), which powers stats and queue re-ranking in 1.0.
- If rejected, slices 0.3–0.4 need a different interaction spec; 0.1–0.2 are unaffected.
