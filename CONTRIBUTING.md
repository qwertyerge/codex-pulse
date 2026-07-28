# Contributing to Codex Pulse

Thanks for helping improve Codex Pulse. Keep changes focused, testable, and safe for people who use local Codex data.

## Before You Start

- Search existing issues before opening a new one.
- Use an issue to discuss behavior changes that affect users or project scope.
- Keep each pull request focused on one coherent change.
- Do not push directly to `main`.

## Development Setup

You need either macOS ARM64 or Windows 11 x64 with the matching Tauri prerequisites, Node.js, pnpm, and a Rust toolchain. Windows development targets native Codex; WSL is useful for issue triage but is not a supported runtime.

```bash
pnpm install
pnpm tauri dev
```

Frontend code lives in `src/`; Rust and Tauri code lives in `src-tauri/src/`.

## Making a Change

1. Create a feature branch from the current `main` branch.
2. Follow the existing Vue Composition API and Rust module patterns.
3. Add behavior-focused Vitest or Rust coverage for regressions and behavior changes.
4. Use concise Conventional Commit-style subjects such as `feat:`, `fix:`, or `docs:`.
5. Keep `README.md` and `docs/README.zh-CN.md` aligned when public behavior or setup changes.

## Verification

Run the relevant focused test while iterating, then run the complete repository checks:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

For a Windows NSIS build, run this command from PowerShell:

```powershell
pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis
```

The pull-request workflow defines the `Frontend`, `Rust`, and `Rust (Windows)` checks. A checked-in workflow is not proof that a native Windows runner has passed; include the actual run URL and result when reporting Windows verification.

### Maintainer-only macOS updater build

Maintainers with the encrypted updater key and matching macOS Keychain item
can build and verify the local updater bundle with:

```bash
scripts/build-local-updater-macos.sh
```

The script supplies the local ad-hoc signing identity before Tauri creates the
updater archive, then strictly verifies both the source app and the app inside
the archive. It also requires their executable hashes to match. This is local
updater-integrity evidence only: the app remains ad-hoc signed and is not
Developer ID signed or notarized.

## Pull Requests

Explain the user-visible behavior, link related issues, and list the commands you ran. Include before-and-after screenshots for visual changes. A pull request must pass the `Frontend`, `Rust`, and `Rust (Windows)` workflow checks before it can merge.

## Privacy and Security

Do not include local Codex transcripts, tokens, signing material, unredacted `hooks.json` content, or user-specific paths in an issue, pull request, fixture, screenshot, or log.

Sanitize diagnostics to the smallest reproduction that still demonstrates the behavior. See [SECURITY.md](SECURITY.md) for vulnerability-reporting guidance.
