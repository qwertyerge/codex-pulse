use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, RwLock,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::Utc;
use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::codex::discovery::ScanCache;
use crate::config::{AppConfig, ConfigStore};
use crate::initialization::{InitializationFeed, INITIALIZATION_PROGRESS_EVENT};
use crate::model::{
    AppSnapshot, InitializationPhase, InitializationSnapshot, LocaleMode, MonitoringView,
    RecentEvent, SessionSnapshot, ThemeMode, WeeklyQuota,
};
use crate::monitor::{scan_active_sessions_with_cache, QuotaSourceCache};

pub struct AppState {
    pub codex_home: PathBuf,
    pub store: ConfigStore,
    pub config: Mutex<AppConfig>,
    cached_snapshot: RwLock<CachedSnapshot>,
    scan_cache: Mutex<ScanCache>,
    quota_source_cache: Mutex<QuotaSourceCache>,
    initialization: Mutex<InitializationFeed>,
    recent_event_display: Mutex<HashMap<String, DisplayedRecentEvent>>,
    refresh_in_flight: AtomicBool,
}

#[derive(Default)]
struct CachedSnapshot {
    sessions: Vec<SessionSnapshot>,
    weekly_quota: Option<WeeklyQuota>,
}

const RECENT_EVENT_COALESCE_MS: i64 = 5_000;
pub const FALLBACK_RECONCILIATION_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
struct DisplayedRecentEvent {
    event: RecentEvent,
    displayed_at_ms: i64,
}

impl AppState {
    pub fn new(codex_home: PathBuf, store: ConfigStore) -> Result<Self> {
        let config = store.load()?;
        Ok(Self {
            codex_home,
            store,
            config: Mutex::new(config),
            cached_snapshot: RwLock::new(CachedSnapshot::default()),
            scan_cache: Mutex::new(ScanCache::default()),
            quota_source_cache: Mutex::new(QuotaSourceCache::default()),
            initialization: Mutex::new(InitializationFeed::default()),
            recent_event_display: Mutex::new(HashMap::new()),
            refresh_in_flight: AtomicBool::new(false),
        })
    }

    pub fn from_environment() -> Self {
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
            .unwrap_or_else(|| PathBuf::from(".codex"));
        let store = ConfigStore::for_user();
        Self::new(codex_home, store.clone()).unwrap_or_else(|_| Self {
            codex_home: PathBuf::from(".codex"),
            store,
            config: Mutex::new(AppConfig::default()),
            cached_snapshot: RwLock::new(CachedSnapshot::default()),
            scan_cache: Mutex::new(ScanCache::default()),
            quota_source_cache: Mutex::new(QuotaSourceCache::default()),
            initialization: Mutex::new(InitializationFeed::default()),
            recent_event_display: Mutex::new(HashMap::new()),
            refresh_in_flight: AtomicBool::new(false),
        })
    }

    fn cached_snapshot(&self) -> (Vec<SessionSnapshot>, Option<WeeklyQuota>) {
        self.cached_snapshot
            .read()
            .map(|snapshot| (snapshot.sessions.clone(), snapshot.weekly_quota.clone()))
            .unwrap_or_default()
    }

    fn cached_initialization(&self) -> InitializationSnapshot {
        self.initialization
            .lock()
            .map(|feed| feed.snapshot())
            .unwrap_or_default()
    }
}

fn publish_initialization_event(
    app: &tauri::AppHandle,
    state: &AppState,
    now_ms: i64,
    phase: InitializationPhase,
    summary: impl Into<String>,
) {
    let event = state
        .initialization
        .lock()
        .ok()
        .map(|mut feed| feed.record(now_ms, phase, summary.into()));
    if let Some(event) = event {
        let _ = app.emit(INITIALIZATION_PROGRESS_EVENT, event);
    }
}

fn begin_initialization(app: &tauri::AppHandle, state: &AppState, now_ms: i64) {
    let event = state
        .initialization
        .lock()
        .ok()
        .map(|mut feed| feed.begin(now_ms));
    if let Some(event) = event {
        let _ = app.emit(INITIALIZATION_PROGRESS_EVENT, event);
    }
}

fn coalesce_recent_events(
    sessions: &mut [SessionSnapshot],
    display: &mut HashMap<String, DisplayedRecentEvent>,
    observed_at_ms: i64,
) {
    let visible_ids = sessions
        .iter()
        .map(|session| session.thread_id.clone())
        .collect::<HashSet<_>>();
    display.retain(|thread_id, _| visible_ids.contains(thread_id));

    for session in sessions {
        let Some(candidate) = session.recent_event.clone() else {
            display.remove(&session.thread_id);
            continue;
        };

        let Some(current) = display.get_mut(&session.thread_id) else {
            display.insert(
                session.thread_id.clone(),
                DisplayedRecentEvent {
                    event: candidate,
                    displayed_at_ms: observed_at_ms,
                },
            );
            continue;
        };

        if candidate.occurred_at_ms > current.event.occurred_at_ms
            && observed_at_ms - current.displayed_at_ms >= RECENT_EVENT_COALESCE_MS
        {
            current.event = candidate;
            current.displayed_at_ms = observed_at_ms;
        } else {
            session.recent_event = Some(current.event.clone());
        }
    }
}

