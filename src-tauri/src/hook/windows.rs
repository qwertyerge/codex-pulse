use std::{
    collections::hash_map::DefaultHasher,
    fs::OpenOptions,
    hash::{Hash, Hasher},
    path::Path,
};

use tauri::{AppHandle, Emitter, Manager};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

pub(super) fn endpoint_name() -> String {
    let scope = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    endpoint_name_for(&scope)
}

fn endpoint_name_for(scope: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    scope.hash(&mut hasher);
    format!(
        r"\\.\pipe\com.codexpulse.desktop.{:016x}.events",
        hasher.finish()
    )
}

pub fn notify_running_instance() {
    notify_at(&notification_endpoint_name());
}

pub fn start_listener(app: AppHandle) -> anyhow::Result<()> {
    tauri::async_runtime::spawn(async move {
        let mut listener = match PipeListener::bind_at(endpoint_name()).await {
            Ok(listener) => listener,
            Err(error) => {
                report_listener_unavailable(&app, error);
                return;
            }
        };
        let app_for_refresh = app.clone();
        run_listener_loop(
            &mut listener,
            move || crate::commands::schedule_refresh(app_for_refresh.clone()),
            move |error| report_listener_failure(&app, error),
        )
        .await;
    });
    Ok(())
}

fn report_listener_unavailable(app: &AppHandle, error: std::io::Error) {
    app.state::<crate::commands::AppState>()
        .set_monitoring_degraded_reason(format!("Live hook listener unavailable: {error}"));
    let _ = app.emit(crate::hook::SESSIONS_CHANGED_EVENT, ());
}

trait HookListener {
    async fn accept(&mut self) -> std::io::Result<()>;
}

async fn run_listener_loop<L, N, F>(listener: &mut L, mut on_notification: N, mut on_failure: F)
where
    L: HookListener,
    N: FnMut(),
    F: FnMut(std::io::Error),
{
    loop {
        match listener.accept().await {
            Ok(()) => on_notification(),
            Err(error) => {
                on_failure(error);
                break;
            }
        }
    }
}

fn report_listener_failure(app: &AppHandle, error: std::io::Error) {
    app.state::<crate::commands::AppState>()
        .set_monitoring_degraded_reason(format!("Live hook listener stopped: {error}"));
    let _ = app.emit(crate::hook::SESSIONS_CHANGED_EVENT, ());
}

fn notification_endpoint_name() -> String {
    #[cfg(debug_assertions)]
    if let Ok(endpoint) = std::env::var("CODEX_PULSE_TEST_HOOK_ENDPOINT") {
        return endpoint;
    }

    endpoint_name()
}

fn notify_at(endpoint: &str) {
    let _ = OpenOptions::new().read(true).write(true).open(endpoint);
}

fn replace_server_with<T>(
    server: &mut T,
    create_replacement: impl FnOnce() -> std::io::Result<T>,
) -> std::io::Result<()> {
    let replacement = create_replacement()?;
    let connected = std::mem::replace(server, replacement);
    drop(connected);
    Ok(())
}

struct PipeListener {
    endpoint: String,
    server: NamedPipeServer,
}

impl PipeListener {
    async fn bind_at(endpoint: String) -> std::io::Result<Self> {
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&endpoint)?;
        Ok(Self { endpoint, server })
    }

    async fn accept(&mut self) -> std::io::Result<()> {
        self.server.connect().await?;
        replace_server_with(&mut self.server, || {
            ServerOptions::new().create(&self.endpoint)
        })
    }
}

impl HookListener for PipeListener {
    async fn accept(&mut self) -> std::io::Result<()> {
        PipeListener::accept(self).await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        collections::VecDeque,
        io,
        path::Path,
        rc::Rc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::{
        endpoint_name_for, notify_at, replace_server_with, run_listener_loop, HookListener,
        PipeListener,
    };

    struct InjectedListener {
        results: VecDeque<io::Result<()>>,
    }

    impl HookListener for InjectedListener {
        async fn accept(&mut self) -> io::Result<()> {
            self.results
                .pop_front()
                .expect("the loop should stop after the injected failure")
        }
    }

    struct DropProbe {
        events: Rc<RefCell<Vec<&'static str>>>,
        event: &'static str,
    }

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.events.borrow_mut().push(self.event);
        }
    }

    #[test]
    fn derives_a_deterministic_endpoint_that_separates_user_scopes() {
        let endpoint = endpoint_name_for(Path::new("CodexPulseTestScope"));
        let repeated = endpoint_name_for(Path::new("CodexPulseTestScope"));
        let other_scope = endpoint_name_for(Path::new("OtherCodexPulseTestScope"));

        assert!(endpoint.starts_with(r"\\.\pipe\com.codexpulse.desktop."));
        assert!(endpoint.ends_with(".events"));
        assert_eq!(endpoint, repeated);
        assert_ne!(endpoint, other_scope);

        let hash = endpoint
            .strip_prefix(r"\\.\pipe\com.codexpulse.desktop.")
            .and_then(|value| value.strip_suffix(".events"))
            .unwrap();
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn constructing_a_listener_future_does_not_require_a_runtime() {
        let unique_id = uuid::Uuid::from_u128(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                ^ ((std::process::id() as u128) << 96),
        );
        let endpoint = format!(r"\\.\pipe\com.codexpulse.desktop.test.{unique_id}.events");

        let future = PipeListener::bind_at(endpoint);
        drop(future);
    }

    #[test]
    fn creates_the_replacement_before_dropping_the_connected_server() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut server = DropProbe {
            events: events.clone(),
            event: "drop connected",
        };

        replace_server_with(&mut server, || {
            events.borrow_mut().push("create replacement");
            Ok(DropProbe {
                events: events.clone(),
                event: "drop replacement",
            })
        })
        .unwrap();

        assert_eq!(
            events.borrow().as_slice(),
            ["create replacement", "drop connected"]
        );
    }

    #[tokio::test]
    async fn accepts_a_second_connection_on_a_replacement_server_instance() {
        let unique_id = uuid::Uuid::from_u128(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                ^ ((std::process::id() as u128) << 96),
        );
        let endpoint = format!(r"\\.\pipe\com.codexpulse.desktop.test.{}.events", unique_id);
        let mut listener = PipeListener::bind_at(endpoint.clone()).await.unwrap();

        notify_at(&endpoint);
        tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("first connection should be accepted within five seconds")
            .unwrap();

        notify_at(&endpoint);
        tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("replacement server should accept within five seconds")
            .unwrap();
    }

    #[tokio::test]
    async fn reports_a_terminal_failure_after_the_listener_has_accepted_a_notification() {
        let mut listener = InjectedListener {
            results: VecDeque::from([
                Ok(()),
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "replacement instance failed",
                )),
            ]),
        };
        let mut notifications = 0;
        let mut failures = Vec::new();

        run_listener_loop(
            &mut listener,
            || notifications += 1,
            |error| failures.push(error.to_string()),
        )
        .await;

        assert_eq!(notifications, 1);
        assert_eq!(failures, ["replacement instance failed"]);
    }
}
