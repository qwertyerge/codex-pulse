use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use walkdir::WalkDir;

use crate::codex::jsonl::{parse_line, ParsedRecord, TranscriptEvent};
use crate::model::{
    LifecycleEvent, LifecycleEventKind, RecentEvent, ThreadMeta, UserMessage, WeeklyQuota,
};

const MAX_CACHED_LIFECYCLE_EVENTS: usize = 64;

#[derive(Debug, Clone)]
pub struct ParsedTranscript {
    pub meta: ThreadMeta,
    pub events: Vec<LifecycleEvent>,
    pub recent_events: Vec<RecentEvent>,
    pub user_messages: Vec<UserMessage>,
    pub weekly_quota: Option<WeeklyQuota>,
    pub last_observed_at_ms: Option<i64>,
}

#[derive(Default)]
struct TranscriptAccumulator {
    meta: Option<ThreadMeta>,
    unbound_events: Vec<TranscriptEvent>,
    recent_event: Option<RecentEvent>,
    user_message: Option<UserMessage>,
    weekly_quota: Option<WeeklyQuota>,
    last_observed_at_ms: Option<i64>,
}

impl TranscriptAccumulator {
    fn apply_complete_line(&mut self, line: &str) {
        if !line.contains("\"type\":\"session_meta\"")
            && !line.contains("\"type\":\"event_msg\"")
            && !line.contains("\"type\":\"response_item\"")
        {
            return;
        }

        match parse_line(line) {
            // A subagent transcript can embed its parent's session metadata
            // later in the file. The first metadata record identifies the
            // transcript itself; subsequent records are context, not a new
            // lifecycle owner.
            Ok(Some(ParsedRecord::Meta(record))) if self.meta.is_none() => {
                self.record_observation(record.session_created_at_ms);
                self.meta = Some(record);
            }
            Ok(Some(ParsedRecord::Meta(_))) => {}
            Ok(Some(ParsedRecord::Lifecycle(record))) => {
                self.record_observation(record.occurred_at_ms);
                self.push_lifecycle(record);
            }
            Ok(Some(ParsedRecord::LifecycleAndRecent(lifecycle, recent))) => {
                self.record_observation(lifecycle.occurred_at_ms);
                self.push_lifecycle(lifecycle);
                self.record_recent(recent);
            }
            Ok(Some(ParsedRecord::Recent(record))) => {
                self.record_observation(record.occurred_at_ms);
                self.record_recent(record);
            }
            Ok(Some(ParsedRecord::UserMessage(record))) => {
                self.record_observation(record.occurred_at_ms);
                self.record_user_message(record);
            }
            Ok(Some(ParsedRecord::WeeklyQuota(candidate))) => {
                self.record_observation(candidate.observed_at_ms);
                if self
                    .weekly_quota
                    .as_ref()
                    .is_none_or(|current| candidate.observed_at_ms >= current.observed_at_ms)
                {
                    self.weekly_quota = Some(candidate);
                }
            }
            Ok(Some(ParsedRecord::Ignored)) | Ok(None) | Err(_) => {}
        }
    }

    fn push_lifecycle(&mut self, event: TranscriptEvent) {
        self.unbound_events.push(event);
        if self.unbound_events.len() > MAX_CACHED_LIFECYCLE_EVENTS {
            self.unbound_events = compact_lifecycle_events(&self.unbound_events);
        }
    }

    fn record_recent(&mut self, candidate: RecentEvent) {
        if self.recent_event.as_ref().is_none_or(|current| {
            candidate.priority > current.priority
                || (candidate.priority == current.priority
                    && candidate.occurred_at_ms >= current.occurred_at_ms)
        }) {
            self.recent_event = Some(candidate);
        }
    }

    fn record_user_message(&mut self, candidate: UserMessage) {
        if self
            .user_message
            .as_ref()
            .is_none_or(|current| candidate.occurred_at_ms >= current.occurred_at_ms)
        {
            self.user_message = Some(candidate);
        }
    }

    fn record_observation(&mut self, occurred_at_ms: i64) {
        if self
            .last_observed_at_ms
            .is_none_or(|current| occurred_at_ms >= current)
        {
            self.last_observed_at_ms = Some(occurred_at_ms);
        }
    }

