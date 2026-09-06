@AGENTS.md

## Claude Code specifics

- Lane 1/2 work: start in plan mode — interview, present the plan, then write code.
- A Stop hook runs `make check` when you finish; fix failures before ending the turn.
- Skills: `/ship` (pre-PR checklist), `/specify` (Lane-2 triple) — catalog in `.claude/skills/SKILLS.md`.
