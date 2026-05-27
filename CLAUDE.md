# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

EchoMate is a cross-platform (macOS 12+, Windows 10+) desktop AI reply copilot for WeChat. It is a **local, privacy-first tool** that watches for a global hotkey, reads the clipboard, calls Claude Code CLI (`claude -p`) or Codex CLI (`codex exec`) to generate 5 candidate replies, and displays them in a popup. The user manually copies and pastes the chosen reply into WeChat — the tool never sends messages automatically.

## Tech Stack

- **Language**: Rust
- **Desktop shell**: Tauri 2 (system WebView, bundler, plugin permissions)
- **Database**: SQLite via `rusqlite` with SQLCipher encryption (`bundled-sqlcipher-vendored-openssl`)
- **Hotkey**: `tauri-plugin-global-shortcut` or `global-hotkey` crate
- **Clipboard**: `tauri-plugin-clipboard-manager` or `arboard`
- **Input simulation**: `enigo` (for simulating Ctrl/Cmd+C)
- **Subprocess**: `tokio::process` for calling Claude/Codex CLI
- **Logging**: `tracing` + `tracing-subscriber`
- **Keyring**: `keyring` crate for OS keychain (DB key storage)

## Architecture

Four-layer design (see `docs/deep-research-report-*.md` for full diagrams):

1. **Platform layer** — global hotkey, input simulation, clipboard, popup UI, tray, OS permissions
2. **Orchestration layer** — Agent orchestrator, prompt composer, JSON schema validator, Claude/Codex CLI adapter, output parser, timeout/retry/privacy policy
3. **Memory layer** — recent chat fetcher, user style profile, contact facts, memory extractor
4. **Persistence layer** — encrypted SQLite, OS keyring, audit events, local config

### Data Flow

```
User presses global hotkey
  → Simulate Ctrl/Cmd+C
  → Read clipboard text
  → Normalize / deduplicate
  → Save raw message event
  → Query recent N messages + style profile + contact facts
  → Assemble prompt + JSON Schema
  → Call claude -p or codex exec (via subprocess, with hard timeout)
  → Parse JSON output
  → Display 5 candidates + copy buttons in popup
  → User copies one → record send event
  → Async update memory projections & fact confidence
```

### Key Trait Boundaries

The design docs specify these trait boundaries to keep layers decoupled and enable future migration:

- `Provider` — `generate_candidates(req: PromptRequest) -> CandidateEnvelope`
- `ChatRepository` — `save_message`, `recent_messages`, `record_send`
- `MemoryRepository` — `load_style_profile`, `load_contact_facts`, `apply_patch`

### Memory Model

Three-tier context system (not raw history dumps):
- **Raw events table**: messages, candidates, send events with timestamps, provider, latency
- **User style profile**: sentence length, tone, humor/flirt level, common/avoid phrases, emoji usage
- **Contact facts table**: preferences, schedule, taboos, with `evidence_message_ids`, `confidence`, `superseded_by` for explainable memory

## Crate Structure (planned)

```
src-tauri/src/
  app/             // boot, dependency wiring
  platform/        // hotkey, clipboard, input sim, permissions
  agent/           // orchestrator, prompt, parser, schema
  provider/        // claude.rs, codex.rs (CLI adapters)
  store/           // sqlite repos, migrations
  memory/          // style.rs, facts.rs, projection.rs
  security/        // keyring, db key, redaction
  ui/              // tauri commands, window management, tray
  domain/          // message, candidate, memory_item, events
```

## Core Design Constraints

- **No WeChat protocol manipulation** — no hooking, no reverse engineering, no auto-send
- **Hotkey-triggered only** — the tool only acts when the user explicitly presses the hotkey; no background clipboard monitoring
- **Single contact scope** — designed for one conversation partner (no contact identification needed)
- **Local-only storage** — all data stored locally in encrypted SQLite; no cloud upload
- **CLI-based LLM calls** — uses the user's locally installed `claude` or `codex` CLI (user manages their own auth); no direct API calls
- **Hard timeouts on all external processes** — 45s default, `kill_on_drop` semantics
- **Schema-first output** — always use `--json-schema` / `--output-schema` with Claude/Codex; never parse natural language output
- **No admin privileges required** — runs at user integrity level only

## CLI Integration Notes

Claude Code: `claude -p --output-format json --json-schema <schema> --no-session-persistence --max-turns 2`

Codex CLI: `codex exec --json --output-schema <schema> --ephemeral --skip-git-repo-check` (needs `--skip-git-repo-check` because this is not a git repo context for Codex)

Both CLIs require the user to be already authenticated on their machine.

## Design Documents

- `docs/ChatGPT-微信AI助手回复方案.md` — Full product design conversation: feature scope, config schema, prompt templates, privacy rules, MVP roadmap
- `docs/deep-research-report-Rust 与 Go 构建本地跨平台 Agent 桌面助手的技术选型报告 .md` — Rust vs Go technology selection report with architecture diagrams, library recommendations, security baseline, CI/CD strategy, risk matrix, and 6-week MVP timeline
