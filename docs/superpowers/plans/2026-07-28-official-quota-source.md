# 官方动态周额度数据源实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task by task.

**目标：** 让 Pulse 通过 Codex App Server 获取默认 `codex` 周额度，并在 Windows 上无弹窗、非阻塞地持续更新。

**架构：** 后端新增 App Server 协议解析层、隐藏子进程客户端和额度监控任务。监控任务把成功读取的官方额度写入现有 `CachedSnapshot`；会话扫描只负责会话数据，不再把 JSONL 中可能过期或属于 Spark 的额度送到界面。

**技术栈：** Rust、Tokio、Serde JSON、Tauri、Windows process flags。

## 全局约束

- 不改变前端契约、额度 UI、会话生命周期和窗口行为。
- 只接受 `limitId == "codex"` 且 `windowDurationMins == 10080` 的周额度。
- 官方额度观测超过 5 分钟或已到重置时间时，不再展示。
- 会话刷新只发送额度刷新信号，不能等待 App Server。
- Windows 子进程必须隐藏；应用退出时子进程随任务回收。
- 当前改动前基线为 83/84 个 Rust 测试通过；唯一失败为 `git::command::tests::timeout_returns_promptly_while_a_background_reaper_finishes_cleanup`，连续两次分别约 679ms、802ms，属于本任务范围外的既有计时失败。

## 任务 1：解析官方额度协议

**文件：**

- 新建：`src-tauri/src/codex/rate_limits.rs`
- 修改：`src-tauri/src/codex/mod.rs`

1. 先写失败测试，分别覆盖：
   - `account/rateLimits/read` 响应中的 `rateLimitsByLimitId.codex`；
   - 忽略 `codex_bengalfox`；
   - 只选择 10080 分钟窗口；
   - `account/rateLimits/updated` 通知；
   - 无有效额度返回 `None`。
2. 运行：
   `cargo test codex::rate_limits::tests --manifest-path src-tauri/Cargo.toml`
   并确认测试因实现缺失而失败。
3. 实现：

   ```rust
   pub fn weekly_quota_from_message(
       line: &str,
       observed_at_ms: i64,
   ) -> anyhow::Result<Option<WeeklyQuota>>;
   ```

   将 `usedPercent` 转为 0–100 的整数，将 `resetsAt` 秒转换为毫秒，保留传入的观测时间。
4. 重跑测试并提交：
   `feat(quota): parse Codex App Server rate limits`

## 任务 2：实现可测试的隐藏 App Server 客户端

**文件：**

- 新建：`src-tauri/src/codex/app_server.rs`
- 新建：`src-tauri/tests/fixtures/codex_app_server.rs`
- 修改：`src-tauri/src/codex/mod.rs`

1. 先写失败测试，覆盖：
   - 候选顺序为 PATH `codex` 优先、LocalAppData 缓存目录按修改时间从新到旧；
   - 测试夹具完成 `initialize` / `initialized` / `account/rateLimits/read` JSONL 握手；
   - 客户端能接收读取响应和主动通知。
2. 用 `rustc` 编译夹具，运行：
   `cargo test codex::app_server::tests --manifest-path src-tauri/Cargo.toml`
   并确认红灯。
3. 实现 `AppServerClient`：
   - `tokio::process::Command` 启动 `app-server --listen stdio://`；
   - stdin/stdout 管道通信，stderr 丢弃，`kill_on_drop(true)`；
   - Windows 设置 `CREATE_NO_WINDOW = 0x08000000`；
   - 每个候选必须通过初始化握手才算可用，失败后继续尝试下一个。
4. 重跑测试并提交：
   `feat(quota): add hidden Codex App Server client`

## 任务 3：实现后台刷新、去抖和重启

**文件：**

- 新建：`src-tauri/src/quota_monitor.rs`
- 修改：`src-tauri/src/lib.rs`

1. 先写失败测试，覆盖：
   - 启动后立即读取；
   - 多个会话活动信号在 5 秒内合并为一次读取；
   - 主动通知立即产出额度更新；
   - 子进程退出后按退避策略重连。
2. 运行：
   `cargo test quota_monitor::tests --manifest-path src-tauri/Cargo.toml`
   并确认红灯。
3. 实现：

   ```rust
   pub struct QuotaRefreshHandle;
   pub enum QuotaMonitorEvent {
       Observed(WeeklyQuota),
   }
   pub fn spawn_quota_monitor(
       updates: tokio::sync::mpsc::UnboundedSender<QuotaMonitorEvent>,
   ) -> QuotaRefreshHandle;
   ```

   监控任务独立处理子进程 I/O、5 秒去抖、通知和有限退避；发送端调用必须立即返回。
4. 重跑测试并提交：
   `feat(quota): refresh official limits in background`

## 任务 4：接入应用状态并切断 JSONL 额度回退

**文件：**

- 修改：`src-tauri/src/commands.rs`
- 修改：`src-tauri/src/app.rs`

1. 先修改或新增失败测试，覆盖：
   - JSONL 中的旧额度不再进入首页快照；
   - 新鲜且未重置的官方额度可见；
   - 观测超过 5 分钟的额度不可见；
   - 已到重置时间的额度不可见；
   - 会话扫描仅触发非阻塞额度刷新信号，不覆盖已缓存的官方额度。
2. 运行：
   `cargo test commands::tests --manifest-path src-tauri/Cargo.toml`
   并确认红灯。
3. 接线：
   - `AppState` 保存额度刷新句柄；
   - App setup 启动监控任务并接收 `QuotaMonitorEvent::Observed`；
   - `schedule_refresh` 只请求额度刷新并更新会话；
   - `snapshot_for_state_at` 按“5 分钟内且重置时间在未来”过滤额度；
   - 删除首页快照对 `QuotaSourceCache` 和扫描结果额度的依赖。
4. 重跑测试并提交：
   `fix(quota): use official limits for snapshots`

## 任务 5：完整验证与人工核验

1. 运行定向测试：

   ```powershell
   cargo test codex::rate_limits::tests --manifest-path src-tauri/Cargo.toml
   cargo test codex::app_server::tests --manifest-path src-tauri/Cargo.toml
   cargo test quota_monitor::tests --manifest-path src-tauri/Cargo.toml
   cargo test commands::tests --manifest-path src-tauri/Cargo.toml
   ```

2. 运行全量验证：

   ```powershell
   cargo test --manifest-path src-tauri/Cargo.toml
   pnpm test
   pnpm build
   ```

   如既有 Git 计时测试仍失败，记录实际结果，不把它误报为本任务通过。
3. 检查 `git diff --check`、工作树状态和提交范围，确保没有提交本地 `AGENTS.md`、构建产物或生成 schema。
4. 启动当前工作树版本，停在人工核验点，请用户确认：
   - Pulse 剩余百分比与 Codex 菜单一致；
   - 重置时间一致；
   - 产生新用量后能在短时间内更新；
   - 刷新时没有 CMD/PowerShell 弹窗。
5. 人工通过后，再讨论最大化按钮和暗色额度栏的后续分支，不在本任务混改。
