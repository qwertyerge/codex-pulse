# Codex Pulse

[English](../README.md)

[![CI](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/qwertyerge/codex-pulse/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/qwertyerge/codex-pulse)](https://github.com/qwertyerge/codex-pulse/releases/latest)
[![License](https://img.shields.io/github/license/qwertyerge/codex-pulse)](../LICENSE)

Codex Pulse 是用于观察活跃 Codex 任务的轻量 macOS 与 Windows 桌面工具。它读取本地 Codex 会话数据，显示实时运行信息，并可置顶而不干扰当前工作区。

> [!IMPORTANT]
> Codex Pulse 是独立社区项目，与 OpenAI 无隶属关系，也未获得其认可。

> [!WARNING]
> 当前发布的 macOS 构建属于实验性产物，未使用 Developer ID 签名，也未经过 Apple 公证，因此尚不支持常规的 Gatekeeper 安装流程。Windows NSIS 是已发布的未签名实验性安装程序。请勿绕过 SmartScreen、Gatekeeper 或企业安全策略；若相关策略不允许使用这些产物，请从源码构建。

## 功能

- 展示活跃会话、当前运行时长、会话时长、最近事件和最近提示。
- 将 Hook 事件与本地会话转录结果核对：清理无后续活动的遗留子任务，同时保留仍持续产生日志的会话。
- 支持 Pin/Unpin、深色/浅色/跟随系统外观，以及中英法德四种界面语言。
- 仅通过专用的 Open 操作触发 Codex deeplink。
- 以有界的本地读取方式显示周额度；Markdown 会经过净化，外部链接交给系统浏览器打开。

## 平台支持

| 环境 | 状态 |
| --- | --- |
| macOS ARM64 | 已发布的实验性 `0.3.2` DMG |
| Windows 11 x64 原生 Codex | 已发布的实验性 `0.3.2` NSIS，未签名 |
| WSL、Windows ARM64、Windows 10 | 不支持 |

Windows 支持要求使用原生 Codex 应用和原生 Windows Codex 数据；不支持也不会转换 WSL 会话或路径。该 MVP 不支持原生 Mica 或 Acrylic，保留的是现有 CSS 半透明表面。原生 Windows 安装产物和交互式桌面 UX 证据仍为 `pending-user-eyeball`；仅有已检入的工作流不能证明 GitHub Actions 已经验证 Windows。

## 自动更新与隐私

版本 `0.3.2` 尚不包含自动更新功能。手动安装首个支持更新的生产版本后，Codex Pulse 会在启动后以及此后每六小时检查一次 GitHub Releases。发现新版本时，应用会自动下载带更新签名的安装程序，并在安装和重启 Codex Pulse 前征求确认。

更新请求不会发送 Codex 转录、提示词、会话数据、额度数据或项目路径；GitHub 仍会依照其条款与隐私政策处理常规请求元数据。更新检查失败与本地会话监控相互隔离，不会影响会话监控继续正常运行。

首个支持更新的版本必须手动安装，之后才能建立自动升级路径。更新签名用于验证发布产物的完整性，但不能替代 Apple 公证或 Windows Authenticode；Gatekeeper、SmartScreen 与企业安全策略仍然适用。

## 环境要求

- 已安装 Tauri 原生构建依赖的 macOS ARM64 或 Windows 11 x64
- 所选平台对应的原生 Codex
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

在 Windows 11 x64 上，通过 PowerShell 构建未签名 NSIS 安装程序：

```powershell
pnpm install --frozen-lockfile
pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis
```

安装程序会生成在 `src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis`。缺少 WebView2 时，`downloadBootstrapper` 策略需要网络连接。该安装程序仅用于实验性测试；SmartScreen 或企业策略可能阻止运行，且不得绕过这些保护。

## 开发、测试与构建

```bash
pnpm install
pnpm tauri dev
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

`pnpm tauri build` 会在 `src-tauri/target/release/bundle/` 生成 macOS 应用和 DMG。release 构建不会自动获得 Developer ID 签名或 Apple 公证；向其他用户分发前请完成签名和公证。macOS 上仍须遵守 Gatekeeper 要求。

## Hook 监控

在应用中选择 **Enable hooks**，Pulse 会将命令合并写入 `$CODEX_HOME/hooks.json`；未设置 `CODEX_HOME` 时使用 `~/.codex/hooks.json`。该文件仍是实际配置来源，已有的其他 Hook 分组不会被覆盖。

## 目录

- `src/`：Vue 组件、composable、国际化、样式与前端测试
- `src-tauri/src/`：Tauri 命令、Codex 转录解析、监控与本地配置
- `docs/superpowers/`：设计记录与实现计划

## 许可证

本项目采用 [Apache License 2.0](../LICENSE) 许可证。
