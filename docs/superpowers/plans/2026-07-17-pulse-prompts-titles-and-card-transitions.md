# Codex Pulse 用户消息、标题与任务卡动画 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 显示 Codex 实际短标题、最近用户消息、受限默认最大化和任务卡进出动画。

**Architecture:** `session_index.jsonl` 作为标题覆盖源；JSONL 扫描将 `user_message` 绑定到根会话；Vue 卡片独立冻结 Last prompt 与 Recent。列表 TransitionGroup 只观察根会话集合变化。

**Tech Stack:** Rust、Tauri、Vue 3、Vitest。

## Global Constraints

- SQLite 标题仅为 `session_index.jsonl.thread_name` 缺失时的回退。
- 最大化不得突破现有 480 宽和当前显示器工作区高度。
- 任务卡进出 180ms；常规内容刷新不得重新播放动画。
- `prefers-reduced-motion` 禁用动画。

---

### Task 1: 标题索引与最近用户消息

**Files:**
- Create: `src-tauri/src/codex/session_index.rs`
- Modify: `src-tauri/src/model.rs`, `src-tauri/src/codex/jsonl.rs`, `src-tauri/src/codex/discovery.rs`, `src-tauri/src/registry.rs`, `src-tauri/src/monitor.rs`
- Test: 相同 Rust 模块中的 `#[cfg(test)]`

- [ ] 写失败测试：索引标题覆盖 SQLite 标题；解析最近 `user_message`；根会话聚合子会话最后一条用户消息。
- [ ] 运行对应 `cargo test`，确认失败。
- [ ] 实现 `lookup_thread_names(codex_home, thread_ids)`、`LastUserMessage` 与注册表根会话归并。
- [ ] 运行对应 `cargo test`，确认通过。

### Task 2: 窗口最大化

**Files:**
- Modify: `src-tauri/src/app.rs`
- Test: `src-tauri/src/app.rs`

- [ ] 写辅助函数测试：屏幕工作区产生的最大尺寸包含 480 宽与 16px 安全边距。
- [ ] 在创建窗口后设置最大尺寸再调用 `window.maximize()`。
- [ ] 运行 `cargo test`，确认通过。

### Task 3: Last prompt 与任务卡动画

**Files:**
- Modify: `src/types.ts`, `src/App.vue`, `src/components/SessionCard.vue`, `src/styles.css`
- Test: `src/__tests__/SessionCard.spec.ts`, `src/__tests__/App.spec.ts`

- [ ] 写失败测试：Last prompt 位于 Recent 前、展开冻结并在收起后刷新；标题有完整 tooltip；列表使用 TransitionGroup。
- [ ] 运行 `pnpm test -- --run`，确认失败。
- [ ] 实现独立的 Last prompt 冻结状态、标题 tooltip 与 `task-card` 进出样式。
- [ ] 运行 Vitest，确认通过。

### Task 4: 全量验证与安装检查

- [ ] 在 `src-tauri` 运行 `cargo fmt --check && cargo test && cargo clippy -- -D warnings`。
- [ ] 在项目根目录运行 `pnpm test -- --run && pnpm build && pnpm tauri build --debug`。
- [ ] 停止已运行的 CodexPulse，使用 `ditto` 更新 `/Applications/Codex Pulse.app` 后启动。
- [ ] 用 Computer Use 检查短标题、Last prompt 展开、窗口最大尺寸与任务卡动画。
