# 0001. Process and quality bar

- Status: Accepted
- Date: 2026-09-06

## Context

Solo project, but the owner runs several active repositories that receive PRs (fakeflix is the standing quality bar; learny demonstrates the ADR/hexagonal discipline). Past side projects died from prototype sprawl: code without decisions, decisions without records. A study tool is also used daily — regressions are felt immediately, so quality is functional, not cosmetic.

## Decision

1. Every slice is shippable and merges through a PR, even when the author is the only reviewer. Small PRs; conventional commits (`docs:`, `feat:`, `chore:`).
2. Every load-bearing decision gets an ADR here, before or together with the code that implements it.
3. Hexagonal boundaries are enforced by module layout (`domain` imports nothing from `application`, `infrastructure`, `presentation`, or any framework) — checked in review, not by hope.
4. Nothing merges that isn't usable: merged means the daily loop still works end to end (PROJECT_BRIEF → Roadmap acceptance criteria).

## Consequences

Slower start than a prototype sprint; in exchange, decisions survive context loss, slices never strand the user mid-loop, and the repo stays legible to future contributors (or future selves).