    fn snapshot(&self) -> Option<ParsedTranscript> {
        let meta = self.meta.clone()?;
        let events = self
            .unbound_events
            .iter()
            .cloned()
            .map(|event| event.bind(meta.thread_id.clone()))
            .collect();
        Some(ParsedTranscript {
            meta,
            events,
            recent_events: self.recent_event.iter().cloned().collect(),
            user_messages: self.user_message.iter().cloned().collect(),
            weekly_quota: self.weekly_quota.clone(),
            last_observed_at_ms: self.last_observed_at_ms,
        })
    }
}

fn compact_lifecycle_events(events: &[TranscriptEvent]) -> Vec<TranscriptEvent> {
    let mut active = false;
    let mut current_turn_id = None;
    let mut current_turn_started_at_ms = None;
    let mut last_activity_at_ms = None;
    let mut last_event_at_ms = i64::MIN;

    for event in events {
        if event.occurred_at_ms < last_event_at_ms {
            continue;
        }
        match event.kind {
            LifecycleEventKind::SessionStart => last_activity_at_ms = Some(event.occurred_at_ms),
            LifecycleEventKind::TurnStart => {
                let same_active_turn =
                    active && current_turn_id.as_deref() == event.turn_id.as_deref();
                if !same_active_turn {
                    current_turn_id = event.turn_id.clone();
                    current_turn_started_at_ms = Some(event.occurred_at_ms);
                }
                active = true;
                last_activity_at_ms = Some(event.occurred_at_ms);
            }
            LifecycleEventKind::Activity => {
                if current_turn_id.is_none() {
                    current_turn_id = event.turn_id.clone();
                    current_turn_started_at_ms = Some(event.occurred_at_ms);
                }
                active = true;
                last_activity_at_ms = Some(event.occurred_at_ms);
            }
            LifecycleEventKind::TurnEnd
            | LifecycleEventKind::SubagentEnd
            | LifecycleEventKind::Abort => {
                if event.turn_id.is_none() || current_turn_id.as_deref() == event.turn_id.as_deref()
                {
                    active = false;
                    current_turn_id = None;
                    current_turn_started_at_ms = None;
                    last_activity_at_ms = Some(event.occurred_at_ms);
                }
            }
        }
        last_event_at_ms = event.occurred_at_ms;
    }

    if !active {
        return Vec::new();
    }
    let started_at_ms = current_turn_started_at_ms.expect("active turn has a start timestamp");
    let mut compacted = vec![TranscriptEvent {
        kind: LifecycleEventKind::TurnStart,
        turn_id: current_turn_id,
        occurred_at_ms: started_at_ms,
    }];
    if let Some(last_activity_at_ms) = last_activity_at_ms.filter(|at| *at > started_at_ms) {
        compacted.push(TranscriptEvent {
            kind: LifecycleEventKind::Activity,
            turn_id: compacted[0].turn_id.clone(),
            occurred_at_ms: last_activity_at_ms,
        });
    }
    compacted
}

#[derive(Clone, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

impl FileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

struct CachedTranscript {
    identity: FileIdentity,
    modified: SystemTime,
    length: u64,
    processed_line_count: u64,
    byte_offset: u64,
    parsed: TranscriptAccumulator,
}

#[derive(Default)]
pub struct ScanCache {
    entries: HashMap<PathBuf, CachedTranscript>,
}

impl ScanCache {
    pub fn refresh(&mut self, paths: &[PathBuf]) -> Vec<ParsedTranscript> {
        let candidates = paths.iter().map(PathBuf::as_path).collect::<HashSet<_>>();
        self.entries
            .retain(|path, _| candidates.contains(path.as_path()));

        let mut transcripts = Vec::new();
        for path in paths {
            match self.refresh_one(path) {
                Ok(Some(transcript)) => transcripts.push(transcript),
                Ok(None) => {}
                Err(_) => {
                    self.entries.remove(path);
                }
            }
        }
        transcripts
    }

