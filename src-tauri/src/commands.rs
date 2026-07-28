use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, RwLock,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use chrono::Utc;
use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::codex::discovery::ScanCache;
use crate::config::{AppConfig, ConfigStore};
use crate::git::{
    command::ProcessGitRunner, enrichment::GitSessionEnricher, resolver::GitRepositoryResolver,
    store::GitCacheStore,
};
use crate::initialization::{InitializationFeed, INITIALIZATION_PROGRESS_EVENT};
use crate::model::{
    AppSnapshot, InitializationPhase, InitializationSnapshot, LocaleMode, MonitoringView,
    RecentEvent, SessionSnapshot, ThemeMode, WeeklyQuota,
};
use crate::monitor::scan_active_sessions_with_cache;
use crate::quota_monitor::{spawn_quota_monitor, QuotaMonitorEvent, QuotaRefreshHandle};

pub struct AppState {
    pub codex_home: PathBuf,
    pub store: ConfigStore,
    pub config: Mutex<AppConfig>,
    monitoring_degraded_reason: RwLock<Option<String>>,
    cached_snapshot: RwLock<CachedSnapshot>,
    scan_cache: Mutex<ScanCache>,
    git_enricher: Mutex<GitSessionEnricher<GitRepositoryResolver<ProcessGitRunner>>>,
    quota_refresh: Mutex<Option<QuotaRefreshHandle>>,
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
const OFFICIAL_QUOTA_MAX_AGE_MS: i64 = 5 * 60 * 1_000;
pub const FALLBACK_RECONCILIATION_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
struct DisplayedRecentEvent {
    event: RecentEvent,
    displayed_at_ms: i64,
}

impl AppState {
    pub fn new(codex_home: PathBuf, store: ConfigStore) -> Result<Self> {
        let config = store.load()?;
        Ok(Self::with_config(codex_home, store, config))
    }

    fn with_config(codex_home: PathBuf, store: ConfigStore, config: AppConfig) -> Self {
        let cache_path = store.path().with_file_name("git-cache.sqlite3");
        let cache = GitCacheStore::open(&cache_path).ok();
        let runner = ProcessGitRunner::new(PathBuf::from("git"), Duration::from_secs(2));
        let resolver = GitRepositoryResolver::new(runner);

        Self {
            codex_home,
            store,
            config: Mutex::new(config),
            monitoring_degraded_reason: RwLock::new(None),
            cached_snapshot: RwLock::new(CachedSnapshot::default()),
            scan_cache: Mutex::new(ScanCache::default()),
            git_enricher: Mutex::new(GitSessionEnricher::new(resolver, cache)),
            quota_refresh: Mutex::new(None),
            initialization: Mutex::new(InitializationFeed::default()),
            recent_event_display: Mutex::new(HashMap::new()),
            refresh_in_flight: AtomicBool::new(false),
        }
    }

    pub fn from_environment() -> Self {
        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
            .unwrap_or_else(|| PathBuf::from(".codex"));
        let store = ConfigStore::for_user();
        Self::new(codex_home.clone(), store.clone())
            .unwrap_or_else(|_| Self::with_config(codex_home, store, AppConfig::default()))
    }

    pub fn set_monitoring_degraded_reason(&self, reason: String) {
        if let Ok(mut current) = self.monitoring_degraded_reason.write() {
            *current = Some(reason);
        }
    }

