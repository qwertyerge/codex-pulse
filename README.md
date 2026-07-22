# Codex Pulse

[简体中文](docs/README.zh-CN.md)

[![CI](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/qwertyerge/codex-pulse)](https://github.com/qwertyerge/codex-pulse/releases/latest)
[![License](https://img.shields.io/github/license/qwertyerge/codex-pulse)](LICENSE)

Codex Pulse is a compact macOS desktop companion for watching active Codex tasks. It reads local Codex session data, shows live runtime information, and can stay above other windows without taking over the workspace.

> [!IMPORTANT]
> Codex Pulse is an independent community project. It is not affiliated with or endorsed by OpenAI.

> [!WARNING]
> Published macOS artifacts are experimental. They are not Developer ID signed or Apple notarized, so normal Gatekeeper installation is not yet supported. Build from source for the current supported path.

## Highlights

- Shows active sessions, current-run duration, session age, recent activity, and the latest prompt.
- Reconciles hook events with local session transcripts; stale unfinished descendants are removed while sessions with new runtime activity remain visible.
- Provides an always-on-top Pin/Unpin control, dark/light/system appearance, and English, Chinese, French, and German UI.
- Opens a task through its Codex deeplink only from the dedicated Open action.
- Displays a bounded, locally read weekly quota footer. Markdown content is sanitized and external links are handed to the system browser.

## Requirements

- macOS with Tauri's native build prerequisites
- Node.js and pnpm
- Rust toolchain (`rustup` recommended)

## Build from Source

```bash
git clone https://github.com/qwertyerge/codex-pulse.git
cd codex-pulse
pnpm install --frozen-lockfile
pnpm tauri build
```

The macOS app and DMG are written under `src-tauri/target/release/bundle/`.

## Develop, Test, and Build

```bash
pnpm install
pnpm tauri dev
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

`pnpm tauri build` writes the macOS app and DMG under `src-tauri/target/release/bundle/`. A release build is not automatically Developer ID signed or notarized; add Apple signing and notarization before distributing it to other users.

## Hook Monitoring

Select **Enable hooks** in the application to merge Pulse commands into `$CODEX_HOME/hooks.json`; when `CODEX_HOME` is unset, it uses `~/.codex/hooks.json`. The file remains the actual configuration source, so existing unrelated hook groups are preserved.

## Layout

- `src/` — Vue components, composables, i18n, styles, and tests
- `src-tauri/src/` — Tauri commands, Codex transcript parsing, monitoring, and local configuration
- `docs/superpowers/` — design notes and implementation plans

## License

Licensed under the [Apache License 2.0](LICENSE).
