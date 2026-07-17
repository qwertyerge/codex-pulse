use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocaleMode {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "de")]
    German,
}

impl Default for LocaleMode {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InitializationPhase {
    Idle,
    Starting,
    DiscoveringCandidates,
    ReadingQuota,
    ReconcilingSessions,
    Complete,
    Failed,
}

impl Default for InitializationPhase {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationEvent {
    pub run_id: u64,
    pub sequence: u64,
    pub occurred_at_ms: i64,
    pub phase: InitializationPhase,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InitializationSnapshot {
    pub run_id: u64,
    pub phase: InitializationPhase,
    pub events: Vec<InitializationEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub thread_id: String,
    pub title: String,
    pub cwd: String,
    pub session_created_at_ms: i64,
    pub current_run_started_at_ms: i64,
    pub recent_event: Option<RecentEvent>,
    pub last_user_message: Option<UserMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserMessage {
    pub content: String,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyQuota {
    pub used_percent: u8,
    pub remaining_percent: u8,
    pub resets_at_ms: i64,
    #[serde(skip)]
    pub observed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentEvent {
    pub summary: String,
    pub detail: Option<String>,
    pub occurred_at_ms: i64,
    #[serde(skip)]
    pub priority: RecentEventPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecentEventPriority {
    ToolResult,
    Milestone,
    AgentMessage,
}

impl Default for RecentEventPriority {
    fn default() -> Self {
        Self::ToolResult
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringView {
    pub enabled: bool,
    pub needs_repair: bool,
    pub stale_count: usize,
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub sessions: Vec<SessionSnapshot>,
    pub weekly_quota: Option<WeeklyQuota>,
    pub is_loading: bool,
    pub initialization: InitializationSnapshot,
    pub monitoring: MonitoringView,
    pub always_on_top: bool,
    pub launch_at_login: bool,
    pub locale: LocaleMode,
    pub theme: ThemeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Active,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTitle {
    Stored(String),
    EphemeralFallback(String),
    Untitled,
}

impl ResolvedTitle {
    pub fn display(&self) -> &str {
        match self {
            Self::Stored(title) | Self::EphemeralFallback(title) => title,
            Self::Untitled => "Untitled Codex session",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMeta {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub is_subagent: bool,
    pub title: ResolvedTitle,
    pub cwd: String,
    pub session_created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub kind: LifecycleEventKind,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEventKind {
    SessionStart,
    TurnStart,
    Activity,
    TurnEnd,
    SubagentEnd,
    Abort,
}
