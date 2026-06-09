# EchoMate Agent Rules

- Never run e2e mock provider flows against the user's real EchoMate profile.
- For tests, set `HOME` or the app data directory to a temporary directory before launching Tauri.
- Do not use the user's real contact aliases, partner context, or personal chat screenshots as test fixtures.
- Test data must use fake contacts and must not be saved to `~/.echomate/echomate.db`.
- If a local manual test needs persistence, create a disposable contact and delete its context before finishing.
