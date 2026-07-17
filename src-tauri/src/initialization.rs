use std::collections::VecDeque;

use crate::model::{InitializationEvent, InitializationPhase, InitializationSnapshot};

pub const INITIALIZATION_PROGRESS_EVENT: &str = "initialization-progress";
pub const INITIALIZATION_EVENT_CAPACITY: usize = 120;

#[derive(Default)]
pub struct InitializationFeed {
    phase: InitializationPhase,
    run_id: u64,
    next_sequence: u64,
    events: VecDeque<InitializationEvent>,
}

impl InitializationFeed {
    pub fn begin(&mut self, now_ms: i64) -> InitializationEvent {
        self.phase = InitializationPhase::Starting;
        self.run_id += 1;
        self.events.clear();
        self.record(
            now_ms,
            InitializationPhase::Starting,
            "Starting Codex Pulse reconciliation".into(),
        )
    }

    pub fn record(
        &mut self,
        now_ms: i64,
        phase: InitializationPhase,
        summary: String,
    ) -> InitializationEvent {
        self.next_sequence += 1;
        self.phase = phase;
        let event = InitializationEvent {
            run_id: self.run_id,
            sequence: self.next_sequence,
            occurred_at_ms: now_ms,
            phase,
            summary,
        };
        self.events.push_back(event.clone());
        while self.events.len() > INITIALIZATION_EVENT_CAPACITY {
            self.events.pop_front();
        }
        event
    }

    pub fn snapshot(&self) -> InitializationSnapshot {
        InitializationSnapshot {
            run_id: self.run_id,
            phase: self.phase,
            events: self.events.iter().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InitializationFeed, INITIALIZATION_EVENT_CAPACITY};
    use crate::model::InitializationPhase;

    #[test]
    fn keeps_only_the_newest_events_in_sequence_order() {
        let mut feed = InitializationFeed::default();
        feed.begin(1);
        for index in 0..=INITIALIZATION_EVENT_CAPACITY {
            feed.record(
                2 + index as i64,
                InitializationPhase::ReadingQuota,
                format!("event {index}"),
            );
        }

        let snapshot = feed.snapshot();
        assert_eq!(snapshot.events.len(), INITIALIZATION_EVENT_CAPACITY);
        assert_eq!(snapshot.events.first().unwrap().summary, "event 1");
        assert_eq!(
            snapshot.events.last().unwrap().summary,
            format!("event {INITIALIZATION_EVENT_CAPACITY}")
        );
        assert!(snapshot
            .events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence));
        assert_eq!(snapshot.run_id, 1);
    }

    #[test]
    fn begin_clears_prior_run_and_marks_starting() {
        let mut feed = InitializationFeed::default();
        feed.record(1, InitializationPhase::Failed, "old failure".into());
        feed.begin(2);

        let snapshot = feed.snapshot();
        assert_eq!(snapshot.phase, InitializationPhase::Starting);
        assert_eq!(snapshot.run_id, 1);
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(
            snapshot.events[0].summary,
            "Starting Codex Pulse reconciliation"
        );
    }

    #[test]
    fn assigns_a_new_run_id_without_reusing_event_sequences() {
        let mut feed = InitializationFeed::default();
        let first = feed.begin(1);
        let second = feed.begin(2);

        assert_eq!(first.run_id, 1);
        assert_eq!(second.run_id, 2);
        assert!(second.sequence > first.sequence);
    }
}
