# Codex Pulse

[简体中文](docs/README.zh-CN.md)

[![CI](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/qwertyerge/codex-pulse)](https://github.com/qwertyerge/codex-pulse/releases/latest)
[![License](https://img.shields.io/github/license/qwertyerge/codex-pulse)](LICENSE)

Codex Pulse is a compact macOS and Windows desktop companion for watching active Codex tasks. It reads local Codex session data, shows live runtime information, and can stay above other windows without taking over the workspace.

> [!IMPORTANT]
> Codex Pulse is an independent community project. It is not affiliated with or endorsed by OpenAI.

> [!WARNING]
> Published macOS artifacts are experimental. They are not Developer ID signed or Apple notarized, so normal Gatekeeper installation is not yet supported. The Windows NSIS installer is an unsigned experimental Draft Release artifact. Do not bypass SmartScreen or enterprise security policy. Build from source for the current supported path.

## Highlights

- Shows active sessions, current-run duration, session age, recent activity, and the latest prompt.
- Reconciles hook events with local session transcripts; stale unfinished descendants are removed while sessions with new runtime activity remain visible.
- Provides an always-on-top Pin/Unpin control, dark/light/system appearance, and English, Chinese, French, and German UI.
- Opens a task through its Codex deeplink only from the dedicated Open action.
- Displays a bounded, locally read weekly quota footer. Markdown content is sanitized and external links are handed to the system browser.

## Platform Support

| Environment | Status |
| --- | --- |
| macOS ARM64 | Existing experimental DMG |
| Windows 11 x64 native Codex | `0.3.0` MVP, unsigned experimental Draft Release NSIS |
| WSL, Windows ARM64, Windows 10 | Unsupported |

Windows support requires the native Codex app and native Windows Codex data. WSL sessions and paths are not supported or translated. The MVP does not support native Mica or Acrylic; it preserves the existing CSS translucent surfaces. The native Windows installer artifact and interactive desktop UX proof remain `pending-user-eyeball`; the checked-in workflows alone are not evidence that GitHub Actions has verified Windows.

## Requirements

- macOS ARM64 or Windows 11 x64 with Tauri's native build prerequisites
- Native Codex for the selected platform
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

On Windows 11 x64, build the unsigned NSIS installer from PowerShell:

```powershell
pnpm install --frozen-lockfile
pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis
```

The installer is written under `src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis`. The `downloadBootstrapper` WebView2 policy requires network access when WebView2 is missing. Treat the installer as experimental: SmartScreen or enterprise policy may block it, and those protections must not be bypassed.

## Develop, Test, and Build

```bash
pnpm install
pnpm tauri dev
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

`pnpm tauri build` writes the macOS app and DMG under `src-tauri/target/release/bundle/`. A release build is not automatically Developer ID signed or notarized; add Apple signing and notarization before distributing it to other users. On macOS, Gatekeeper requirements still apply.

## Hook Monitoring

Select **Enable hooks** in the application to merge Pulse commands into `$CODEX_HOME/hooks.json`; when `CODEX_HOME` is unset, it uses `~/.codex/hooks.json`. The file remains the actual configuration source, so existing unrelated hook groups are preserved.

## Layout

- `src/` — Vue components, composables, i18n, styles, and tests
- `src-tauri/src/` — Tauri commands, Codex transcript parsing, monitoring, and local configuration
- `docs/superpowers/` — design notes and implementation plans

## License

Licensed under the [Apache License 2.0](LICENSE).