    fn refresh_one(&mut self, path: &Path) -> Result<Option<ParsedTranscript>> {
        let metadata = fs::metadata(path)?;
        let identity = FileIdentity::from_metadata(&metadata);
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        let length = metadata.len();

        if !self.entries.contains_key(path) {
            self.entries.insert(
                path.to_owned(),
                CachedTranscript::read_from_start(path, identity, modified, length)?,
            );
        } else {
            let entry = self.entries.get_mut(path).expect("cache entry exists");
            let can_append =
                entry.identity == identity && length > entry.length && modified >= entry.modified;
            let is_unchanged =
                entry.identity == identity && length == entry.length && modified == entry.modified;

            if can_append {
                let (processed_line_count, byte_offset) = read_complete_lines(
                    path,
                    &mut entry.parsed,
                    entry.processed_line_count,
                    entry.byte_offset,
                )?;
                entry.processed_line_count = processed_line_count;
                entry.byte_offset = byte_offset;
                entry.length = length;
                entry.modified = modified;
            } else if !is_unchanged {
                *entry = CachedTranscript::read_from_start(path, identity, modified, length)?;
            }
        }

        Ok(self
            .entries
            .get(path)
            .and_then(|entry| entry.parsed.snapshot()))
    }

    #[cfg(test)]
    fn cursor_for(&self, path: &Path) -> Option<(u64, u64)> {
        self.entries
            .get(path)
            .map(|entry| (entry.processed_line_count, entry.byte_offset))
    }
}

impl CachedTranscript {
    fn read_from_start(
        path: &Path,
        identity: FileIdentity,
        modified: SystemTime,
        length: u64,
    ) -> Result<Self> {
        let mut parsed = TranscriptAccumulator::default();
        let (processed_line_count, byte_offset) = read_complete_lines(path, &mut parsed, 0, 0)?;
        Ok(Self {
            identity,
            modified,
            length,
            processed_line_count,
            byte_offset,
            parsed,
        })
    }
}

fn read_complete_lines(
    path: &Path,
    parsed: &mut TranscriptAccumulator,
    mut processed_line_count: u64,
    byte_offset: u64,
) -> Result<(u64, u64)> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(byte_offset))?;
    let mut reader = BufReader::new(file);
    let mut next_offset = byte_offset;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }
        if !line.ends_with('\n') {
            break;
        }
        parsed.apply_complete_line(&line);
        processed_line_count += 1;
        next_offset += bytes_read as u64;
    }

    Ok((processed_line_count, next_offset))
}

pub fn read_transcript(path: &Path) -> Result<ParsedTranscript> {
    let mut cache = ScanCache::default();
    cache
        .refresh(&[path.to_owned()])
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("transcript is missing session metadata: {}", path.display()))
}

