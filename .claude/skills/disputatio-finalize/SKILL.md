---
name: disputatio-finalize
description: Finalizes and publishes disputatio changes with consistent branch names, Conventional Commit messages and PR titles, structured PR bodies, verification notes, and the publish step. Use when asked to finalize work, commit, push, open or update a pull request, or write a PR description. Not for implementing features, reviewing code, or debugging CI.
license: CC-BY-4.0
metadata:
  author: disputatio contributors
  version: 1.0.0
---

# disputatio Finalize

Apply this repository's conventions when preparing or publishing completed work. Finalizing authorizes inspecting the diff, choosing metadata, staging intended files, committing, pushing, and opening a ready-for-review PR. Keep narrower requests proportional.

## Naming

Conventional Commits for commits and PR titles — `<type>(<optional-scope>)<optional-!>: <imperative summary>` — types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `build`, `ci`, `perf`, `style`, `revert`. Scope with the touched area (`frontend`, `queue`, `ingest`) when it aids scanning.

Branch names: `<type>/<short-kebab-summary>` — lowercase, concise, no dots or version numbers. Examples: `feat/app-skeleton`, `chore/hook-wiring`, `docs/pr-description-format`.

## PR body (fixed shape)

```markdown
## Summary
(What and why, plain prose — understandable with no access to internal planning.)

## Changes
(Concrete changes, grouped by area; bold sub-headers when the PR is big.)

## Verification
(Commands run and results: test counts, lint/type/build green. Manual-pass items last.)

*Process: lane <Direct|Planned|Specified> · ADR <link or none>.*
```

Rules (enforced by review):

- Paragraphs are **single unwrapped lines** — never hard-wrap prose; GitHub renders the wraps literally.
- **Self-contained history**: no internal task IDs ("task 3/8", "slice 0.1 phase 2"). Reference spec paths (`specs/queue-slice/`) instead; a PR must stand alone.
- **No AI attribution** anywhere: no `Co-Authored-By`, no "Generated with", no model names.
- Declare the lane and link the ADR whenever a choice is load-bearing.

## Verification before publishing

`make check` green is the only universal gate: fitness (boundaries), markdownlint, compose config, `pnpm biome check .`, `pnpm vitest run`, `pnpm build`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Rust/TS parts are skipped honestly (echo) while the corresponding code does not exist; never claim a check you did not run.

## Publish mechanics (machine-specific)

1. `git checkout main && git pull --ff-only`, branch `type/kebab-summary` from main.
2. Commit (the `commit-msg` hook enforces the format), push with `-u`.
3. `gh pr create --base main --head <branch> --title <conventional> --body-file <file>`.
4. **Never** `gh pr edit` on this machine (GraphQL projectCards break it) — update via `gh api -X PATCH repos/augusto-dmh/disputatio/pulls/N -f title=... -F body=@file`. No `jq` in WSL — use `gh api` field files or python3.
5. Agents never merge. Report the PR URL and stop; the human ships. After merge: sync main, delete the branch, tick the spec task if one maps to it.
