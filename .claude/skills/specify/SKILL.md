---
name: specify
description: Generate a Lane-2 spec triple (requirements, design, tasks) in specs/<feature>/ for ambiguous or cross-boundary work. Use when escalating to the Specified lane.
---

# specify

Produce the reviewable "what" before the "how". Three files, never more (`make fitness` enforces the cap).

1. **Interview first** (plan mode if available):
   - What problem, why now, which slice?
   - What are the *expectations* — measurable, written before building? ("sem expectativa registrada, a decisão não pode ser avaliada depois" — TLC class 22)
   - What is "done when", as verification commands plus observable outcomes?
   - Which modules/boundaries does it touch? Rust↔TS crossings? Schema changes?
   - What alternatives were rejected, and why?
2. **Scaffold**: copy `specs/_templates/{requirements,design,tasks}.md` into `specs/<feature-slug>/`; fill from the interview.
   - `requirements.md`: Context, Expectations, EARS requirements (WHEN/IF … THE SYSTEM SHALL), Done-when.
   - `design.md`: approach, modules touched, data-model changes, ≥1 rejected alternative, ADR link if load-bearing.
   - `tasks.md`: ordered, each task one PR-able commit; if the list exceeds ~8 tasks, split the feature.
3. **Decide the ADR**: if a choice here is hard to reverse or shapes architecture, write it in `docs/adr/` and link it from `design.md`. ADRs are the only decision log — don't restate the spec in the ADR or vice versa.
4. **Gate**: present the triple for human review *before* implementation starts. `make fitness` must pass (3-file cap).
