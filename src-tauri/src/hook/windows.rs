use std::{
    collections::hash_map::DefaultHasher,
    fs::OpenOptions,
    hash::{Hash, Hasher},
    path::Path,
};

use tauri::AppHandle;
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
    let mut listener = PipeListener::bind_at(endpoint_name())?;
    tauri::async_runtime::spawn(async move {
        while listener.accept().await.is_ok() {
            crate::commands::schedule_refresh(app.clone());
        }
    });
    Ok(())
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
    fn bind_at(endpoint: String) -> std::io::Result<Self> {
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

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        path::Path,
        rc::Rc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::{endpoint_name_for, notify_at, replace_server_with, PipeListener};

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
    fn derives_a_deterministic_endpoint_from_a_literal_scope() {
        let endpoint = endpoint_name_for(Path::new("CodexPulseTestScope"));

        assert!(endpoint.starts_with(r"\\.\pipe\com.codexpulse.desktop."));
        assert_eq!(
            endpoint,
            r"\\.\pipe\com.codexpulse.desktop.3555edab93826d28.events"
        );
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
        let mut listener = PipeListener::bind_at(endpoint.clone()).unwrap();

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
}
