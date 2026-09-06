# 0007. AI integration: staged — direct API first, sidecars later, MCP last

- Status: Accepted
- Date: 2026-09-06

## Context

The harness needs AI for card drafting, narration fidelity scoring, and tutor dialogue; later, heavier ingest/backfill jobs; and eventually interop with the owner's coding agents (Cursor/ZCode already drive the vault today). Three integration patterns exist for a desktop app: plain HTTP to LLM APIs, spawning agent CLIs as sidecars, and exposing the app itself as an MCP server. Research ranked them by effort vs value in that order for this use case.

## Decision

Stage AI by slice, all behind provider ports (deterministic offline adapters in tests, learny's pattern):

1. **Direct LLM API** (slices 0.3–0.4): card drafting, fidelity scoring, disputatio tutor. BYOK API keys managed in-app; prompts versioned in the repo; structured outputs preferred.
2. **Agent-CLI sidecars** (when ingest jobs outgrow simple calls): the app spawns the owner's existing agent CLIs as sidecar processes for bulk backfill/ingest — reusing proven tooling without coupling the core loop to it.
3. **MCP server** (post-1.0): disputatio exposes queue/session/scheduling tools over MCP so coding agents can read and write the harness — the app becomes agent-native instead of agent-hosted.

## Consequences

- The core loop never depends on an external agent being installed or logged in.
- Vendor changes are port-level, not domain-level.
- The MCP surface is a deliberate late addition: the internal data model should stabilize first, or the tool contract churns.
