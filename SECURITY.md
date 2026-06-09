# Security Policy

## Supported Versions

EchoMate is currently in early development and has not published a stable release support window yet. Security fixes are provided for the active `main` branch and the current `0.1.x` pre-stable line only.

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅        |
| < 0.1   | ❌        |

When a broader release policy is introduced, this table will be updated to list the supported release lines and end-of-support dates.

## Reporting a Vulnerability

Please do not report suspected vulnerabilities in public GitHub issues, discussions, screenshots, or logs.

Use GitHub private vulnerability reporting for this repository when available. If private vulnerability reporting is not available, contact the maintainers through the repository's preferred private contact channel and clearly mark the message as a security report.

Please include as much of the following as you can safely share:

- A concise description of the vulnerability and affected area.
- Steps to reproduce, a minimal proof of concept, or a safe demonstration.
- The affected EchoMate version, commit, operating system, and relevant configuration.
- The expected impact, including whether local chat data, provider credentials, clipboard contents, screenshots, or the local database may be exposed or modified.
- Any temporary mitigation you have identified.

Maintainers will aim to acknowledge valid reports within 7 days. After triage, maintainers will coordinate privately with the reporter, prioritize a fix according to severity, and publish a public advisory or release note after a mitigation is available when disclosure is appropriate.

If a report is declined, maintainers will explain the reason when possible, such as inability to reproduce, expected local-only behavior, duplicate report, or an issue that does not create a security boundary violation.

## Sensitive Data and Test Reports

EchoMate works close to personal messaging workflows. Security reports and reproductions must avoid real private data whenever possible.

- Use fake contacts and synthetic chat text in examples and fixtures.
- Redact personal identifiers, tokens, file paths, screenshots, clipboard contents, and logs before sharing.
- Do not attach real partner context, real contact aliases, or personal chat screenshots unless the minimum necessary evidence cannot be provided another way.
- If real data is unavoidable, share only the smallest necessary excerpt through a private channel.

## Local Data Safety

EchoMate stores local application data on the user's machine, including the local EchoMate database. Security testing and reproduction steps must use a temporary `HOME` or application data directory and must not write test data to `~/.echomate/echomate.db`.
