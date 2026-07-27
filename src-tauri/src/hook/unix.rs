use std::{
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::Context;
use tauri::AppHandle;

const SOCKET_FILE: &str = "events.sock";

pub fn socket_path() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("CodexPulse")
        .join(SOCKET_FILE)
}

pub fn notify_running_instance() {
    if let Ok(mut stream) = UnixStream::connect(socket_path()) {
        let _ = stream.write_all(b"refresh\n");
    }
}

pub fn start_listener(app: AppHandle) -> anyhow::Result<()> {
    let path = socket_path();
    let parent = path
        .parent()
        .context("Codex Pulse socket has no parent directory")?;
    std::fs::create_dir_all(parent)?;
    remove_stale_socket(&path)?;
    let listener = UnixListener::bind(&path).with_context(|| {
        format!(
            "Could not bind Codex Pulse event socket at {}",
            path.display()
        )
    })?;
    listener.set_nonblocking(true)?;

    thread::Builder::new()
        .name("codex-pulse-hook-listener".into())
        .spawn(move || loop {
            match listener.accept() {
                Ok((_stream, _address)) => {
                    crate::commands::schedule_refresh(app.clone());
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(_) => break,
            }
        })?;
    Ok(())
}

fn remove_stale_socket(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("Could not replace stale socket at {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{remove_stale_socket, SOCKET_FILE};

    #[test]
    fn removes_an_existing_socket_path_before_binding() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(SOCKET_FILE);
        std::fs::write(&path, "stale").unwrap();

        remove_stale_socket(&path).unwrap();

        assert!(!path.exists());
    }
}
