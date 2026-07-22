# Security Policy

## Supported Versions

Security fixes target the current `main` branch and the latest published release. Older releases do not receive a guaranteed security-fix backport.

## Reporting a Vulnerability

Open a [public GitHub issue](https://github.com/qwertyerge/codex-pulse/issues/new/choose) with a minimal, fully redacted reproduction. This project does not currently offer a private reporting channel.

Before submitting, remove all local Codex transcripts, tokens, credentials, signing material, complete `hooks.json` content, private repository names, and user-specific paths. Do not attach raw session data. Replace sensitive values with stable placeholders and include only the minimum sanitized diagnostic detail needed to reproduce the problem.

Describe the affected version, macOS version and architecture, impact, reproduction steps, and any mitigation you have already tested. A maintainer will triage the issue in public and may ask for a safer reduced reproduction.
