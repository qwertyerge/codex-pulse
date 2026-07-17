use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use anyhow::Result;
use chrono::{Datelike, Duration as ChronoDuration, Local};

use crate::codex::discovery::{discover_recent_jsonl_files, ScanCache};
use crate::codex::jsonl::{parse_line, ParsedRecord};
use crate::codex::session_index::lookup_thread_names;
use crate::codex::title::{lookup_titles, recent_thread_paths};
use crate::model::{ResolvedTitle, SessionSnapshot, WeeklyQuota};
use crate::registry::SessionRegistry;

const RECENT_SESSION_WINDOW: Duration = Duration::from_secs(6 * 60 * 60);
const SQLITE_CANDIDATE_LIMIT: usize = 32;
const QUOTA_DISCOVERY_INTERVAL: Duration = Duration::from_secs(60);
const QUOTA_SOURCE_FILE_LIMIT: usize = 16;
const QUOTA_INITIAL_TAIL_BYTES: u64 = 256 * 1024;

pub struct ScanResult {
    pub sessions: Vec<SessionSnapshot>,
    pub weekly_quota: Option<WeeklyQuota>,
}

/// Maintains a separate, bounded source for quota observations.
///
/// Active-session discovery intentionally remains tied to the recent SQLite
/// candidates. Quota observations can outlive an active turn, so this cache
/// scans today's and yesterday's session directories at a low cadence and
/// incrementally parses only the selected files between discoveries.
#[derive(Default)]
pub struct QuotaSourceCache {
    entries: HashMap<PathBuf, CachedQuotaSource>,
    candidate_paths: Vec<PathBuf>,
    last_discovery_at: Option<Instant>,
}

#[derive(Clone, PartialEq, Eq)]
struct QuotaFileIdentity {
    device: u64,
    inode: u64,
}

impl QuotaFileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

struct CachedQuotaSource {
    identity: QuotaFileIdentity,
    modified: SystemTime,
    length: u64,
    processed_line_count: u64,
    byte_offset: u64,
    latest_weekly_quota: Option<WeeklyQuota>,
}

impl QuotaSourceCache {
    pub fn latest_weekly_quota(&mut self, codex_home: &Path, now_ms: i64) -> Option<WeeklyQuota> {
        if self
            .last_discovery_at
            .is_none_or(|last| last.elapsed() >= QUOTA_DISCOVERY_INTERVAL)
        {
            self.candidate_paths = recent_quota_candidate_paths(codex_home);
            self.last_discovery_at = Some(Instant::now());
        }

        let candidates = self.candidate_paths.clone();
        let candidate_set = candidates
            .iter()
            .map(PathBuf::as_path)
            .collect::<HashSet<_>>();
        self.entries
            .retain(|path, _| candidate_set.contains(path.as_path()));
        for path in candidates {
            if self.refresh_one(&path).is_err() {
                self.entries.remove(&path);
            }
        }

        self.entries
            .values()
            .filter_map(|entry| entry.latest_weekly_quota.as_ref())
            .filter(|quota| quota.resets_at_ms > now_ms)
            .max_by_key(|quota| quota.observed_at_ms)
            .cloned()
    }

    fn refresh_one(&mut self, path: &Path) -> Result<()> {
        let metadata = fs::metadata(path)?;
        let identity = QuotaFileIdentity::from_metadata(&metadata);
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let length = metadata.len();

        if !self.entries.contains_key(path) {
            self.entries.insert(
                path.to_owned(),
                CachedQuotaSource::read_tail(path, identity, modified, length)?,
            );
            return Ok(());
        }

        let entry = self
            .entries
            .get_mut(path)
            .expect("quota cache entry exists");
        let can_append =
            entry.identity == identity && length > entry.length && modified >= entry.modified;
        let is_unchanged =
            entry.identity == identity && length == entry.length && modified == entry.modified;
        if can_append {
            let (processed_line_count, byte_offset) = read_quota_lines(
                path,
                entry.byte_offset,
                false,
                entry.processed_line_count,
                &mut entry.latest_weekly_quota,
            )?;
            entry.processed_line_count = processed_line_count;
            entry.byte_offset = byte_offset;
            entry.length = length;
            entry.modified = modified;
        } else if !is_unchanged {
            *entry = CachedQuotaSource::read_tail(path, identity, modified, length)?;
        }
        Ok(())
    }
}

impl CachedQuotaSource {
    fn read_tail(
        path: &Path,
        identity: QuotaFileIdentity,
        modified: SystemTime,
        length: u64,
    ) -> Result<Self> {
        let tail_offset = length.saturating_sub(QUOTA_INITIAL_TAIL_BYTES);
        let mut latest_weekly_quota = None;
        let (processed_line_count, byte_offset) = read_quota_lines(
            path,
            tail_offset,
            tail_offset > 0,
            0,
            &mut latest_weekly_quota,
        )?;
        Ok(Self {
            identity,
            modified,
            length,
            processed_line_count,
            byte_offset,
            latest_weekly_quota,
        })
    }
}

