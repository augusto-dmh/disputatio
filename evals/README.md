# evals

Reserved vocabulary for AI-output quality gates. Cases land here with the first prompts (slice 0.3 — card drafting, narration fidelity scoring, tutor behavior; ADRs 0006/0007).

Rules, inherited from learny's golden-fixture discipline:

- Deterministic fixtures; network-free adapters as the default provider (real providers behind ports).
- Assert **stable identity** — never generated UUIDs or timestamps.
- Run: `make eval` (CI-gated like the rest of the vocabulary once cases exist).