pub fn snapshot_for_home(codex_home: &Path, now_ms: i64) -> Result<AppSnapshot> {
    let mut scan_cache = ScanCache::default();
    let scan = scan_active_sessions_with_cache(codex_home, now_ms, &mut scan_cache)?;
    let mut quota_source_cache = QuotaSourceCache::default();
    let weekly_quota = quota_source_cache
        .latest_weekly_quota(codex_home, now_ms)
        .or(scan.weekly_quota);
    Ok(AppSnapshot {
        sessions: scan.sessions,
        weekly_quota,
        is_loading: false,
        initialization: InitializationSnapshot::default(),
        monitoring: MonitoringView {
            enabled: false,
            needs_repair: false,
            stale_count: 0,
            degraded_reason: None,
        },
        always_on_top: false,
        launch_at_login: false,
        locale: LocaleMode::System,
        theme: ThemeMode::System,
    })
}

pub fn set_always_on_top_config(state: &AppState, value: bool) -> Result<()> {
    let mut current = state
        .config
        .lock()
        .map_err(|_| anyhow::anyhow!("Codex Pulse config lock is poisoned"))?;
    let mut next = current.clone();
    next.always_on_top = value;
    state.store.save(&next)?;
    *current = next;
    Ok(())
}

pub fn set_locale_config(state: &AppState, locale: LocaleMode) -> Result<()> {
    let mut current = state
        .config
        .lock()
        .map_err(|_| anyhow::anyhow!("Codex Pulse config lock is poisoned"))?;
    let mut next = current.clone();
    next.locale = locale;
    state.store.save(&next)?;
    *current = next;
    Ok(())
}

#[tauri::command]
pub fn set_theme(theme: ThemeMode, state: State<'_, AppState>) -> Result<ThemeMode, String> {
    let mut current = state
        .config
        .lock()
        .map_err(|_| "Codex Pulse config lock is poisoned".to_string())?;
    let mut next = current.clone();
    next.theme = theme;
    state.store.save(&next).map_err(|error| error.to_string())?;
    *current = next;
    Ok(theme)
}

#[tauri::command]
pub fn set_locale(locale: LocaleMode, state: State<'_, AppState>) -> Result<LocaleMode, String> {
    set_locale_config(&state, locale).map_err(|error| error.to_string())?;
    Ok(locale)
}

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let (sessions, weekly_quota) = state.cached_snapshot();
    let mut snapshot = AppSnapshot {
        sessions,
        weekly_quota,
        is_loading: state.refresh_in_flight.load(Ordering::Acquire),
        initialization: state.cached_initialization(),
        monitoring: MonitoringView {
            enabled: false,
            needs_repair: false,
            stale_count: 0,
            degraded_reason: None,
        },
        always_on_top: false,
        launch_at_login: false,
        locale: LocaleMode::System,
        theme: ThemeMode::System,
    };
    let config = state
        .config
        .lock()
        .map_err(|_| "Codex Pulse config lock is poisoned".to_string())?;
    snapshot.always_on_top = config.always_on_top;
    snapshot.launch_at_login = config.launch_at_login;
    snapshot.locale = config.locale;
    snapshot.theme = config.theme;
    let hooks_installed = crate::hook_config::is_installed(&state.codex_home);
    snapshot.monitoring.enabled = config.monitoring_enabled && hooks_installed;
    snapshot.monitoring.needs_repair = config.monitoring_enabled && !hooks_installed;
    Ok(snapshot)
}

