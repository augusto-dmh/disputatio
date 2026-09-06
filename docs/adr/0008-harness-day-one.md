# 0008. Day-one quality harness

- Status: Accepted
- Date: 2026-09-06

## Context

Before slice 0.1, the repo needed its harness: the always-on agent context, the verification vocabulary, the enforcement gates, and the development process — so quality exists from the first line of code, not retrofitted. Three inputs shaped it: (1) a survey of the owner's own repos (learnly, fakeflix, tally, herald, kappy) for proven conventions; (2) current vendor guidance (Anthropic's Claude Code/skills/hooks docs, OpenAI's AGENTS.md standard, Cursor rules, Spec Kit/Kiro) and its critique — heavyweight spec cycles pay off only for ambiguous, cross-boundary work; (3) the Tech Leads Club fundamentos course itself: governance-as-code ("enforçar no código"), fitness functions, three-tier governance, and the anti-overengineering doctrine ("o mínimo que garanta qualidade", class 23; class 19's arc shows correct patterns in wrong context as the named failure mode).

## Decision

1. **`AGENTS.md` is the single always-on context** (≤160 lines, enforced by fitness). `CLAUDE.md` imports it for Claude Code; other tools read it natively. No per-tool mirrors.
2. **Three work lanes** instead of one mandatory spec cycle (docs/PROCESS.md): Direct / Planned / Specified, with written escalation triggers. Lane 2 stores at most a 3-file spec triple (`requirements` EARS, `design`, `tasks`) plus recorded expectations — Spec-Kit/Kiro formats, capped.
3. **Verification vocabulary in a Makefile** (`setup/infra/dev/lint/test/check/fitness/eval`) — same names for human, agent, and CI. `make check` is the only universal gate; a Claude Code Stop hook runs it so agents cannot finish red.
4. **Governance-as-code from day 0**: `scripts/fitness.py` enforces ADR naming, the AGENTS.md cap, the specs 3-file cap, domain purity, the TS↔Rust import boundary, and the compose contract. It lives in `make lint` and CI, and grows as the codebase does.
5. **Guardrails**: conventional commits + PR titles (git hook + CI), PR template (lane checkbox), dependabot, path-gated Rust/TS CI jobs ready for slice 0.1, branch protection on main enabled after this PR merges (checks must exist first).
6. **Evals reserved, not built**: `evals/` + `make eval` exist as vocabulary; cases land with the first prompts (slice 0.3, ADR 0006/0007).

**Deliberately not adopted** (the "not a copy" receipt): learny's per-feature 6-file `.specs/` cycle (replaced by the lane system — Tier-2 weight only where it pays); `.specs/project/STATE.md` AD-log (ADRs are the only decision log — no duplication); constitution files and mandatory analyze/checklist phases (documented overhead); per-tool config mirrors beyond the CLAUDE.md import; Cursor/Codex configs (unused mirrors rot).

## Consequences

- Quality gates bind the harness itself: this PR had to pass `make fitness` and the commit conventions before shipping — the checker is dogfooded on its own introduction.
- The lane system requires honesty about scope; the escalation triggers are prose today and may earn a fitness check if they're repeatedly ignored (the growth rule).
- Branch protection lands immediately post-merge; until slice 0.1, CI runs fitness + markdownlint + compose validate + PR-title check only — Rust/TS jobs activate via path conditions, not new workflows.
