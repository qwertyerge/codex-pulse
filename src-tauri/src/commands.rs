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

use crate::config::{AppConfig, ConfigStore};
use crate::model::{AppSnapshot, MonitoringView, RecentEvent, SessionSnapshot};
use crate::monitor::scan_active_sessions;

pub struct AppState {
    pub codex_home: PathBuf,
    pub store: ConfigStore,
    pub config: Mutex<AppConfig>,
    sessions: RwLock<Vec<SessionSnapshot>>,
    recent_event_display: Mutex<HashMap<String, DisplayedRecentEvent>>,
    refresh_in_flight: AtomicBool,
}

const RECENT_EVENT_COALESCE_MS: i64 = 5_000;

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
            sessions: RwLock::new(Vec::new()),
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
            sessions: RwLock::new(Vec::new()),
            recent_event_display: Mutex::new(HashMap::new()),
            refresh_in_flight: AtomicBool::new(false),
        })
    }

    fn cached_sessions(&self) -> Vec<SessionSnapshot> {
        self.sessions
            .read()
            .map(|sessions| sessions.clone())
            .unwrap_or_default()
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
    Ok(AppSnapshot {
        sessions: scan_active_sessions(codex_home, now_ms)?,
        is_loading: false,
        monitoring: MonitoringView {
            enabled: false,
            needs_repair: false,
            stale_count: 0,
            degraded_reason: None,
        },
        always_on_top: false,
        launch_at_login: false,
        locale: "system".into(),
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

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let mut snapshot = AppSnapshot {
        sessions: state.cached_sessions(),
        is_loading: state.refresh_in_flight.load(Ordering::Acquire),
        monitoring: MonitoringView {
            enabled: false,
            needs_repair: false,
            stale_count: 0,
            degraded_reason: None,
        },
        always_on_top: false,
        launch_at_login: false,
        locale: "system".into(),
    };
    let config = state
        .config
        .lock()
        .map_err(|_| "Codex Pulse config lock is poisoned".to_string())?;
    snapshot.always_on_top = config.always_on_top;
    snapshot.launch_at_login = config.launch_at_login;
    snapshot.locale = config.locale.clone();
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
    let codex_home = state.codex_home.clone();
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            scan_active_sessions(&codex_home, Utc::now().timestamp_millis())
        })
        .await;
        let state = app.state::<AppState>();
        if let Ok(Ok(mut sessions)) = result {
            if let Ok(mut display) = state.recent_event_display.lock() {
                coalesce_recent_events(&mut sessions, &mut display, Utc::now().timestamp_millis());
            }
            if let Ok(mut cached) = state.sessions.write() {
                *cached = sessions;
            }
        }
        state.refresh_in_flight.store(false, Ordering::Release);
        let _ = app.emit(crate::hook::SESSIONS_CHANGED_EVENT, ());
    });
}

pub fn start_fallback_reconciliation(app: tauri::AppHandle) {
    schedule_refresh(app.clone());
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{coalesce_recent_events, set_always_on_top_config, AppState};
    use crate::config::ConfigStore;
    use crate::model::{RecentEvent, RecentEventPriority, SessionSnapshot};

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
        assert!(!snapshot.monitoring.enabled);
        assert_eq!(snapshot.monitoring.stale_count, 0);

        set_always_on_top_config(&state, true).unwrap();
        assert!(state.config.lock().unwrap().always_on_top);
        assert!(state.store.load().unwrap().always_on_top);
        assert!(state.cached_sessions().is_empty());
    }
}
