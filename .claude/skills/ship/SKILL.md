---
name: ship
description: Pre-PR checklist for disputatio - verification, lane declaration, conventional commit, PR body. Use before opening any pull request.
---

# ship

Run the full checklist before any PR. Skip nothing silently.

1. **Sync**: `git checkout main && git pull --ff-only`, branch from main (`feat/…`, `fix/…`, `docs/…`).
2. **Verify**: `make check` green. If it isn't, fix before proceeding — the gate is not negotiable (docs/adr/0008).
3. **Lane**: declare the lane used (Direct / Planned / Specified) in the PR template. Escalate if, while shipping, the change revealed cross-boundary or ambiguous scope (AGENTS.md triggers).
4. **Decisions**: if a load-bearing choice was made, write the ADR (`docs/adr/NNNN-slug.md`, pattern-checked by `make fitness`) and link it in the PR.
5. **Specified lane only**: confirm the spec's *Expectations* section was written before implementation and is linked in the PR.
6. **Commit**: conventional message — `.githooks/commit-msg` enforces `type(scope?): summary`.
7. **PR body**: template filled — what & why, verification output, decisions, checklist ticked.
8. **Stop**: push the branch, open the PR, hand to the human. Agents don't merge.
