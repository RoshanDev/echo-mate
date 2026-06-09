# Security Policy

## Supported Versions

EchoMate is in active early development. Security fixes are applied to the default branch unless maintainers publish a release support policy.

## Reporting a Vulnerability

Please do not report security vulnerabilities in public issues.

Instead, use GitHub's private vulnerability reporting or another private maintainer contact channel configured for this repository. Include:

- A concise description of the vulnerability.
- Steps to reproduce or a proof of concept, if safe to share.
- Affected platforms and versions.
- Potential impact.
- Any suggested mitigation.

Maintainers will acknowledge reports as soon as practical and will coordinate fixes privately before public disclosure when appropriate.

## Sensitive Data Guidelines

EchoMate works near personal messaging workflows, so security reports and reproduction materials must avoid real private chat content whenever possible.

- Use fake contacts and synthetic chat text in examples.
- Redact personal identifiers, tokens, file paths, screenshots, and logs before sharing.
- Do not attach real partner context, contact aliases, or personal chat screenshots.
- If real data is unavoidable to demonstrate impact, share the minimum necessary information through a private channel only.

## Local Data

By default, EchoMate stores local application data under the user's app data location, including the local EchoMate database. Tests and security reproductions should use a temporary `HOME` or app data directory and must not write to `~/.echomate/echomate.db`.