fn read_quota_lines(
    path: &Path,
    byte_offset: u64,
    skip_first_line: bool,
    mut processed_line_count: u64,
    latest_weekly_quota: &mut Option<WeeklyQuota>,
) -> Result<(u64, u64)> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(byte_offset))?;
    let mut reader = BufReader::new(file);
    let mut next_offset = byte_offset;
    let mut line = String::new();

    if skip_first_line {
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 || !line.ends_with('\n') {
            return Ok((processed_line_count, byte_offset));
        }
        next_offset += bytes_read as u64;
        line.clear();
    }

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 || !line.ends_with('\n') {
            break;
        }
        if line.contains("\"type\":\"token_count\"") {
            if let Ok(Some(ParsedRecord::WeeklyQuota(candidate))) = parse_line(&line) {
                if latest_weekly_quota
                    .as_ref()
                    .is_none_or(|current| candidate.observed_at_ms >= current.observed_at_ms)
                {
                    *latest_weekly_quota = Some(candidate);
                }
            }
        }
        processed_line_count += 1;
        next_offset += bytes_read as u64;
    }

    Ok((processed_line_count, next_offset))
}

fn recent_quota_candidate_paths(codex_home: &Path) -> Vec<PathBuf> {
    let sessions = codex_home.join("sessions");
    let today = Local::now().date_naive();
    let mut paths = Vec::new();

    for days_ago in 0..=1 {
        let day = today - ChronoDuration::days(days_ago);
        let day_dir = sessions
            .join(format!("{:04}", day.year()))
            .join(format!("{:02}", day.month()))
            .join(format!("{:02}", day.day()));
        let Ok(entries) = fs::read_dir(day_dir) else {
            continue;
        };
        paths.extend(
            entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "jsonl")
                }),
        );
    }

    paths.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });
    paths.reverse();
    paths.truncate(QUOTA_SOURCE_FILE_LIMIT);
    paths
}

pub fn scan_active_sessions(codex_home: &Path, now_ms: i64) -> Result<Vec<SessionSnapshot>> {
    let mut cache = ScanCache::default();
    Ok(scan_active_sessions_with_cache(codex_home, now_ms, &mut cache)?.sessions)
}

