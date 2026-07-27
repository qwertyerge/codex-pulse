use std::fs::OpenOptions;

use tauri::AppHandle;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

pub(super) fn endpoint_name() -> String {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let scope = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    let mut hasher = DefaultHasher::new();
    scope.hash(&mut hasher);
    format!(
        r"\\.\pipe\com.codexpulse.desktop.{:016x}.events",
        hasher.finish()
    )
}

pub fn notify_running_instance() {
    notify_at(&endpoint_name());
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

fn notify_at(endpoint: &str) {
    let _ = OpenOptions::new().read(true).write(true).open(endpoint);
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
        let next = ServerOptions::new().create(&self.endpoint)?;
        let connected = std::mem::replace(&mut self.server, next);
        drop(connected);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{notify_at, PipeListener};

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
