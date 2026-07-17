# Codex Pulse

[English](#english) · [中文](#中文)

## English

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

## 中文

Codex Pulse 是用于观察活跃 Codex 任务的轻量 macOS 桌面工具。它读取本地 Codex 会话数据，显示实时运行信息，并可置顶而不干扰当前工作区。

### 功能

- 展示活跃会话、当前运行时长、会话时长、最近事件和最近提示。
- 将 Hook 事件与本地会话转录结果核对：清理无后续活动的遗留子任务，同时保留仍持续产生日志的会话。
- 支持 Pin/Unpin、深色/浅色/跟随系统外观，以及中英法德四种界面语言。
- 仅通过专用的 Open 操作触发 Codex deeplink。
- 以有界的本地读取方式显示周额度；Markdown 会经过净化，外部链接交给系统浏览器打开。

### 环境要求

- 已安装 Tauri 原生构建依赖的 macOS
- Node.js 与 pnpm
- Rust 工具链（推荐通过 `rustup` 安装）

### 开发、测试与构建

```bash
pnpm install
pnpm tauri dev
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

`pnpm tauri build` 会在 `src-tauri/target/release/bundle/` 生成 macOS 应用和 DMG。release 构建不会自动获得 Developer ID 签名或 Apple 公证；向其他用户分发前请完成签名和公证。

### Hook 监控

在应用中选择 **Enable hooks**，Pulse 会将命令合并写入 `$CODEX_HOME/hooks.json`；未设置 `CODEX_HOME` 时使用 `~/.codex/hooks.json`。该文件仍是实际配置来源，已有的其他 Hook 分组不会被覆盖。

### 目录

- `src/`：Vue 组件、composable、国际化、样式与前端测试
- `src-tauri/src/`：Tauri 命令、Codex 转录解析、监控与本地配置
- `docs/superpowers/`：设计记录与实现计划