/// Runs the expensive JSONL/SQLite reconciliation away from the WebView invoke path.
pub fn schedule_refresh(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    if state.refresh_in_flight.swap(true, Ordering::AcqRel) {
        return;
    }
    begin_initialization(&app, &state, Utc::now().timestamp_millis());
    let codex_home = state.codex_home.clone();
    let app_for_scan = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            let state = app_for_scan.state::<AppState>();
            let now_ms = Utc::now().timestamp_millis();
            publish_initialization_event(
                &app_for_scan,
                &state,
                Utc::now().timestamp_millis(),
                InitializationPhase::ReadingQuota,
                "Reading bounded weekly quota observations",
            );
            let mut quota_source_cache = state
                .quota_source_cache
                .lock()
                .map_err(|_| anyhow::anyhow!("Codex Pulse quota source cache lock is poisoned"))?;
            let weekly_quota = quota_source_cache.latest_weekly_quota(&codex_home, now_ms);
            drop(quota_source_cache);
            publish_initialization_event(
                &app_for_scan,
                &state,
                Utc::now().timestamp_millis(),
                InitializationPhase::DiscoveringCandidates,
                "Discovering recent active-session candidates",
            );
            publish_initialization_event(
                &app_for_scan,
                &state,
                Utc::now().timestamp_millis(),
                InitializationPhase::ReconcilingSessions,
                "Reconciling active Codex sessions",
            );
            let mut scan_cache = state
                .scan_cache
                .lock()
                .map_err(|_| anyhow::anyhow!("Codex Pulse scan cache lock is poisoned"))?;
            let mut scan = scan_active_sessions_with_cache(&codex_home, now_ms, &mut scan_cache)?;
            scan.weekly_quota = weekly_quota.or(scan.weekly_quota);
            Ok::<_, anyhow::Error>(scan)
        })
        .await;
        let state = app.state::<AppState>();
        match result {
            Ok(Ok(mut scan)) => {
                if let Ok(mut display) = state.recent_event_display.lock() {
                    coalesce_recent_events(
                        &mut scan.sessions,
                        &mut display,
                        Utc::now().timestamp_millis(),
                    );
                }
                if let Ok(mut cached) = state.cached_snapshot.write() {
                    cached.sessions = scan.sessions;
                    cached.weekly_quota = scan.weekly_quota;
                }
                publish_initialization_event(
                    &app,
                    &state,
                    Utc::now().timestamp_millis(),
                    InitializationPhase::Complete,
                    "Active session reconciliation complete",
                );
            }
            Ok(Err(error)) => publish_initialization_event(
                &app,
                &state,
                Utc::now().timestamp_millis(),
                InitializationPhase::Failed,
                format!("Reconciliation failed: {error}"),
            ),
            Err(error) => publish_initialization_event(
                &app,
                &state,
                Utc::now().timestamp_millis(),
                InitializationPhase::Failed,
                format!("Reconciliation failed: {error}"),
            ),
        }
        state.refresh_in_flight.store(false, Ordering::Release);
        let _ = app.emit(crate::hook::SESSIONS_CHANGED_EVENT, ());
    });
}

pub fn start_fallback_reconciliation(app: tauri::AppHandle) {
    schedule_refresh(app.clone());
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(
                FALLBACK_RECONCILIATION_SECONDS,
            ))
            .await;
            schedule_refresh(app.clone());
        }
    });
}

#[tauri::command]
pub fn enable_monitoring(state: State<'_, AppState>) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let command = format!("\"{}\" __hook", executable.display());
    crate::hook_config::install(&state.codex_home, &command).map_err(|error| error.to_string())?;

    let mut current = state
        .config
        .lock()
        .map_err(|_| "Codex Pulse config lock is poisoned".to_string())?;
    let mut next = current.clone();
    next.monitoring_enabled = true;
    state.store.save(&next).map_err(|error| error.to_string())?;
    *current = next;
    Ok(())
}

#[tauri::command]
pub fn set_always_on_top(
    value: bool,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    window
        .set_always_on_top(value)
        .map_err(|error| error.to_string())?;
    if let Err(error) = set_always_on_top_config(&state, value) {
        let _ = window.set_always_on_top(!value);
        return Err(error.to_string());
    }
    Ok(value)
}

#[tauri::command]
pub fn open_thread(thread_id: String, app: tauri::AppHandle) -> Result<(), String> {
    let url = crate::deep_link::thread_url(&thread_id).map_err(|error| error.to_string())?;
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|error| error.to_string())
}

fn validate_project_path(path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("Project path is empty".into());
    }
    let path = PathBuf::from(path);
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("Could not access project path {}: {error}", path.display()))?;
    if !metadata.is_dir() {
        return Err(format!(
            "Project path is not a directory: {}",
            path.display()
        ));
    }
    Ok(path)
}

#[tauri::command]
pub fn open_project_path(path: String, app: tauri::AppHandle) -> Result<(), String> {
    let path = validate_project_path(&path)?;
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<String>)
        .map_err(|error| error.to_string())
}

fn validate_external_url(value: &str) -> Result<(), String> {
    let parsed =
        url::Url::parse(value).map_err(|error| format!("Invalid external URL: {error}"))?;
    if matches!(parsed.scheme(), "http" | "https" | "mailto") {
        Ok(())
    } else {
        Err(format!(
            "Unsupported external URL scheme: {}",
            parsed.scheme()
        ))
    }
}

