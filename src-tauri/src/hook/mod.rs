use tauri::AppHandle;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

pub const SESSIONS_CHANGED_EVENT: &str = "sessions-changed";

pub fn notify_running_instance() {
    platform::notify_running_instance();
}

pub fn start_listener(app: AppHandle) -> anyhow::Result<()> {
    platform::start_listener(app)
}

#[cfg(unix)]
pub use unix::socket_path;

#[cfg(windows)]
#[doc(hidden)]
pub fn windows_endpoint_name() -> String {
    windows::endpoint_name()
}
