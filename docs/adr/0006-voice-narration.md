# 0006. Voice narration (narratio) in scope

- Status: Accepted
- Date: 2026-09-06

## Context

Narration — telling material back in your own words after one attentive reading — is the classical tradition's convergent recall principle (Mason's core tool; narratio before notation), and landscape research found **zero software coverage** for it. It is the app's clearest differentiator. Generation-based recall also fixes the recognition-based-review violation common to flashcard tools. The owner chose voice-first.

## Decision

1. The narratio stage is **voice-first**: MediaRecorder capture in the webview → ASR transcription → LLM fidelity scoring of the telling against the source chunk → omissions and confusions become **card drafts** → mandatory human keep/kill curation → accepted cards enter FSRS (ADR 0004). Nothing is ever auto-imported.
2. A **typed fallback** exists (same loop, typed summary) for no-mic contexts and accessibility.
3. **ASR engine is open**, decided in slice 0.3 with explicit criteria: latency, cost per minute, privacy (local whisper.cpp sidecar vs cloud Whisper-class API). The engine sits behind a port, so the choice is reversible.
4. Fidelity scoring is also behind a provider port (LLM, ADR 0007), with the prompt versioned in the repository.

## Consequences

- An audio pipeline (capture, codecs, permissions) enters slice 0.3 — the largest new surface in the roadmap.
- Scoring must grade fidelity, not eloquence; the rubric ships with the prompt and is iterated like code.
- Card drafts inherit chunk provenance, so review stays contextual (the mnemonic-medium lesson).
