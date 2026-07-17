use serde::{Deserialize, Serialize};

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
    pub is_loading: bool,
    pub monitoring: MonitoringView,
    pub always_on_top: bool,
    pub launch_at_login: bool,
    pub locale: String,
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
