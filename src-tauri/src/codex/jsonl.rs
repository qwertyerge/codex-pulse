use anyhow::{anyhow, Result};
use chrono::DateTime;
use serde_json::Value;

use crate::model::{
    LifecycleEvent, LifecycleEventKind, RecentEvent, RecentEventPriority, ResolvedTitle,
    ThreadMeta, UserMessage, WeeklyQuota,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptEvent {
    pub kind: LifecycleEventKind,
    pub turn_id: Option<String>,
    pub occurred_at_ms: i64,
}

impl TranscriptEvent {
    pub fn bind(self, thread_id: impl Into<String>) -> LifecycleEvent {
        LifecycleEvent {
            thread_id: thread_id.into(),
            turn_id: self.turn_id,
            kind: self.kind,
            occurred_at_ms: self.occurred_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedRecord {
    Meta(ThreadMeta),
    Lifecycle(TranscriptEvent),
    LifecycleAndRecent(TranscriptEvent, RecentEvent),
    Recent(RecentEvent),
    UserMessage(UserMessage),
    WeeklyQuota(WeeklyQuota),
    Ignored,
}

pub fn parse_line(line: &str) -> Result<Option<ParsedRecord>> {
    if line.trim().is_empty() {
        return Ok(None);
    }

    let value: Value = serde_json::from_str(line)?;
    let record_type = value.get("type").and_then(Value::as_str);
    let payload = value.get("payload").unwrap_or(&Value::Null);

    match record_type {
        Some("session_meta") => parse_meta(payload).map(Some),
        Some("event_msg") => parse_event(payload, timestamp_ms(&value)?).map(Some),
        Some("response_item") => Ok(Some(ParsedRecord::Ignored)),
        _ => Ok(Some(ParsedRecord::Ignored)),
    }
}

fn parse_meta(payload: &Value) -> Result<ParsedRecord> {
    let thread_id = required_string(payload, "id")?;
    let cwd = required_string(payload, "cwd")?;
    let created_at = payload
        .get("timestamp")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("session metadata is missing timestamp"))?;

    let parent_thread_id = payload
        .pointer("/source/subagent/thread_spawn/parent_thread_id")
        .and_then(Value::as_str)
        .map(str::to_owned);

    Ok(ParsedRecord::Meta(ThreadMeta {
        thread_id,
        parent_thread_id: parent_thread_id.clone(),
        is_subagent: parent_thread_id.is_some(),
        title: ResolvedTitle::Untitled,
        cwd,
        session_created_at_ms: parse_timestamp_ms(created_at)?,
    }))
}

fn parse_event(payload: &Value, occurred_at_ms: i64) -> Result<ParsedRecord> {
    let Some(kind) = payload.get("type").and_then(Value::as_str) else {
        return Ok(ParsedRecord::Ignored);
    };

    if let Some(summary) = (kind == "agent_message")
        .then(|| payload.get("message").and_then(Value::as_str))
        .flatten()
    {
        return Ok(ParsedRecord::Recent(RecentEvent {
            summary: compact_summary(summary),
            detail: Some(compact_detail(summary)),
            occurred_at_ms,
            priority: RecentEventPriority::AgentMessage,
        }));
    }
    if let Some(message) = (kind == "user_message")
        .then(|| payload.get("message").and_then(Value::as_str))
        .flatten()
    {
        return Ok(ParsedRecord::UserMessage(UserMessage {
            content: compact_detail(message),
            occurred_at_ms,
        }));
    }

    if kind == "token_count" {
        return Ok(payload
            .get("rate_limits")
            .and_then(|rate_limits| parse_weekly_quota(rate_limits, occurred_at_ms))
            .map_or(ParsedRecord::Ignored, ParsedRecord::WeeklyQuota));
    }

    if kind == "patch_apply_end" {
        let count = payload
            .get("changes")
            .and_then(Value::as_object)
            .map_or(0, |changes| changes.len());
        let summary = if payload.get("success").and_then(Value::as_bool) == Some(false) {
            "Patch application failed".to_owned()
        } else {
            format!(
                "Updated {count} {}",
                if count == 1 { "file" } else { "files" }
            )
        };
        return Ok(ParsedRecord::Recent(RecentEvent {
            summary,
            detail: None,
            occurred_at_ms,
            priority: RecentEventPriority::Milestone,
        }));
    }

    if kind == "web_search_end" {
        let query = payload.get("query").and_then(Value::as_str);
        return Ok(ParsedRecord::Recent(RecentEvent {
            summary: query.map_or_else(
                || "Completed web search".to_owned(),
                |query| format!("Searched: {}", compact_summary(query)),
            ),
            detail: None,
            occurred_at_ms,
            priority: RecentEventPriority::ToolResult,
        }));
    }

    if kind == "mcp_tool_call_end" {
        let tool = payload.pointer("/invocation/tool").and_then(Value::as_str);
        return Ok(ParsedRecord::Recent(RecentEvent {
            summary: tool.map_or_else(
                || "Completed tool call".to_owned(),
                |tool| format!("Completed {tool}"),
            ),
            detail: None,
            occurred_at_ms,
            priority: RecentEventPriority::ToolResult,
        }));
    }

    let kind = match kind {
        "task_started" => LifecycleEventKind::TurnStart,
        "task_complete" => LifecycleEventKind::TurnEnd,
        _ => return Ok(ParsedRecord::Ignored),
    };

    let lifecycle = TranscriptEvent {
        kind,
        turn_id: payload
            .get("turn_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        occurred_at_ms,
    };
    if matches!(lifecycle.kind, LifecycleEventKind::TurnEnd) {
        if let Some(message) = payload.get("last_agent_message").and_then(Value::as_str) {
            return Ok(ParsedRecord::LifecycleAndRecent(
                lifecycle,
                RecentEvent {
                    summary: compact_summary(message),
                    detail: Some(compact_detail(message)),
                    occurred_at_ms,
                    priority: RecentEventPriority::AgentMessage,
                },
            ));
        }
    }
    Ok(ParsedRecord::Lifecycle(lifecycle))
}

fn parse_weekly_quota(rate_limits: &Value, observed_at_ms: i64) -> Option<WeeklyQuota> {
    if rate_limits.get("limit_id").and_then(Value::as_str) != Some("codex") {
        return None;
    }

    let bucket = ["primary", "secondary"].into_iter().find_map(|slot| {
        rate_limits
            .get(slot)
            .filter(|bucket| bucket.get("window_minutes").and_then(Value::as_i64) == Some(10_080))
    })?;
    let used_percent = bucket
        .get("used_percent")
        .and_then(Value::as_f64)?
        .round()
        .clamp(0.0, 100.0) as u8;
    let resets_at_ms = bucket
        .get("resets_at")
        .and_then(Value::as_i64)?
        .saturating_mul(1_000);

    Some(WeeklyQuota {
        used_percent,
        remaining_percent: 100 - used_percent,
        resets_at_ms,
        observed_at_ms,
    })
}

fn compact_summary(value: &str) -> String {
    const LIMIT: usize = 120;
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= LIMIT {
        return compact;
    }
    format!("{}…", compact.chars().take(LIMIT - 1).collect::<String>())
}

fn compact_detail(value: &str) -> String {
    const LIMIT: usize = 2_000;
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.chars().count() <= LIMIT {
        return normalized;
    }
    format!(
        "{}…",
        normalized.chars().take(LIMIT - 1).collect::<String>()
    )
}

fn timestamp_ms(value: &Value) -> Result<i64> {
    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("record is missing timestamp"))?;
    parse_timestamp_ms(timestamp)
}

fn parse_timestamp_ms(value: &str) -> Result<i64> {
    Ok(DateTime::parse_from_rfc3339(value)?.timestamp_millis())
}

fn required_string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("session metadata is missing {key}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_line, ParsedRecord};
    use crate::model::{LifecycleEventKind, RecentEventPriority};

    const ROOT_ID: &str = "00000000-0000-4000-8000-000000000001";

    #[test]
    fn parses_root_metadata_and_task_start() {
        let metadata = format!(
            r#"{{"timestamp":"2026-07-16T07:00:00Z","type":"session_meta","payload":{{"id":"{ROOT_ID}","timestamp":"2026-07-16T07:00:00Z","cwd":"/repo","source":"cli"}}}}"#
        );
        let event = r#"{"timestamp":"2026-07-16T07:01:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-root"}}"#;

        let ParsedRecord::Meta(meta) = parse_line(&metadata).unwrap().unwrap() else {
            panic!("expected session metadata")
        };
        assert_eq!(meta.thread_id, ROOT_ID);
        assert_eq!(meta.cwd, "/repo");
        assert_eq!(meta.session_created_at_ms, 1_784_185_200_000);

        let ParsedRecord::Lifecycle(started) = parse_line(event).unwrap().unwrap() else {
            panic!("expected task start")
        };
        assert_eq!(started.kind, LifecycleEventKind::TurnStart);
        assert_eq!(started.turn_id.as_deref(), Some("turn-root"));
        assert_eq!(started.occurred_at_ms, 1_784_185_260_000);
    }

    #[test]
    fn parses_subagent_parent_and_ignores_unknown_records() {
        let child = r#"{"timestamp":"2026-07-16T07:00:00Z","type":"session_meta","payload":{"id":"00000000-0000-4000-8000-000000000002","timestamp":"2026-07-16T07:00:00Z","cwd":"/repo","source":{"subagent":{"thread_spawn":{"parent_thread_id":"00000000-0000-4000-8000-000000000001"}}}}}"#;
        let unknown = r#"{"type":"response_item","payload":{"type":"message"}}"#;

        let ParsedRecord::Meta(meta) = parse_line(child).unwrap().unwrap() else {
            panic!("expected subagent metadata")
        };
        assert_eq!(meta.parent_thread_id.as_deref(), Some(ROOT_ID));
        assert!(meta.is_subagent);
        assert!(matches!(
            parse_line(unknown).unwrap(),
            Some(ParsedRecord::Ignored)
        ));
    }

    #[test]
    fn rejects_malformed_json_without_aborting_the_stream() {
        assert!(parse_line("{not json").is_err());
    }

    #[test]
    fn extracts_an_agent_message_as_a_recent_display_event() {
        let line = r#"{"timestamp":"2026-07-17T07:03:00Z","type":"event_msg","payload":{"type":"agent_message","message":"Finished the database migration"}}"#;

        let ParsedRecord::Recent(event) = parse_line(line).unwrap().unwrap() else {
            panic!("expected a recent display event");
        };

        assert_eq!(event.summary, "Finished the database migration");
        assert_eq!(event.occurred_at_ms, 1_784_271_780_000);
        assert_eq!(event.priority, RecentEventPriority::AgentMessage);
    }

    #[test]
    fn preserves_markdown_line_structure_for_expanded_details() {
        let line = r#"{"timestamp":"2026-07-17T07:03:00Z","type":"event_msg","payload":{"type":"user_message","message":"Before\n\n- first\n- second\n\n[Reference](https://example.com)"}}"#;

        let ParsedRecord::UserMessage(message) = parse_line(line).unwrap().unwrap() else {
            panic!("expected a user message");
        };

        assert!(message.content.contains("\n\n- first\n- second\n\n"));
        assert!(message.content.contains("[Reference](https://example.com)"));
    }

    #[test]
    fn ignores_bare_exec_completion_and_summarizes_patch_application() {
        let exec = r#"{"timestamp":"2026-07-17T07:03:00Z","type":"response_item","payload":{"type":"custom_tool_call","name":"exec","status":"completed"}}"#;
        let patch = r#"{"timestamp":"2026-07-17T07:04:00Z","type":"event_msg","payload":{"type":"patch_apply_end","success":true,"changes":{"src/App.vue":"updated","src/styles.css":"updated"}}}"#;

        assert!(matches!(
            parse_line(exec).unwrap(),
            Some(ParsedRecord::Ignored)
        ));
        let ParsedRecord::Recent(event) = parse_line(patch).unwrap().unwrap() else {
            panic!("expected patch summary");
        };
        assert_eq!(event.summary, "Updated 2 files");
        assert_eq!(event.priority, RecentEventPriority::Milestone);
    }

    #[test]
    fn parses_default_codex_weekly_quota_from_primary_or_secondary_window() {
        let primary = r#"{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":81.0,"window_minutes":10080,"resets_at":1784870653},"secondary":null}}}"#;
        let secondary = r#"{"timestamp":"2026-07-17T12:01:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":12.0,"window_minutes":300,"resets_at":1784800000},"secondary":{"used_percent":22.0,"window_minutes":10080,"resets_at":1784871000}}}}"#;

        for (line, used, remaining) in [(primary, 81, 19), (secondary, 22, 78)] {
            let ParsedRecord::WeeklyQuota(quota) = parse_line(line).unwrap().unwrap() else {
                panic!("expected weekly quota");
            };
            assert_eq!(quota.used_percent, used);
            assert_eq!(quota.remaining_percent, remaining);
        }
    }

    #[test]
    fn ignores_weekly_quota_that_is_not_the_default_codex_limit() {
        for limit_id in [Some("codex_bengalfox"), Some("premium"), None] {
            let limit_id = limit_id
                .map(|value| format!(r#""limit_id":"{value}","#))
                .unwrap_or_default();
            let line = format!(
                r#"{{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{{limit_id}"primary":{{"used_percent":100.0,"window_minutes":10080,"resets_at":1784870653}}}}}}}}"#
            );

            assert!(matches!(
                parse_line(&line).unwrap(),
                Some(ParsedRecord::Ignored)
            ));
        }
    }

    #[test]
    fn ignores_rate_limit_buckets_that_are_not_weekly() {
        let line = r#"{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":81.0,"window_minutes":300,"resets_at":1784870653}}}}"#;

        assert!(matches!(
            parse_line(line).unwrap(),
            Some(ParsedRecord::Ignored)
        ));
    }
}
