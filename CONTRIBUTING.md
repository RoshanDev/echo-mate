# Contributing to EchoMate

Thank you for helping improve EchoMate. This guide explains how to set up the project, make changes safely, and submit contributions.

## Before You Start

- Read the [README](README.md) to understand the product scope and local development workflow.
- Review the [Code of Conduct](CODE_OF_CONDUCT.md).
- Do not use real personal chat data, contact aliases, partner context, or screenshots as fixtures or test data.
- Never run e2e mock provider flows against a real EchoMate profile.

## Development Setup

Install JavaScript dependencies:

```bash
npm install
```

Run the desktop app during development:

```bash
npx tauri dev
```

EchoMate is a Tauri desktop app, not an HTTP service. Prefer `npx tauri dev` for local validation because direct binary execution can miss Tauri dev resource context.

## Testing and Validation

Useful checks include:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
node --check frontend/main.js
node --check frontend/settings.js
node --check tests/windows-e2e-runner.mjs
node --check tests/macos-smoke-runner.mjs
```

When launching Tauri from tests, set `HOME` or the app data directory to a temporary directory. Test data must use fake contacts and must not be saved to `~/.echomate/echomate.db`.

The macOS smoke test is available with:

```bash
npm run test:e2e:macos
```

Only run this against disposable app state. If a local manual test needs persistence, create a disposable contact and delete its context before finishing.

## Pull Request Guidelines

1. Create a focused branch for your change.
2. Keep changes small and scoped to a single concern when possible.
3. Update documentation when behavior, setup, or user-facing workflows change.
4. Add or update tests for behavioral changes.
5. Run the relevant checks before opening a pull request.
6. In the pull request description, summarize what changed and list the commands you ran.

## Coding Notes

- Follow the existing Rust, JavaScript, HTML, and CSS style in the touched files.
- Avoid storing secrets, personal data, or real chat material in the repository.
- Keep provider-dependent behavior easy to test with mock or disposable state.
- For UI changes, include screenshots when they materially change the runnable app.

## Reporting Bugs or Requesting Features

Use the issue templates under `.github/ISSUE_TEMPLATE/` when available. Include clear reproduction steps, expected behavior, actual behavior, environment details, and logs or screenshots with sensitive data redacted.
