# Agent-harness practices (2026-09-06)

Research dossier behind ADR-0008 (harness) and `docs/PROCESS.md` (lanes). Sources: official vendor guidance plus the spec-tooling ecosystem and its critics, September 2026.

## What each player prescribes

- **Anthropic** — CLAUDE.md: short, loaded every session, only broadly applicable content; an over-specified context file that does not change model behavior is a real failure mode. Explore → plan → code; for larger features, interview the user and write a self-contained spec with a verification step, then execute in a fresh session. "If you can't verify it, don't ship it": tests, build exit codes, deterministic Stop hooks beat advisory prose; subagents give context isolation and adversarial review. Agent Skills: directory + YAML frontmatter with three-level progressive disclosure (metadata always, body on demand, bundled files when needed). Building effective agents: prefer workflows over autonomous agents; evaluator-optimizer only when feedback demonstrably helps. Context engineering: the smallest set of high-signal tokens; just-in-time retrieval over preloading. https://code.claude.com/docs/en/best-practices · https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills · https://www.anthropic.com/engineering/building-effective-agents · https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents · https://anthropic.skilljar.com
- **OpenAI** — AGENTS.md: "a README for agents", no required schema; recommended sections are setup commands, build/test/lint commands, code style, testing instructions, PR rules; discovery walks from the repo root and the closest file to the edited file wins; adopted across Codex, Gemini CLI, Cursor, Copilot. Codex prompt pattern: Goal / Context / Constraints / "Done when". "A short, accurate AGENTS.md beats a long vague one" — link out to detail docs. https://agents.md · https://learn.chatgpt.com/guides/best-practices
- **Cursor** — `.cursor/rules/*.mdc` with frontmatter giving four rule types (always / auto-attached by glob / agent-requested by description / manual); keep rules under 500 lines; reference files instead of copying; plan mode for multi-file or architectural work — "for quick changes, jumping straight to Agent mode is fine"; meta-rule: add a rule only when the agent makes the same mistake repeatedly. https://cursor.com/docs/context/rules · https://cursor.com/docs/agent/plan-mode
- **Spec-driven tooling** — GitHub Spec Kit: constitution → specify → clarify → plan → tasks → analyze → checklist → implement; explicitly allows small fixes to use the normal issue/PR/review/test process. AWS Kiro: requirements (EARS: WHEN/IF … THE SYSTEM SHALL), design, tasks with dependency graphs; "Quick Spec" skips approval gates; steering files with always/fileMatch/manual inclusion. https://github.com/github/spec-kit · https://kiro.dev/docs/specs/
- **Critique** — spec-driven development pays for multi-day, ambiguous, multi-file features where requirements are genuinely uncertain; it is ceremony for small tasks, bug fixes, and spikes; artifacts drift from code without discipline; teams report running full SDD for months without provable benefit. https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html

## Convergences

Lean always-on context file · progressive disclosure · an explicit verification loop the agent can run itself · plan-then-execute only for big work · file-based, version-controlled rules iterated from observed mistakes · deterministic enforcement (hooks, linters) over prose.

## Divergences

Three competing always-on formats (CLAUDE.md vs AGENTS.md vs cursor rules) — AGENTS.md is the emerging vendor-neutral standard; spec artifact weight (mandated phase trees vs ephemeral plans); rules granularity (per-path conditionals vs one lean file plus links).

## Task-size tiers (the right-sizing consensus)

Tier 0: no plan, no spec — direct execution with the verification commands. Tier 1: prompt with Goal/Constraints/"Done when" plus an ephemeral plan. Tier 2: written spec artifacts only for ambiguous, cross-boundary, architectural work. Escalate when you cannot state "done when" in one sentence or the change spans subsystems/languages.

## Adopted implications

One lean always-on file (AGENTS.md, capped) · hooks before rules · conditional depth via skills and glob-scoped rules · plans are throwaway by default · spec artifacts only at Tier 2 · tier triggers written into the context file · rules grow from repeated mistakes and get pruned · the verification suite is the only universal gate — spec files are never part of "done".
