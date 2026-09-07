# Skills catalog

Curated playbooks, loaded on demand (progressive disclosure) — never imported wholesale into context.

| Skill | Lane | Purpose |
| --- | --- | --- |
| [`disputatio-finalize`](disputatio-finalize/SKILL.md) | all | Deterministic PR finishing: branch names, conventional titles, structured bodies (Summary / Changes / Verification), publish mechanics |
| [`specify`](specify/SKILL.md) | 2 | Generate the spec triple in `specs/<feature>/` for ambiguous or cross-boundary work |

## Provenance policy

- Self-authored skills tie to an ADR or a documented process doc (`docs/PROCESS.md`). The finalize skill encodes the maintainer's established PR conventions.
- Vendor/official skills, if ever vendored, are hash-pinned in a `skills-lock.json` — none today.
- No third-party blog-quality skills: each skill is reviewed like code, because it *is* process code.
