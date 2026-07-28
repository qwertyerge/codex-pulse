use std::{path::PathBuf, time::Duration};

use chrono::Utc;
use tokio::{
    sync::mpsc,
    time::{sleep, Instant},
};

use crate::{
    codex::{
        app_server::{default_codex_candidates, AppServerClient},
        rate_limits::weekly_quota_from_message,
    },
    model::WeeklyQuota,
};

const DEBOUNCE_DURATION: Duration = Duration::from_secs(5);
const IDLE_TIMER_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone)]
pub struct QuotaRefreshHandle {
    refresh_tx: mpsc::UnboundedSender<()>,
}

impl QuotaRefreshHandle {
    pub fn request_refresh(&self) {
        let _ = self.refresh_tx.send(());
    }
}

#[derive(Debug)]
pub enum QuotaMonitorEvent {
    Observed(WeeklyQuota),
}

#[derive(Clone)]
struct MonitorConfig {
    debounce: Duration,
    reconnect_delays: Vec<Duration>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            debounce: DEBOUNCE_DURATION,
            reconnect_delays: vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(5),
                Duration::from_secs(10),
                Duration::from_secs(30),
            ],
        }
    }
}

pub fn spawn_quota_monitor(
    updates: mpsc::UnboundedSender<QuotaMonitorEvent>,
) -> QuotaRefreshHandle {
    spawn_quota_monitor_with_config(
        default_codex_candidates(),
        updates,
        MonitorConfig::default(),
    )
}

fn spawn_quota_monitor_with_config(
    candidates: Vec<PathBuf>,
    updates: mpsc::UnboundedSender<QuotaMonitorEvent>,
    config: MonitorConfig,
) -> QuotaRefreshHandle {
    let (refresh_tx, refresh_rx) = mpsc::unbounded_channel();
    tauri::async_runtime::spawn(run_monitor(candidates, updates, refresh_rx, config));
    QuotaRefreshHandle { refresh_tx }
}

