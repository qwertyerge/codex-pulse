use std::path::Path;
use std::{
    fs,
    io::{BufRead, BufReader},
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use walkdir::WalkDir;

use crate::codex::jsonl::{parse_line, ParsedRecord, TranscriptEvent};
use crate::model::{LifecycleEvent, RecentEvent, ThreadMeta, UserMessage};

#[derive(Debug, Clone)]
pub struct ParsedTranscript {
    pub meta: ThreadMeta,
    pub events: Vec<LifecycleEvent>,
    pub recent_events: Vec<RecentEvent>,
    pub user_messages: Vec<UserMessage>,
}

pub fn read_transcript(path: &Path) -> Result<ParsedTranscript> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut meta = None;
    let mut unbound_events: Vec<TranscriptEvent> = Vec::new();
    let mut recent_events = Vec::new();
    let mut user_messages = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if !line.contains("\"type\":\"session_meta\"")
            && !line.contains("\"type\":\"event_msg\"")
            && !line.contains("\"type\":\"response_item\"")
        {
            continue;
        }
        match parse_line(&line) {
            // A subagent transcript can embed its parent's session metadata
            // later in the file. The first metadata record identifies the
            // transcript itself; subsequent records are context, not a new
            // lifecycle owner.
            Ok(Some(ParsedRecord::Meta(record))) if meta.is_none() => meta = Some(record),
            Ok(Some(ParsedRecord::Meta(_))) => {}
            Ok(Some(ParsedRecord::Lifecycle(record))) => unbound_events.push(record),
            Ok(Some(ParsedRecord::LifecycleAndRecent(lifecycle, recent))) => {
                unbound_events.push(lifecycle);
                recent_events.push(recent);
            }
            Ok(Some(ParsedRecord::Recent(record))) => recent_events.push(record),
            Ok(Some(ParsedRecord::UserMessage(record))) => user_messages.push(record),
            Ok(Some(ParsedRecord::Ignored)) | Ok(None) | Err(_) => {}
        }
    }

    let meta =
        meta.ok_or_else(|| anyhow!("transcript is missing session metadata: {}", path.display()))?;
    let events = unbound_events
        .into_iter()
        .map(|event| event.bind(meta.thread_id.clone()))
        .collect();

    Ok(ParsedTranscript {
        meta,
        events,
        recent_events,
        user_messages,
    })
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
    use std::fs;

    use super::read_transcript;
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
}
