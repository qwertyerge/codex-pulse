use std::collections::{HashMap, HashSet};

use crate::model::{
    LifecycleEvent, LifecycleEventKind, Phase, RecentEvent, ResolvedTitle, SessionSnapshot,
    ThreadMeta, UserMessage,
};

const STALE_AFTER_MS: i64 = 3_600_000;

#[derive(Debug, Clone)]
struct ThreadNode {
    thread_id: String,
    parent_thread_id: Option<String>,
    title: ResolvedTitle,
    cwd: String,
    session_created_at_ms: i64,
    current_turn_id: Option<String>,
    current_turn_started_at_ms: Option<i64>,
    last_activity_at_ms: i64,
    last_event_at_ms: i64,
    phase: Phase,
    recent_event: Option<RecentEvent>,
    last_user_message: Option<UserMessage>,
}

impl ThreadNode {
    fn placeholder(thread_id: String, at_ms: i64) -> Self {
        Self {
            thread_id,
            parent_thread_id: None,
            title: ResolvedTitle::Untitled,
            cwd: String::new(),
            session_created_at_ms: at_ms,
            current_turn_id: None,
            current_turn_started_at_ms: None,
            last_activity_at_ms: at_ms,
            last_event_at_ms: at_ms,
            phase: Phase::Idle,
            recent_event: None,
            last_user_message: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    nodes: HashMap<String, ThreadNode>,
}

impl SessionRegistry {
    pub fn apply_meta(&mut self, meta: ThreadMeta) {
        let node = self.nodes.entry(meta.thread_id.clone()).or_insert_with(|| {
            ThreadNode::placeholder(meta.thread_id.clone(), meta.session_created_at_ms)
        });

        node.parent_thread_id = meta.parent_thread_id;
        node.cwd = meta.cwd;
        node.session_created_at_ms = meta.session_created_at_ms;
        if !matches!(meta.title, ResolvedTitle::Untitled) {
            node.title = meta.title;
        }
    }

    pub fn apply_event(&mut self, event: LifecycleEvent) {
        let node = self
            .nodes
            .entry(event.thread_id.clone())
            .or_insert_with(|| {
                ThreadNode::placeholder(event.thread_id.clone(), event.occurred_at_ms)
            });

        if event.occurred_at_ms < node.last_event_at_ms {
            return;
        }

        match event.kind {
            LifecycleEventKind::SessionStart => {
                node.last_activity_at_ms = event.occurred_at_ms;
            }
            LifecycleEventKind::TurnStart => {
                let same_active_turn =
                    node.phase == Phase::Active && node.current_turn_id == event.turn_id;
                if !same_active_turn {
                    node.current_turn_id = event.turn_id;
                    node.current_turn_started_at_ms = Some(event.occurred_at_ms);
                }
                node.phase = Phase::Active;
                node.last_activity_at_ms = event.occurred_at_ms;
            }
            LifecycleEventKind::Activity => {
                if node.current_turn_id.is_none() {
                    node.current_turn_id = event.turn_id;
                    node.current_turn_started_at_ms = Some(event.occurred_at_ms);
                }
                node.phase = Phase::Active;
                node.last_activity_at_ms = event.occurred_at_ms;
            }
            LifecycleEventKind::TurnEnd
            | LifecycleEventKind::SubagentEnd
            | LifecycleEventKind::Abort => {
                let is_current_turn =
                    event.turn_id.is_none() || node.current_turn_id == event.turn_id;
                if is_current_turn {
                    node.phase = Phase::Idle;
                    node.current_turn_id = None;
                    node.current_turn_started_at_ms = None;
                    node.last_activity_at_ms = event.occurred_at_ms;
                }
            }
        }

        node.last_event_at_ms = event.occurred_at_ms;
    }

    pub fn apply_recent_event(&mut self, thread_id: &str, event: RecentEvent) {
        let node = self
            .nodes
            .entry(thread_id.to_owned())
            .or_insert_with(|| ThreadNode::placeholder(thread_id.to_owned(), event.occurred_at_ms));
        if node.recent_event.as_ref().is_none_or(|existing| {
            event.priority > existing.priority
                || (event.priority == existing.priority
                    && event.occurred_at_ms >= existing.occurred_at_ms)
        }) {
            node.recent_event = Some(event);
        }
    }

    pub fn apply_user_message(&mut self, thread_id: &str, message: UserMessage) {
        let node = self.nodes.entry(thread_id.to_owned()).or_insert_with(|| {
            ThreadNode::placeholder(thread_id.to_owned(), message.occurred_at_ms)
        });
        if node
            .last_user_message
            .as_ref()
            .is_none_or(|old| old.occurred_at_ms <= message.occurred_at_ms)
        {
            node.last_user_message = Some(message);
        }
    }

    pub fn mark_stale(&mut self, now_ms: i64) {
        for node in self.nodes.values_mut() {
            if node.phase == Phase::Active && now_ms - node.last_activity_at_ms >= STALE_AFTER_MS {
                node.phase = Phase::Stale;
            }
        }
    }

    pub fn snapshots(&self, _now_ms: i64) -> Vec<SessionSnapshot> {
        let mut root_ids = HashSet::new();
        for node in self
            .nodes
            .values()
            .filter(|node| node.phase == Phase::Active)
        {
            root_ids.insert(self.root_id(&node.thread_id));
        }

        let mut snapshots = root_ids
            .into_iter()
            .filter_map(|root_id| self.snapshot_for_root(&root_id))
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| {
            right
                .current_run_started_at_ms
                .cmp(&left.current_run_started_at_ms)
                .then_with(|| left.thread_id.cmp(&right.thread_id))
        });
        snapshots
    }

    fn root_id(&self, thread_id: &str) -> String {
        let mut current = thread_id;
        let mut visited = HashSet::new();
        while let Some(parent_id) = self
            .nodes
            .get(current)
            .and_then(|node| node.parent_thread_id.as_deref())
        {
            if !visited.insert(current.to_owned()) || !self.nodes.contains_key(parent_id) {
                break;
            }
            current = parent_id;
        }
        current.to_owned()
    }

    fn snapshot_for_root(&self, root_id: &str) -> Option<SessionSnapshot> {
        let root = self.nodes.get(root_id)?;
        let earliest_active_start = self
            .nodes
            .values()
            .filter(|node| node.phase == Phase::Active && self.root_id(&node.thread_id) == root_id)
            .filter_map(|node| node.current_turn_started_at_ms)
            .min()?;

        Some(SessionSnapshot {
            thread_id: root.thread_id.clone(),
            title: root.title.display().to_owned(),
            cwd: root.cwd.clone(),
            session_created_at_ms: root.session_created_at_ms,
            current_run_started_at_ms: earliest_active_start,
            recent_event: self
                .nodes
                .values()
                .filter(|node| self.root_id(&node.thread_id) == root_id)
                .filter_map(|node| node.recent_event.as_ref())
                .max_by_key(|event| (event.priority, event.occurred_at_ms))
                .cloned(),
            last_user_message: self
                .nodes
                .values()
                .filter(|node| self.root_id(&node.thread_id) == root_id)
                .filter_map(|node| node.last_user_message.as_ref())
                .max_by_key(|message| message.occurred_at_ms)
                .cloned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SessionRegistry;
    use crate::model::{
        LifecycleEvent, LifecycleEventKind, RecentEvent, RecentEventPriority, ResolvedTitle,
        SessionSnapshot, ThreadMeta,
    };

    fn meta(id: &str, parent: Option<&str>, created_at_ms: i64) -> ThreadMeta {
        ThreadMeta {
            thread_id: id.into(),
            parent_thread_id: parent.map(str::to_owned),
            is_subagent: parent.is_some(),
            title: if parent.is_none() {
                ResolvedTitle::Stored("Root title".into())
            } else {
                ResolvedTitle::Untitled
            },
            cwd: "/repo".into(),
            session_created_at_ms: created_at_ms,
        }
    }

    fn event(id: &str, turn: &str, kind: LifecycleEventKind, at: i64) -> LifecycleEvent {
        LifecycleEvent {
            thread_id: id.into(),
            turn_id: Some(turn.into()),
            kind,
            occurred_at_ms: at,
        }
    }

    #[test]
    fn active_descendant_keeps_only_the_root_visible() {
        let mut registry = SessionRegistry::default();
        registry.apply_meta(meta("root", None, 1_000));
        registry.apply_meta(meta("child", Some("root"), 2_000));
        registry.apply_event(event(
            "child",
            "turn-1",
            LifecycleEventKind::TurnStart,
            3_000,
        ));

        assert_eq!(
            registry.snapshots(4_000),
            vec![SessionSnapshot {
                thread_id: "root".into(),
                title: "Root title".into(),
                cwd: "/repo".into(),
                session_created_at_ms: 1_000,
                current_run_started_at_ms: 3_000,
                recent_event: None,
                last_user_message: None,
            }]
        );
    }

    #[test]
    fn earliest_active_node_defines_current_run() {
        let mut registry = SessionRegistry::default();
        registry.apply_meta(meta("root", None, 1_000));
        registry.apply_meta(meta("child", Some("root"), 2_000));
        registry.apply_event(event(
            "root",
            "turn-root",
            LifecycleEventKind::TurnStart,
            3_000,
        ));
        registry.apply_event(event(
            "child",
            "turn-child",
            LifecycleEventKind::TurnStart,
            2_500,
        ));

        assert_eq!(
            registry.snapshots(4_000)[0].current_run_started_at_ms,
            2_500
        );
    }

    #[test]
    fn active_descendant_exposes_its_newest_recent_event_on_the_root() {
        let mut registry = SessionRegistry::default();
        registry.apply_meta(meta("root", None, 1_000));
        registry.apply_meta(meta("child", Some("root"), 2_000));
        registry.apply_event(event(
            "child",
            "turn-child",
            LifecycleEventKind::TurnStart,
            3_000,
        ));
        registry.apply_recent_event(
            "root",
            RecentEvent {
                summary: "Root update".into(),
                detail: None,
                occurred_at_ms: 3_500,
                priority: RecentEventPriority::Milestone,
            },
        );
        registry.apply_recent_event(
            "child",
            RecentEvent {
                summary: "Child update".into(),
                detail: None,
                occurred_at_ms: 4_000,
                priority: RecentEventPriority::Milestone,
            },
        );

        assert_eq!(
            registry.snapshots(4_001)[0].recent_event,
            Some(RecentEvent {
                summary: "Child update".into(),
                detail: None,
                occurred_at_ms: 4_000,
                priority: RecentEventPriority::Milestone,
            })
        );
    }

    #[test]
    fn agent_message_beats_a_newer_low_signal_event_for_the_same_root() {
        let mut registry = SessionRegistry::default();
        registry.apply_meta(meta("root", None, 1_000));
        registry.apply_event(event("root", "turn", LifecycleEventKind::TurnStart, 2_000));
        registry.apply_recent_event(
            "root",
            RecentEvent {
                summary: "Implemented the monitor".into(),
                detail: None,
                occurred_at_ms: 3_000,
                priority: RecentEventPriority::AgentMessage,
            },
        );
        registry.apply_recent_event(
            "root",
            RecentEvent {
                summary: "Searched the web".into(),
                detail: None,
                occurred_at_ms: 4_000,
                priority: RecentEventPriority::ToolResult,
            },
        );

        assert_eq!(
            registry.snapshots(4_001)[0]
                .recent_event
                .as_ref()
                .map(|event| &event.summary),
            Some(&"Implemented the monitor".to_owned())
        );
    }

    #[test]
    fn duplicate_and_older_events_do_not_regress_state() {
        let mut registry = SessionRegistry::default();
        registry.apply_meta(meta("root", None, 1_000));
        let started = event("root", "turn-1", LifecycleEventKind::TurnStart, 3_000);
        registry.apply_event(started.clone());
        registry.apply_event(started);
        registry.apply_event(event(
            "root",
            "older-turn",
            LifecycleEventKind::TurnEnd,
            2_000,
        ));

        assert_eq!(registry.snapshots(4_000).len(), 1);
        assert_eq!(
            registry.snapshots(4_000)[0].current_run_started_at_ms,
            3_000
        );
    }

    #[test]
    fn stale_node_disappears_and_new_activity_recovers_it() {
        let mut registry = SessionRegistry::default();
        registry.apply_meta(meta("root", None, 1_000));
        registry.apply_event(event(
            "root",
            "turn-1",
            LifecycleEventKind::TurnStart,
            2_000,
        ));
        registry.mark_stale(3_602_000);
        assert!(registry.snapshots(3_602_000).is_empty());
        registry.apply_event(event(
            "root",
            "turn-1",
            LifecycleEventKind::Activity,
            3_603_000,
        ));
        assert_eq!(registry.snapshots(3_603_000).len(), 1);
    }
}
