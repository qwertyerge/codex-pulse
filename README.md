# Codex Pulse

[简体中文](docs/README.zh-CN.md)

Codex Pulse is a compact macOS desktop companion for watching active Codex tasks. It reads local Codex session data, shows live runtime information, and can stay above other windows without taking over the workspace.

### Highlights

- Shows active sessions, current-run duration, session age, recent activity, and the latest prompt.
- Reconciles hook events with local session transcripts; stale unfinished descendants are removed while sessions with new runtime activity remain visible.
- Provides an always-on-top Pin/Unpin control, dark/light/system appearance, and English, Chinese, French, and German UI.
- Opens a task through its Codex deeplink only from the dedicated Open action.
- Displays a bounded, locally read weekly quota footer. Markdown content is sanitized and external links are handed to the system browser.

### Requirements

- macOS with Tauri's native build prerequisites
- Node.js and pnpm
- Rust toolchain (`rustup` recommended)

### Develop, Test, and Build

```bash
pnpm install
pnpm tauri dev
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

`pnpm tauri build` writes the macOS app and DMG under `src-tauri/target/release/bundle/`. A release build is not automatically Developer ID signed or notarized; add Apple signing and notarization before distributing it to other users.

### Hook Monitoring

Select **Enable hooks** in the application to merge Pulse commands into `$CODEX_HOME/hooks.json`; when `CODEX_HOME` is unset, it uses `~/.codex/hooks.json`. The file remains the actual configuration source, so existing unrelated hook groups are preserved.

### Layout

- `src/` — Vue components, composables, i18n, styles, and tests
- `src-tauri/src/` — Tauri commands, Codex transcript parsing, monitoring, and local configuration
- `docs/superpowers/` — design notes and implementation plans
