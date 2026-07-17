# Codex Pulse 高信号最近事件 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用可读进展替代裸工具名，并支持卡片事件展开冻结。

**Architecture:** JSONL 层把结构化事件转为带优先级的摘要；注册表在同一根会话中按优先级和时间选择一条。Vue 卡片保存展开时的事件快照，避免阅读时被刷新替换。

**Tech Stack:** Rust、Tauri、Vue 3、Vitest。

## Global Constraints

- 仅显示一条事件，后端继续保留每根会话 5 秒合并窗口。
- 不展示 `token_count`、reasoning 或裸 `exec` 完成。
- 相对时间全部小写。

---

### Task 1: 高信号 JSONL 摘要

**Files:**
- Modify: `src-tauri/src/model.rs`, `src-tauri/src/codex/jsonl.rs`, `src-tauri/src/registry.rs`
- Test: `src-tauri/src/codex/jsonl.rs`, `src-tauri/src/registry.rs`

- [ ] 写失败测试：`agent_message` 的文本优先于较新的低优先级补丁/搜索事件，且裸 `custom_tool_call exec` 被忽略。
- [ ] 运行 `cargo test high_signal -- --nocapture`，确认失败。
- [ ] 新增事件优先级；归纳代理消息、补丁文件数、搜索查询、MCP 工具完成、任务/子代理完成；移除 `response_item` 工具名展示。
- [ ] 运行 `cargo test high_signal`，确认通过。

### Task 2: 展开冻结交互

**Files:**
- Modify: `src/components/SessionCard.vue`, `src/lib/duration.ts`, `src/styles.css`
- Test: `src/__tests__/SessionCard.spec.ts`, `src/__tests__/duration.spec.ts`

- [ ] 写失败测试：时间文案为小写，点击事件行可展开；当传入新事件时展开状态仍显示旧快照，收起后显示新事件。
- [ ] 运行 `pnpm test -- --run src/__tests__/SessionCard.spec.ts`，确认失败。
- [ ] 在卡片中维护展开事件快照，增加可访问的展开按钮、完整文本和视觉状态。
- [ ] 运行对应 Vitest，确认通过。

### Task 3: 回归、打包与实际窗口验证

**Files:**
- Modify: none expected

- [ ] 运行 `cargo fmt --check && cargo test && cargo clippy -- -D warnings`（在 `src-tauri`）。
- [ ] 运行 `pnpm test -- --run && pnpm build && pnpm tauri build --debug`（项目根目录）。
- [ ] 将新 `.app` 安装到 `/Applications/Codex Pulse.app`，检查真实窗口显示高信号文本、全小写 `ago` 和展开冻结行为。
