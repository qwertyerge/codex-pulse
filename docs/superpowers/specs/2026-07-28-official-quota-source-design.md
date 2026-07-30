# 官方动态周额度数据源设计

## 目标

让 Pulse 展示与 Codex“剩余用量”界面一致、会随账户使用动态变化的默认周额度；不把 Spark 等模型专属额度误认为默认额度，也不回退到明显过期的数据。

## 已确认事实

- Codex 的额度消耗由服务端计算，会受模型、上下文、推理、工具调用、检索和缓存等因素影响，Pulse 不能根据本地消息数或 token 精确反算。
- 当前会话使用 `gpt-5.6-sol`，但会话 JSONL 只写入了 `codex_bengalfox`（Spark）额度，因此 JSONL 不是可靠的默认额度来源。
- 官方 Codex App Server 提供稳定方法 `account/rateLimits/read` 和通知 `account/rateLimits/updated`。
- 实测 `rateLimitsByLimitId.codex` 返回已用 `5%`、窗口 `10080` 分钟及正确重置时间，与 Codex 界面一致。

官方参考：

- [Codex Pricing](https://developers.openai.com/codex/pricing)
- [Codex App Server](https://developers.openai.com/codex/app-server)

## 方案

Pulse 后端维护一个独立、隐藏、长期运行的 Codex App Server 子进程，通过 stdio JSONL 完成初始化和额度读取。

数据流：

1. Pulse 启动后解析可执行的 Codex 二进制：先验证 `PATH` 中的 `codex` 能否启动；Windows 再按修改时间从新到旧验证 `%LOCALAPPDATA%\OpenAI\Codex\bin\<版本>\codex.exe`。
2. 以无窗口方式启动 `codex app-server --listen stdio://`。
3. 发送 `initialize`、`initialized`，随后调用 `account/rateLimits/read`。
4. 从 `rateLimitsByLimitId.codex` 的 `primary`、`secondary` 中选择 `windowDurationMins == 10080` 的周额度。
5. 将 `usedPercent`、`100 - usedPercent` 和 `resetsAt` 写入现有 `WeeklyQuota`，前端契约不变。
6. Pulse 收到会话活动后以 5 秒窗口去抖读取，同时监听 `account/rateLimits/updated`；额度更新在独立后台任务中完成，不能阻塞会话状态刷新。

## 失败处理

- App Server 启动、认证、协议或读取失败时，只保留观测时间不超过 5 分钟且尚未重置的最近一次官方额度；超过时限后显示不可用。
- 没有成功的官方额度时显示“额度暂不可用”，不再用旧 JSONL 默认额度冒充当前值。
- 子进程意外退出后按退避策略重启；应用退出时回收子进程。
- Windows 子进程必须使用无控制台窗口启动标志，避免命令行弹窗。

## 范围

本次只替换额度数据源，不修改额度 UI、会话生命周期、刷新事件、Spark 展示或最大化窗口行为。旧 JSONL 额度解析代码保留以避免无关重构，但快照不再用它提供额度。

## 验证

- 单元测试覆盖协议响应解析、只选择 `codex`、只选择 10080 分钟窗口、异常响应和无可用额度。
- 后端测试覆盖官方额度优先、失败时保留最近成功值、无成功值时不可用。
- Windows 测试覆盖可执行文件发现与无窗口启动配置。
- 完整运行 Rust、前端测试和构建。
- 人工核验 Pulse 的剩余百分比和重置时间与 Codex 菜单一致，并在产生新用量后能够更新。