pub fn scan_active_sessions_with_cache(
    codex_home: &Path,
    now_ms: i64,
    cache: &mut ScanCache,
) -> Result<ScanResult> {
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
    let transcripts = cache.refresh(&transcript_paths);
    let weekly_quota = transcripts
        .iter()
        .filter_map(|transcript| transcript.weekly_quota.as_ref())
        .filter(|quota| quota.resets_at_ms > now_ms)
        .max_by_key(|quota| quota.observed_at_ms)
        .cloned();
    let thread_ids = transcripts
        .iter()
        .map(|transcript| transcript.meta.thread_id.clone())
        .collect::<HashSet<_>>();
    let titles = lookup_titles(&database, &thread_ids).unwrap_or_default();
    let sidebar_titles = lookup_thread_names(codex_home, &thread_ids);

    for transcript in transcripts {
        let last_observed_at_ms = transcript.last_observed_at_ms;
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
        if let Some(last_observed_at_ms) = last_observed_at_ms {
            registry.apply_runtime_activity(&thread_id, last_observed_at_ms);
        }
    }
    registry.mark_stale(now_ms);

    Ok(ScanResult {
        sessions: registry.snapshots(now_ms),
        weekly_quota,
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use super::{scan_active_sessions_with_cache, QuotaSourceCache};
    use crate::codex::discovery::ScanCache;

    #[test]
    fn retains_a_descendant_with_recent_runtime_activity() {
        let temp = tempfile::tempdir().unwrap();
        let codex_home = temp.path();
        let sessions = codex_home.join("sessions/2026/07/17");
        fs::create_dir_all(&sessions).unwrap();
        fs::write(
            sessions.join("root.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000001\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/root\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"root-turn\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"root-turn\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:04:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"primary\":{\"used_percent\":81.0,\"window_minutes\":10080,\"resets_at\":1784870653}}}}\n"
            ),
        )
        .unwrap();
        fs::write(
            sessions.join("child.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000002\",\"timestamp\":\"2026-07-17T07:01:00Z\",\"cwd\":\"/child\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"00000000-0000-4000-8000-000000000001\"}}}}}\n",
                "{\"timestamp\":\"2026-07-17T07:03:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"child-turn\"}}\n",
                "{\"timestamp\":\"2026-07-17T08:03:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"Still working\"}}\n"
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

        let mut cache = ScanCache::default();
        let now_ms = chrono::DateTime::parse_from_rfc3339("2026-07-17T08:04:00Z")
            .unwrap()
            .timestamp_millis();
        let result = scan_active_sessions_with_cache(codex_home, now_ms, &mut cache).unwrap();
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(
            result.sessions[0].thread_id,
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(result.sessions[0].title, "Root task");
        assert_eq!(result.sessions[0].cwd, "/root");
        assert_eq!(
            result.sessions[0].current_run_started_at_ms,
            1_784_271_780_000
        );
        assert_eq!(result.weekly_quota.as_ref().unwrap().used_percent, 81);
    }

    #[test]
    fn excludes_a_stale_unfinished_descendant_when_its_root_has_completed() {
        let temp = tempfile::tempdir().unwrap();
        let codex_home = temp.path();
        let sessions = codex_home.join("sessions/2026/07/17");
        fs::create_dir_all(&sessions).unwrap();
        fs::write(
            sessions.join("root.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000011\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/root\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"root-turn\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"root-turn\"}}\n"
            ),
        )
        .unwrap();
        fs::write(
            sessions.join("child.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"00000000-0000-4000-8000-000000000012\",\"timestamp\":\"2026-07-17T07:01:00Z\",\"cwd\":\"/child\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"00000000-0000-4000-8000-000000000011\"}}}}}\n",
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
                ["00000000-0000-4000-8000-000000000011", "Completed root"],
            )
            .unwrap();
        drop(connection);

        let now_ms = chrono::DateTime::parse_from_rfc3339("2026-07-17T08:04:00Z")
            .unwrap()
            .timestamp_millis();
        let mut cache = ScanCache::default();
        let result = scan_active_sessions_with_cache(codex_home, now_ms, &mut cache).unwrap();

        assert!(result.sessions.is_empty());
    }

    #[test]
    fn quota_source_reads_recent_sessions_and_rejects_expired_observations() {
        let temp = tempfile::tempdir().unwrap();
        let day = chrono::Local::now().format("%Y/%m/%d").to_string();
        let sessions = temp.path().join("sessions").join(day);
        fs::create_dir_all(&sessions).unwrap();
        fs::write(
            sessions.join("quota.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"quota\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"primary\":{\"used_percent\":81.0,\"window_minutes\":10080,\"resets_at\":2}}}}\n"
            ),
        )
        .unwrap();
        let mut cache = QuotaSourceCache::default();

        assert_eq!(
            cache
                .latest_weekly_quota(temp.path(), 1_000)
                .unwrap()
                .used_percent,
            81
        );
        assert!(cache.latest_weekly_quota(temp.path(), 2_000).is_none());
    }

    #[test]
    fn quota_source_reads_a_tail_observation_without_session_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let day = chrono::Local::now().format("%Y/%m/%d").to_string();
        let sessions = temp.path().join("sessions").join(day);
        fs::create_dir_all(&sessions).unwrap();
        let transcript = sessions.join("large.jsonl");
        let mut file = fs::File::create(&transcript).unwrap();
        file.write_all(&vec![b'x'; 300 * 1024]).unwrap();
        writeln!(file).unwrap();
        writeln!(
            file,
            "{{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"rate_limits\":{{\"primary\":{{\"used_percent\":81.0,\"window_minutes\":10080,\"resets_at\":2}}}}}}}}"
        )
        .unwrap();
        let mut cache = QuotaSourceCache::default();

        assert_eq!(
            cache
                .latest_weekly_quota(temp.path(), 1_000)
                .unwrap()
                .used_percent,
            81
        );

        writeln!(
            file,
            "{{\"timestamp\":\"2026-07-17T07:03:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"rate_limits\":{{\"primary\":{{\"used_percent\":82.0,\"window_minutes\":10080,\"resets_at\":2}}}}}}}}"
        )
        .unwrap();

        assert_eq!(
            cache
                .latest_weekly_quota(temp.path(), 1_000)
                .unwrap()
                .used_percent,
            82
        );
    }

    #[test]
    fn quota_source_limits_daily_candidates_to_sixteen_files() {
        let temp = tempfile::tempdir().unwrap();
        let day = chrono::Local::now().format("%Y/%m/%d").to_string();
        let sessions = temp.path().join("sessions").join(day);
        fs::create_dir_all(&sessions).unwrap();
        for index in 0..17 {
            fs::write(sessions.join(format!("quota-{index}.jsonl")), "{}\n").unwrap();
        }

        assert_eq!(super::recent_quota_candidate_paths(temp.path()).len(), 16);
    }
}
