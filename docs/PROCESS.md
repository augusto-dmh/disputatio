# Process — the three lanes

Work is sized by lanes, not by a single mandatory cycle. Heavyweight process pays off for ambiguous, cross-boundary features and is ceremony everywhere else (the consensus of Anthropic/OpenAI/Cursor guidance, the spec-tooling critique — and TLC class 23: "começar com o mínimo que garanta qualidade").

## Lanes

| Lane | When | Planning artifact | Stored spec |
| --- | --- | --- | --- |
| **0 — Direct** | small fix, single subsystem, obvious done-state | none — go straight to code | none — the PR is the artifact |
| **1 — Planned** | bounded feature, one lane of the architecture | ephemeral plan ("done when" in one sentence); plan mode if available | none — plan is throwaway |
| **2 — Specified** | ambiguous work, Rust↔TS crossings, schema changes, and every roadmap slice by default | interview → reviewable "what" | `specs/<feature>/` triple (3-file cap) + expectations; ADR if load-bearing |

## Triggers

**Escalate** when any of these is true:

- You can't state "done when" in one sentence.
- The change spans `src/` and `src-tauri/` (the IPC boundary).
- A migration touches the record schema (ADR 0003).
- Two reasonable designs exist and picking wrong costs a rewrite.

**De-escalate** when, mid-lane, the spec turns out to describe a one-file change — shrink the ceremony, note it in the PR.

## Verification (the only universal gate)

Every lane ends the same way: `make check` green, conventional commit, PR template filled. Lanes change the thinking before the code, never the gate after it. The Claude Code Stop hook runs `make check` so an agent literally cannot finish without it.

## Governance-as-code

Written rules that matter become checks in `scripts/fitness.py` (TLC classes 21/39/62: manual governance → semi-automated → automated; automate what repeats). When a review comment repeats twice, promote it: fitness check → lint rule → CI job, in that order.

## Growth rule (the anti-bureaucracy clause)

- Add a rule only after the same mistake happens **twice**.
- Prune quarterly: rules nobody obeys get deleted, not re-explained.
- The harness itself follows class 23: start with the minimum that guarantees quality; complexity enters when the need appears.
- `AGENTS.md` stays under 160 lines — depth belongs in skills and docs, loaded on demand.