pub fn discover_jsonl_files(sessions_dir: &Path) -> Vec<std::path::PathBuf> {
    WalkDir::new(sessions_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .map(|entry| entry.into_path())
        .collect()
}

pub fn discover_recent_jsonl_files(
    sessions_dir: &Path,
    modified_since: SystemTime,
) -> Vec<std::path::PathBuf> {
    discover_jsonl_files(sessions_dir)
        .into_iter()
        .filter(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified >= modified_since)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use super::{read_transcript, ScanCache, MAX_CACHED_LIFECYCLE_EVENTS};
    use crate::model::LifecycleEventKind;

    #[test]
    fn binds_task_events_to_the_transcript_thread() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("rollout.jsonl");
        fs::write(
            &transcript,
            concat!(
                "{\"timestamp\":\"2026-07-16T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000001\",\"timestamp\":\"2026-07-16T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-16T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn-1\"}}\n"
            ),
        )
        .unwrap();

        let parsed = read_transcript(&transcript).unwrap();
        assert_eq!(
            parsed.meta.thread_id,
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].thread_id, parsed.meta.thread_id);
        assert_eq!(parsed.events[0].kind, LifecycleEventKind::TurnStart);
    }

    #[test]
    fn ignores_a_partial_tail_line_until_the_next_scan() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("partial.jsonl");
        fs::write(
            &transcript,
            "{\"timestamp\":\"2026-07-16T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000001\",\"timestamp\":\"2026-07-16T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n{\"timestamp\":\"2026-07",
        )
        .unwrap();

        let parsed = read_transcript(&transcript).unwrap();
        assert!(parsed.events.is_empty());
    }

    #[test]
    fn keeps_the_first_metadata_when_a_child_transcript_embeds_its_parent() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("child.jsonl");
        fs::write(
            &transcript,
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"child\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/child\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"parent\"}}}}}\n",
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"child-turn\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"parent\",\"timestamp\":\"2026-07-17T06:00:00Z\",\"cwd\":\"/parent\",\"source\":\"cli\"}}\n"
            ),
        )
        .unwrap();

        let parsed = read_transcript(&transcript).unwrap();

        assert_eq!(parsed.meta.thread_id, "child");
        assert_eq!(parsed.meta.parent_thread_id.as_deref(), Some("parent"));
        assert_eq!(parsed.events[0].thread_id, "child");
    }

    #[test]
    fn increments_from_the_last_complete_line() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("append.jsonl");
        fs::write(
            &transcript,
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"root\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn-1\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"limit_id\":\"codex\",\"primary\":{\"used_percent\":81.0,\"window_minutes\":10080,\"resets_at\":1784870653}}}}\n",
                "{\"timestamp\":\"2026-07-17T07:03:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\""
            ),
        )
        .unwrap();
        let mut cache = ScanCache::default();

        let first = cache.refresh(&[transcript.clone()]);
        assert_eq!(cache.cursor_for(&transcript).unwrap().0, 3);
        assert_eq!(first[0].weekly_quota.as_ref().unwrap().used_percent, 81);

        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&transcript)
            .unwrap();
        writeln!(file, ",\"turn_id\":\"turn-1\"}}}}").unwrap();
        let second = cache.refresh(&[transcript.clone()]);

        assert_eq!(cache.cursor_for(&transcript).unwrap().0, 4);
        assert_eq!(second[0].events.len(), 2);
    }

    #[test]
    fn rebuilds_after_a_transcript_is_truncated() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("truncated.jsonl");
        let metadata = "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"root\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n";
        let started = "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn-1\"}}\n";
        let quota = "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"limit_id\":\"codex\",\"primary\":{\"used_percent\":81.0,\"window_minutes\":10080,\"resets_at\":1784870653}}}}\n";
        fs::write(&transcript, format!("{metadata}{started}{quota}")).unwrap();
        let mut cache = ScanCache::default();
        assert!(cache.refresh(&[transcript.clone()])[0]
            .weekly_quota
            .is_some());

        fs::write(&transcript, format!("{metadata}{started}")).unwrap();
        let rebuilt = cache.refresh(&[transcript.clone()]);

        assert_eq!(cache.cursor_for(&transcript).unwrap().0, 2);
        assert!(rebuilt[0].weekly_quota.is_none());
        assert_eq!(rebuilt[0].events.len(), 1);
    }

    #[test]
    fn evicts_a_candidate_that_disappears_between_refreshes() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("deleted.jsonl");
        fs::write(
            &transcript,
            "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"root\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n",
        )
        .unwrap();
        let mut cache = ScanCache::default();
        assert_eq!(cache.refresh(&[transcript.clone()]).len(), 1);

        fs::remove_file(&transcript).unwrap();

        assert!(cache.refresh(&[transcript.clone()]).is_empty());
        assert!(cache.cursor_for(&transcript).is_none());
    }

    #[test]
    fn bounds_cached_transcript_history_without_losing_the_current_turn() {
        let temp = tempfile::tempdir().unwrap();
        let transcript = temp.path().join("long-running.jsonl");
        let mut file = fs::File::create(&transcript).unwrap();
        writeln!(file, "{{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"root\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}}}").unwrap();
        for index in 0..(MAX_CACHED_LIFECYCLE_EVENTS * 3) {
            writeln!(file, "{{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\",\"turn_id\":\"turn-{index}\"}}}}").unwrap();
            writeln!(file, "{{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\",\"message\":\"status {index}\"}}}}").unwrap();
            writeln!(file, "{{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"prompt {index}\"}}}}").unwrap();
        }

        let parsed = read_transcript(&transcript).unwrap();

        assert!(parsed.events.len() <= MAX_CACHED_LIFECYCLE_EVENTS);
        assert_eq!(
            parsed.events.last().unwrap().turn_id.as_deref(),
            Some("turn-191")
        );
        assert_eq!(parsed.recent_events.len(), 1);
        assert_eq!(parsed.user_messages.len(), 1);
    }
}
