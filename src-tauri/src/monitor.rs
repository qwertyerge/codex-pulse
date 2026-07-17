use std::{
    collections::HashSet,
    path::Path,
    time::{Duration, SystemTime},
};

use anyhow::Result;

use crate::codex::discovery::{discover_recent_jsonl_files, read_transcript, ParsedTranscript};
use crate::codex::session_index::lookup_thread_names;
use crate::codex::title::{lookup_titles, recent_thread_paths};
use crate::model::{ResolvedTitle, SessionSnapshot};
use crate::registry::SessionRegistry;

const RECENT_SESSION_WINDOW: Duration = Duration::from_secs(6 * 60 * 60);
const SQLITE_CANDIDATE_LIMIT: usize = 32;

pub fn scan_active_sessions(codex_home: &Path, now_ms: i64) -> Result<Vec<SessionSnapshot>> {
    let mut registry = SessionRegistry::default();
    let database = codex_home.join("state_5.sqlite");
    let since = SystemTime::now()
        .checked_sub(RECENT_SESSION_WINDOW)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let candidate_paths = recent_thread_paths(&database, SQLITE_CANDIDATE_LIMIT)
        .unwrap_or_default()
        .into_iter()
        .map(|candidate| candidate.rollout_path.into())
        .filter(|path: &std::path::PathBuf| path.is_file())
        .collect::<Vec<_>>();
    let transcript_paths = if candidate_paths.is_empty() {
        discover_recent_jsonl_files(&codex_home.join("sessions"), since)
    } else {
        candidate_paths
    };
    let transcripts = transcript_paths
        .into_iter()
        .filter_map(|transcript_path| read_transcript(&transcript_path).ok())
        .collect::<Vec<ParsedTranscript>>();
    let thread_ids = transcripts
        .iter()
        .map(|transcript| transcript.meta.thread_id.clone())
        .collect::<HashSet<_>>();
    let titles = lookup_titles(&database, &thread_ids).unwrap_or_default();
    let sidebar_titles = lookup_thread_names(codex_home, &thread_ids);

    for transcript in transcripts {
        let mut meta = transcript.meta;
        if let Some(title) = sidebar_titles
            .get(&meta.thread_id)
            .or_else(|| titles.get(&meta.thread_id))
        {
            meta.title = ResolvedTitle::Stored(title.clone());
        }
        let thread_id = meta.thread_id.clone();
        registry.apply_meta(meta);
        for event in transcript.events {
            registry.apply_event(event);
        }
        for event in transcript.recent_events {
            registry.apply_recent_event(&thread_id, event);
        }
        for message in transcript.user_messages {
            registry.apply_user_message(&thread_id, message);
        }
    }

    Ok(registry.snapshots(now_ms))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::scan_active_sessions;

    #[test]
    fn returns_one_titled_root_when_only_its_descendant_is_active() {
        let temp = tempfile::tempdir().unwrap();
        let codex_home = temp.path();
        let sessions = codex_home.join("sessions/2026/07/17");
        fs::create_dir_all(&sessions).unwrap();
        fs::write(
            sessions.join("root.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000001\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/root\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"root-turn\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"root-turn\"}}\n"
            ),
        )
        .unwrap();
        fs::write(
            sessions.join("child.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000002\",\"timestamp\":\"2026-07-17T07:01:00Z\",\"cwd\":\"/child\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"00000000-0000-4000-8000-000000000001\"}}}}}\n",
                "{\"timestamp\":\"2026-07-17T07:03:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"child-turn\"}}\n"
            ),
        )
        .unwrap();
        let database = codex_home.join("state_5.sqlite");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute(
                "CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT NOT NULL)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO threads (id, title) VALUES (?1, ?2)",
                ["00000000-0000-4000-8000-000000000001", "Root task"],
            )
            .unwrap();
        drop(connection);

        let sessions = scan_active_sessions(codex_home, 1_784_272_200_000).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].thread_id,
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(sessions[0].title, "Root task");
        assert_eq!(sessions[0].cwd, "/root");
        assert_eq!(sessions[0].current_run_started_at_ms, 1_784_271_780_000);
    }
}
