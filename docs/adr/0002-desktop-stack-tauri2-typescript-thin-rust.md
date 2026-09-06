# 0002. Desktop stack: Tauri 2, TypeScript frontend, thin Rust backend

- Status: Accepted
- Date: 2026-09-06

## Context

Windows-first desktop app with a content-heavy UI (markdown, voice recording, chat, charts) and deep local integration: filesystem access to the Obsidian vault, a Dockerized Postgres, LLM APIs, audio capture, and (interim) AnkiConnect over HTTP. The owner is fluent in TypeScript and wants Rust or Go in the stack. Research (Sept 2026) compared:

- **Tauri 2.x** — stable since Oct 2024 (~v2.11 line); official plugins for sql, fs, http, notifications, and shell with sidecar spawning; WebView2 on Windows; small binaries.
- **Wails v3 (Go)** — public beta only since Aug 2026: API churn risk, thinner plugin ecosystem.
- **Electron** — reliable and boring, but ~100–200 MB bundles and a third runtime to own; nothing here needs Node native modules.
- **Pure Rust/Go GUIs** (Dioxus, Iced, Slint, egui, Fyne, Gio) — markdown rendering, tables, syntax highlighting, and chart polish are all DIY; Dioxus is webview-based on desktop anyway.

## Decision

Tauri 2.x. TypeScript + React 19 + Vite + Tailwind on the frontend (mirroring learny's frontend choices minus Next), thin Rust backend via Tauri commands. Rust owns vault IO, Postgres access, FSRS scheduling, process control (sidecars), and provider HTTP; the webview renders. "Thin" is enforced by the layering rules in [architecture.md](../architecture.md): Tauri types stop at `presentation`.

## Consequences

- The capabilities/ACL system (fs scopes, sidecar spawn permissions) has a learning curve; misconfiguration is the known friction point — budget for it in slice 0.1.
- The IPC payload rule (aggregate and paginate in Rust; stream bulk data via custom protocol) is mandatory, or the app gets slow (Tauri's documented bandwidth wall).
- Wails v3 is the designated runner-up if Rust proves unproductive — the hexagonal boundaries keep that exit cheap.