async fn run_monitor(
    candidates: Vec<PathBuf>,
    updates: mpsc::UnboundedSender<QuotaMonitorEvent>,
    mut refresh_rx: mpsc::UnboundedReceiver<()>,
    config: MonitorConfig,
) {
    let mut reconnect_attempt = 0;
    loop {
        if refresh_rx.is_closed() || updates.is_closed() {
            return;
        }

        match AppServerClient::connect(&candidates).await {
            Ok(mut client) => {
                reconnect_attempt = 0;
                if run_connection(&mut client, &updates, &mut refresh_rx, config.debounce).await
                    == ConnectionEnd::Shutdown
                {
                    return;
                }
            }
            Err(error) => {
                eprintln!("Codex App Server unavailable: {error:#}");
            }
        }

        let delay = reconnect_delay(&config.reconnect_delays, reconnect_attempt);
        reconnect_attempt = reconnect_attempt.saturating_add(1);
        tokio::select! {
            _ = sleep(delay) => {}
            signal = refresh_rx.recv() => {
                if signal.is_none() {
                    return;
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionEnd {
    Disconnected,
    Shutdown,
}

async fn run_connection(
    client: &mut AppServerClient,
    updates: &mpsc::UnboundedSender<QuotaMonitorEvent>,
    refresh_rx: &mut mpsc::UnboundedReceiver<()>,
    debounce: Duration,
) -> ConnectionEnd {
    if client.request_rate_limits().await.is_err() {
        return ConnectionEnd::Disconnected;
    }

    let debounce_timer = sleep(IDLE_TIMER_DURATION);
    tokio::pin!(debounce_timer);
    let mut debounce_armed = false;

    loop {
        tokio::select! {
            signal = refresh_rx.recv() => {
                if signal.is_none() {
                    return ConnectionEnd::Shutdown;
                }
                if !debounce_armed {
                    debounce_timer.as_mut().reset(Instant::now() + debounce);
                    debounce_armed = true;
                }
            }
            _ = &mut debounce_timer, if debounce_armed => {
                debounce_armed = false;
                debounce_timer
                    .as_mut()
                    .reset(Instant::now() + IDLE_TIMER_DURATION);
                if client.request_rate_limits().await.is_err() {
                    return ConnectionEnd::Disconnected;
                }
            }
            message = client.next_message() => {
                let line = match message {
                    Ok(Some(line)) => line,
                    Ok(None) | Err(_) => return ConnectionEnd::Disconnected,
                };
                let observed_at_ms = Utc::now().timestamp_millis();
                if let Ok(Some(quota)) = weekly_quota_from_message(&line, observed_at_ms) {
                    if updates.send(QuotaMonitorEvent::Observed(quota)).is_err() {
                        return ConnectionEnd::Shutdown;
                    }
                }
            }
        }
    }
}

fn reconnect_delay(delays: &[Duration], attempt: usize) -> Duration {
    delays
        .get(attempt)
        .or_else(|| delays.last())
        .copied()
        .unwrap_or(Duration::from_secs(30))
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::OsString,
        path::{Path, PathBuf},
        process::Command,
        sync::OnceLock,
        time::Duration,
    };

    use tokio::sync::mpsc;

    use super::{
        reconnect_delay, spawn_quota_monitor_with_config, MonitorConfig, QuotaMonitorEvent,
    };

    struct ProcessFixture {
        _directory: tempfile::TempDir,
        executable: PathBuf,
    }

    fn process_fixture() -> &'static Path {
        static FIXTURE: OnceLock<ProcessFixture> = OnceLock::new();

        &FIXTURE
            .get_or_init(|| {
                let directory = tempfile::tempdir().unwrap();
                let executable = directory
                    .path()
                    .join(format!("quota-monitor-fixture{}", env::consts::EXE_SUFFIX));
                let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/codex_app_server.rs");
                let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
                let output = Command::new(rustc)
                    .arg(source)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .expect("quota fixture compiler starts");
                assert!(
                    output.status.success(),
                    "quota fixture compiles: {}",
                    String::from_utf8_lossy(&output.stderr)
                );

                ProcessFixture {
                    _directory: directory,
                    executable,
                }
            })
            .executable
    }

    fn test_config() -> MonitorConfig {
        MonitorConfig {
            debounce: Duration::from_millis(30),
            reconnect_delays: vec![Duration::from_millis(10)],
        }
    }

    #[tokio::test]
    async fn new_connection_immediately_reads_and_publishes_quota() {
        let (updates_tx, mut updates_rx) = mpsc::unbounded_channel();
        let handle = spawn_quota_monitor_with_config(
            vec![process_fixture().to_path_buf()],
            updates_tx,
            test_config(),
        );

        let event = tokio::time::timeout(Duration::from_secs(2), updates_rx.recv())
            .await
            .expect("initial read completes")
            .expect("monitor publishes an event");

        assert!(matches!(
            event,
            QuotaMonitorEvent::Observed(quota) if quota.remaining_percent == 95
        ));
        drop(handle);
    }

    #[tokio::test]
    async fn activity_requests_are_coalesced_within_one_debounce_window() {
        let (updates_tx, mut updates_rx) = mpsc::unbounded_channel();
        let handle = spawn_quota_monitor_with_config(
            vec![process_fixture().to_path_buf()],
            updates_tx,
            test_config(),
        );
        tokio::time::timeout(Duration::from_secs(2), updates_rx.recv())
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), updates_rx.recv())
            .await
            .unwrap()
            .unwrap();

        handle.request_refresh();
        handle.request_refresh();
        handle.request_refresh();

        tokio::time::timeout(Duration::from_secs(2), updates_rx.recv())
            .await
            .expect("debounced response arrives")
            .expect("response event exists");
        tokio::time::timeout(Duration::from_secs(2), updates_rx.recv())
            .await
            .expect("server notification arrives")
            .expect("notification event exists");
        assert!(
            tokio::time::timeout(Duration::from_millis(100), updates_rx.recv())
                .await
                .is_err(),
            "coalesced signals must not cause extra reads"
        );
        drop(handle);
    }

    #[test]
    fn reconnect_backoff_is_bounded() {
        let delays = vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(5),
        ];

        assert_eq!(reconnect_delay(&delays, 0), Duration::from_secs(1));
        assert_eq!(reconnect_delay(&delays, 1), Duration::from_secs(2));
        assert_eq!(reconnect_delay(&delays, 99), Duration::from_secs(5));
    }
}
