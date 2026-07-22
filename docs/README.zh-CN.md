# Codex Pulse

[English](../README.md)

[![CI](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/qwertyerge/codex-pulse)](https://github.com/qwertyerge/codex-pulse/releases/latest)
[![License](https://img.shields.io/github/license/qwertyerge/codex-pulse)](../LICENSE)

Codex Pulse 是用于观察活跃 Codex 任务的轻量 macOS 桌面工具。它读取本地 Codex 会话数据，显示实时运行信息，并可置顶而不干扰当前工作区。

> [!IMPORTANT]
> Codex Pulse 是独立社区项目，与 OpenAI 无隶属关系，也未获得其认可。

> [!WARNING]
> 当前发布的 macOS 构建属于实验性产物，未使用 Developer ID 签名，也未经过 Apple 公证，因此尚不支持常规的 Gatekeeper 安装流程。当前推荐从源码构建。

## 功能

- 展示活跃会话、当前运行时长、会话时长、最近事件和最近提示。
- 将 Hook 事件与本地会话转录结果核对：清理无后续活动的遗留子任务，同时保留仍持续产生日志的会话。
- 支持 Pin/Unpin、深色/浅色/跟随系统外观，以及中英法德四种界面语言。
- 仅通过专用的 Open 操作触发 Codex deeplink。
- 以有界的本地读取方式显示周额度；Markdown 会经过净化，外部链接交给系统浏览器打开。

## 环境要求

- 已安装 Tauri 原生构建依赖的 macOS
- Node.js 与 pnpm
- Rust 工具链（推荐通过 `rustup` 安装）

## 从源码构建

```bash
git clone https://github.com/qwertyerge/codex-pulse.git
cd codex-pulse
pnpm install --frozen-lockfile
pnpm tauri build
```

macOS 应用和 DMG 会生成在 `src-tauri/target/release/bundle/` 下。

## 开发、测试与构建

```bash
pnpm install
pnpm tauri dev
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

`pnpm tauri build` 会在 `src-tauri/target/release/bundle/` 生成 macOS 应用和 DMG。release 构建不会自动获得 Developer ID 签名或 Apple 公证；向其他用户分发前请完成签名和公证。

## Hook 监控

在应用中选择 **Enable hooks**，Pulse 会将命令合并写入 `$CODEX_HOME/hooks.json`；未设置 `CODEX_HOME` 时使用 `~/.codex/hooks.json`。该文件仍是实际配置来源，已有的其他 Hook 分组不会被覆盖。

## 目录

- `src/`：Vue 组件、composable、国际化、样式与前端测试
- `src-tauri/src/`：Tauri 命令、Codex 转录解析、监控与本地配置
- `docs/superpowers/`：设计记录与实现计划

## 许可证

本项目采用 [Apache License 2.0](../LICENSE) 许可证。
