# Contributing to Codex Pulse

Thanks for helping improve Codex Pulse. Keep changes focused, testable, and safe for people who use local Codex data.

## Before You Start

- Search existing issues before opening a new one.
- Use an issue to discuss behavior changes that affect users or project scope.
- Keep each pull request focused on one coherent change.
- Do not push directly to `main`.

## Development Setup

You need macOS with the Tauri prerequisites, Node.js, pnpm, and a Rust toolchain.

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

## Pull Requests

Explain the user-visible behavior, link related issues, and list the commands you ran. Include before-and-after screenshots for visual changes. A pull request must pass the required `Frontend` and `Rust` checks before it can merge.

## Privacy and Security

Do not include local Codex transcripts, tokens, signing material, unredacted `hooks.json` content, or user-specific paths in an issue, pull request, fixture, screenshot, or log.

Sanitize diagnostics to the smallest reproduction that still demonstrates the behavior. See [SECURITY.md](SECURITY.md) for vulnerability-reporting guidance.
