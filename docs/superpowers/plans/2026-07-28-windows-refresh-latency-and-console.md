# Windows 刷新延迟与弹窗实施计划

> **执行要求：** 使用 `superpowers:executing-plans`，逐项 TDD；每个任务完成后单独提交。

**目标：** 不丢刷新通知，5 秒稳定期到点自动更新，并隐藏 Pulse 启动的 Windows Git 控制台。

**实现位置：** 调度和事件截止时间放在 `src-tauri/src/commands.rs`；进程标志放在
`src-tauri/src/git/command.rs`。不新增依赖，不改变 macOS 和额度语义。

## 约束

- 保留 60 秒兜底刷新和 5 秒事件稳定规则。
- 不把方案 B、额度或暗色页脚混入本刷新/控制台 cohort；Windows 0.3.0 验收范围内的窗口最大化仍保留，但不在此 cohort 实施。
- 近期事件合并使用最早截止时间，避免后续事件延迟已经等待的刷新；命令 Hook 的 transcript settle 使用最新截止时间，保证最后一次写入后保留完整静默期。
- 现有 Windows 基线失败单独报告，不放宽断言。

### 任务 1：三态刷新调度

**文件：** `src-tauri/src/commands.rs`

- [ ] 新增纯逻辑 `RefreshGate(AtomicU8)`，状态为 `IDLE`、`RUNNING`、
  `RUNNING_PENDING`；`request() -> bool` 表示是否启动，`complete() -> bool`
  表示是否立即补一次，`is_running() -> bool` 提供加载状态。
- [ ] 先写测试：空闲启动、运行中排队、多次通知只排一次、完成后补一次并回到空闲。
- [ ] 运行：
  `cargo test --manifest-path src-tauri/Cargo.toml commands::tests::refresh_gate -- --nocapture`
  并确认测试先失败。
- [ ] 用 `start_refresh(app)` 承担实际扫描；`schedule_refresh` 只调用
  `request()`，完成路径调用 `complete()`，需要时直接再次调用 `start_refresh`。
- [ ] 再运行聚焦测试并提交：
  `fix: retain refresh notifications received during scans`

### 任务 2：事件截止时间唤醒

**文件：** `src-tauri/src/commands.rs`

- [ ] 把 `coalesce_recent_events(...)` 改为返回 `Option<i64>`，内容是仍被暂存
  候选事件的最早到期毫秒。
- [ ] 先扩展现有测试：14,999ms 返回 15,000ms；15,000ms 采用新事件并返回
  `None`。
- [ ] 运行现有聚焦测试并确认新断言先失败。
- [ ] 新增 `DeadlineWakeup(AtomicI64)`：`arm_earliest(i64) -> bool`、
  `claim(i64) -> bool`、`clear()`；先测试较早截止时间替换、较晚时间合并和
  旧定时器无法 `claim`。
- [ ] 放入 `AppState`；只有 `arm_earliest` 成功时创建定时器，到点且
  `claim` 成功时调用同一 `schedule_refresh`，没有暂存事件时 `clear()`。
- [ ] 运行 `commands::tests` 并提交：
  `fix: release coalesced events at their deadline`

### 任务 3：隐藏 Windows Git 子进程

**文件：** `src-tauri/src/git/command.rs`

- [ ] 新增 `configure_process(&mut Command)`；Windows 使用
  `CommandExt::creation_flags(0x08000000)`，其他平台为空操作。
- [ ] 先写 Windows 聚焦测试，固定并验证 `CREATE_NO_WINDOW` 标志值。
- [ ] 让 `ProcessGitRunner::run` 在 `spawn()` 前调用配置函数。
- [ ] 运行 `git::command::tests`；保留并报告当前超时测试的真实结果，不改阈值。
- [ ] 提交：`fix: hide Git child windows on Windows`

### 任务 4：耗时诊断与总体验证

**文件：** `src-tauri/src/commands.rs`

- [ ] 仅在 debug 构建用 `Instant` 记录额度、会话、Git 和总耗时；日志只含阶段与毫秒。
- [ ] 运行 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`。
- [ ] 运行 `cargo test --manifest-path src-tauri/Cargo.toml`、`pnpm test`、
  `pnpm build`，区分新增失败与已知基线失败。
- [ ] 提交：`chore: add refresh timing diagnostics`
- [ ] 保持开发版运行，通知用户执行设计文档中的四项 Windows 人工验收。