#[tauri::command]
pub fn open_external_url(url: String, app: tauri::AppHandle) -> Result<(), String> {
    validate_external_url(&url)?;
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        coalesce_recent_events, set_always_on_top_config, set_locale_config, validate_external_url,
        validate_project_path, AppState, FALLBACK_RECONCILIATION_SECONDS,
    };
    use crate::config::ConfigStore;
    use crate::model::{LocaleMode, RecentEvent, RecentEventPriority, SessionSnapshot};

    fn session(event: Option<RecentEvent>) -> SessionSnapshot {
        SessionSnapshot {
            thread_id: "root".into(),
            title: "Root".into(),
            cwd: "/repo".into(),
            session_created_at_ms: 1_000,
            current_run_started_at_ms: 2_000,
            recent_event: event,
            last_user_message: None,
        }
    }

    #[test]
    fn holds_recent_event_updates_for_five_seconds_per_session() {
        let first = RecentEvent {
            summary: "First".into(),
            detail: None,
            occurred_at_ms: 3_000,
            priority: RecentEventPriority::Milestone,
        };
        let second = RecentEvent {
            summary: "Second".into(),
            detail: None,
            occurred_at_ms: 4_000,
            priority: RecentEventPriority::Milestone,
        };
        let mut display = HashMap::new();
        let mut initial = vec![session(Some(first.clone()))];
        coalesce_recent_events(&mut initial, &mut display, 10_000);

        let mut within_window = vec![session(Some(second.clone()))];
        coalesce_recent_events(&mut within_window, &mut display, 14_999);
        assert_eq!(within_window[0].recent_event, Some(first));

        let mut after_window = vec![session(Some(second.clone()))];
        coalesce_recent_events(&mut after_window, &mut display, 15_000);
        assert_eq!(after_window[0].recent_event, Some(second));
    }

    #[test]
    fn empty_codex_home_returns_an_empty_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(
            temp.path().to_owned(),
            ConfigStore::new(temp.path().join("config.json")),
        )
        .unwrap();
        let snapshot = super::snapshot_for_home(temp.path(), 1_784_272_200_000).unwrap();

        assert!(snapshot.sessions.is_empty());
        assert!(snapshot.weekly_quota.is_none());
        assert!(!snapshot.monitoring.enabled);
        assert_eq!(snapshot.monitoring.stale_count, 0);

        set_always_on_top_config(&state, true).unwrap();
        assert!(state.config.lock().unwrap().always_on_top);
        assert!(state.store.load().unwrap().always_on_top);
        assert!(state.cached_snapshot().0.is_empty());
    }

    #[test]
    fn accepts_only_safe_external_handoff_schemes() {
        for url in [
            "https://example.com",
            "http://example.com",
            "mailto:hello@example.com",
        ] {
            assert!(validate_external_url(url).is_ok());
        }
        for url in [
            "javascript:alert(1)",
            "file:///tmp/private",
            "codex://thread/123",
        ] {
            assert!(validate_external_url(url).is_err());
        }
    }

    #[test]
    fn validates_project_directories_before_opening() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("notes.txt");
        std::fs::write(&file, "not a directory").unwrap();
        let missing = temp.path().join("missing");

        assert_eq!(
            validate_project_path(temp.path().to_str().unwrap()).unwrap(),
            temp.path()
        );
        assert!(validate_project_path("   ").unwrap_err().contains("empty"));
        assert!(validate_project_path(missing.to_str().unwrap())
            .unwrap_err()
            .contains("Could not access project path"));
        assert!(validate_project_path(file.to_str().unwrap())
            .unwrap_err()
            .contains("not a directory"));
    }

    #[test]
    fn fallback_reconciliation_is_limited_to_one_minute() {
        assert_eq!(FALLBACK_RECONCILIATION_SECONDS, 60);
    }

    #[test]
    fn locale_changes_persist_through_app_state() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(
            temp.path().to_owned(),
            ConfigStore::new(temp.path().join("config.json")),
        )
        .unwrap();

        set_locale_config(&state, LocaleMode::German).unwrap();

        assert_eq!(state.config.lock().unwrap().locale, LocaleMode::German);
        assert_eq!(state.store.load().unwrap().locale, LocaleMode::German);
    }

    #[test]
    fn snapshot_for_home_exposes_the_latest_weekly_quota() {
        let temp = tempfile::tempdir().unwrap();
        let sessions = temp.path().join("sessions/2026/07/17");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(
            sessions.join("root.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"root\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn-1\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"primary\":{\"used_percent\":81.0,\"window_minutes\":10080,\"resets_at\":1784870653}}}}\n"
            ),
        )
        .unwrap();

        let snapshot = super::snapshot_for_home(temp.path(), 1_784_272_200_000).unwrap();

        assert_eq!(snapshot.weekly_quota.as_ref().unwrap().used_percent, 81);
        assert_eq!(
            snapshot.weekly_quota.as_ref().unwrap().remaining_percent,
            19
        );
        assert_eq!(
            snapshot.weekly_quota.as_ref().unwrap().resets_at_ms,
            1_784_870_653_000
        );
    }
}
