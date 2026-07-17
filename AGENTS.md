# Repository Guidelines

## Project Structure & Module Organization

Codex Pulse is a Vue 3 + TypeScript frontend with a Rust/Tauri backend. Frontend code lives in `src/`: UI components in `src/components/`, stateful browser logic in `src/composables/`, small pure helpers in `src/lib/`, and Vitest specs in `src/__tests__/`. Tauri code is under `src-tauri/src/`; `codex/` parses local Codex data, while `monitor.rs`, `registry.rs`, and `commands.rs` reconcile and expose application state. Keep generated files in `src-tauri/gen/`, build output in `dist/`, and `src-tauri/target/` untouched. Design records belong in `docs/superpowers/`.

## Build, Test, and Development Commands

- `pnpm install` installs frontend dependencies.
- `pnpm tauri dev` runs Vite and the desktop application together.
- `pnpm test` runs the Vitest suite once; use `pnpm test:watch` during UI work.
- `pnpm build` type-checks with `vue-tsc` and creates `dist/`.
- `cargo test --manifest-path src-tauri/Cargo.toml` runs Rust unit tests.
- `pnpm tauri build` creates an optimized desktop bundle; add `--debug` for a faster debug bundle.

## Coding Style & Naming Conventions

Use two-space indentation, double quotes, and semicolons in TypeScript/Vue files; follow the existing Composition API style in `<script setup>`. Name components in PascalCase (`SessionCard.vue`), composables as `useX` (`usePulse.ts`), helpers in camelCase, and specs as `Thing.spec.ts`. Rust follows `cargo fmt`: snake_case functions/modules, PascalCase types, and focused modules. Prefer explicit domain names such as `SessionSnapshot` over vague abbreviations.

## Testing Guidelines

Put frontend tests beside the test suite in `src/__tests__/`; use Vitest and Vue Test Utils. Add Rust tests in the owning module's `#[cfg(test)]` block. Cover behavior changes and regressions, especially transcript parsing, active-session lifecycle, timer behavior, and localized UI states. Run both frontend and Rust suites before opening a PR.

## Commit & Pull Request Guidelines

Use concise Conventional Commit-style subjects already used here: `feat:`, `fix:`, or `docs:`. Keep commits scoped and imperative, for example `fix: retain sessions with recent runtime activity`. PRs should explain user-visible behavior, list verification commands, link related issues when available, and include screenshots for visual changes. Never push directly to `main`; use a feature branch and PR.

## Security & Configuration

Never commit credentials, signing keys, or local Codex transcripts. Resolve Codex data through `CODEX_HOME` with the `~/.codex` fallback; do not hard-code a user's home directory. Treat `hooks.json` as user-managed configuration and preserve unrelated hooks when editing it.