    fn monitoring_degraded_reason(&self) -> Option<String> {
        self.monitoring_degraded_reason
            .read()
            .ok()
            .and_then(|current| current.clone())
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

    fn install_quota_refresh(&self, handle: QuotaRefreshHandle) {
        if let Ok(mut current) = self.quota_refresh.lock() {
            *current = Some(handle);
        }
    }

    fn request_quota_refresh(&self) {
        if let Ok(current) = self.quota_refresh.lock() {
            if let Some(handle) = current.as_ref() {
                handle.request_refresh();
            }
        }
    }

    fn observe_weekly_quota(&self, quota: WeeklyQuota) {
        if let Ok(mut cached) = self.cached_snapshot.write() {
            cached.weekly_quota = Some(quota);
        }
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

fn replace_cached_sessions(state: &AppState, sessions: Vec<SessionSnapshot>) {
    if let Ok(mut cached) = state.cached_snapshot.write() {
        cached.sessions = sessions;
    }
}

pub fn snapshot_for_home(codex_home: &Path, now_ms: i64) -> Result<AppSnapshot> {
    let mut scan_cache = ScanCache::default();
    let scan = scan_active_sessions_with_cache(codex_home, now_ms, &mut scan_cache)?;
    Ok(AppSnapshot {
        sessions: scan.sessions,
        weekly_quota: None,
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
    snapshot_for_state_at(&state, Utc::now().timestamp_millis())
}

fn snapshot_for_state_at(state: &AppState, now_ms: i64) -> Result<AppSnapshot, String> {
    let (sessions, weekly_quota) = state.cached_snapshot();
    let weekly_quota = weekly_quota.filter(|quota| {
        quota.resets_at_ms > now_ms
            && now_ms.saturating_sub(quota.observed_at_ms) <= OFFICIAL_QUOTA_MAX_AGE_MS
    });
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
    snapshot.monitoring.degraded_reason = state.monitoring_degraded_reason();
    Ok(snapshot)
}

/// Runs the expensive JSONL/SQLite reconciliation away from the WebView invoke path.
pub fn schedule_refresh(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    state.request_quota_refresh();
    if state.refresh_in_flight.swap(true, Ordering::AcqRel) {
        return;
    }
    begin_initialization(&app, &state, Utc::now().timestamp_millis());
    publish_initialization_event(
        &app,
        &state,
        Utc::now().timestamp_millis(),
        InitializationPhase::ReadingQuota,
        "Requesting official weekly quota",
    );
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
            let mut scan = {
                let mut scan_cache = state
                    .scan_cache
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Codex Pulse scan cache lock is poisoned"))?;
                scan_active_sessions_with_cache(&codex_home, now_ms, &mut scan_cache)?
            };
            if let Ok(mut enricher) = state.git_enricher.lock() {
                enricher.enrich(&mut scan.sessions, now_ms);
            }
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
                replace_cached_sessions(&state, scan.sessions);
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

pub fn start_official_quota_monitor(app: tauri::AppHandle) {
    let (updates_tx, mut updates_rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = spawn_quota_monitor(updates_tx);
    app.state::<AppState>().install_quota_refresh(handle);

    tauri::async_runtime::spawn(async move {
        while let Some(event) = updates_rx.recv().await {
            match event {
                QuotaMonitorEvent::Observed(quota) => {
                    app.state::<AppState>().observe_weekly_quota(quota);
                    let _ = app.emit(crate::hook::SESSIONS_CHANGED_EVENT, ());
                }
            }
        }
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
    use crate::model::{
        LocaleMode, RecentEvent, RecentEventPriority, SessionSnapshot, WeeklyQuota,
    };

    fn session(event: Option<RecentEvent>) -> SessionSnapshot {
        SessionSnapshot {
            thread_id: "root".into(),
            title: "Root".into(),
            cwd: "/repo".into(),
            git: None,
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
    fn app_state_places_git_cache_beside_user_config() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("settings/config.json");
        let _state = AppState::new(
            temp.path().join("codex-home"),
            ConfigStore::new(config_path.clone()),
        )
        .unwrap();

        assert!(config_path.with_file_name("git-cache.sqlite3").is_file());
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
    fn listener_failure_is_exposed_without_disabling_fallback_monitoring() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(
            temp.path().to_owned(),
            ConfigStore::new(temp.path().join("config.json")),
        )
        .unwrap();

        state.set_monitoring_degraded_reason("Live hook listener unavailable: pipe is busy".into());
        let snapshot =
            super::snapshot_for_state_at(&state, chrono::Utc::now().timestamp_millis()).unwrap();
        assert_eq!(
            snapshot.monitoring.degraded_reason.as_deref(),
            Some("Live hook listener unavailable: pipe is busy")
        );
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
    fn transcript_quota_does_not_supply_the_official_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let sessions = temp.path().join("sessions/2026/07/17");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(
            sessions.join("root.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-07-17T07:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"root\",\"timestamp\":\"2026-07-17T07:00:00Z\",\"cwd\":\"/repo\",\"source\":\"cli\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn-1\"}}\n",
                "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"limit_id\":\"codex\",\"primary\":{\"used_percent\":81.0,\"window_minutes\":10080,\"resets_at\":1784870653}}}}\n"
            ),
        )
        .unwrap();

        let snapshot = super::snapshot_for_home(temp.path(), 1_784_272_200_000).unwrap();

        assert!(snapshot.weekly_quota.is_none());
    }

    fn state_with_quota(quota: WeeklyQuota) -> (tempfile::TempDir, AppState) {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(
            temp.path().to_owned(),
            ConfigStore::new(temp.path().join("config.json")),
        )
        .unwrap();
        state.cached_snapshot.write().unwrap().weekly_quota = Some(quota);
        (temp, state)
    }

    #[test]
    fn fresh_unexpired_official_quota_is_visible() {
        let now_ms = 1_000_000;
        let (_temp, state) = state_with_quota(WeeklyQuota {
            used_percent: 5,
            remaining_percent: 95,
            resets_at_ms: now_ms + 60_000,
            observed_at_ms: now_ms - 1_000,
        });

        let snapshot = super::snapshot_for_state_at(&state, now_ms).unwrap();

        assert_eq!(snapshot.weekly_quota.unwrap().remaining_percent, 95);
    }

    #[test]
    fn official_quota_older_than_five_minutes_is_hidden() {
        let now_ms = 1_000_000;
        let (_temp, state) = state_with_quota(WeeklyQuota {
            used_percent: 5,
            remaining_percent: 95,
            resets_at_ms: now_ms + 60_000,
            observed_at_ms: now_ms - 300_001,
        });

        let snapshot = super::snapshot_for_state_at(&state, now_ms).unwrap();

        assert!(snapshot.weekly_quota.is_none());
    }

    #[test]
    fn official_quota_at_its_reset_time_is_hidden() {
        let now_ms = 1_000_000;
        let (_temp, state) = state_with_quota(WeeklyQuota {
            used_percent: 5,
            remaining_percent: 95,
            resets_at_ms: now_ms,
            observed_at_ms: now_ms - 1_000,
        });

        let snapshot = super::snapshot_for_state_at(&state, now_ms).unwrap();

        assert!(snapshot.weekly_quota.is_none());
    }

    #[test]
    fn replacing_scanned_sessions_does_not_overwrite_official_quota() {
        let now_ms = 1_000_000;
        let (_temp, state) = state_with_quota(WeeklyQuota {
            used_percent: 5,
            remaining_percent: 95,
            resets_at_ms: now_ms + 60_000,
            observed_at_ms: now_ms - 1_000,
        });

        super::replace_cached_sessions(&state, vec![session(None)]);
        let snapshot = super::snapshot_for_state_at(&state, now_ms).unwrap();

        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.weekly_quota.unwrap().remaining_percent, 95);
    }
}
