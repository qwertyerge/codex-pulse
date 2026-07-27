#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    if std::env::args().nth(1).as_deref() == Some("__hook") {
        codex_pulse::hook::notify_running_instance();
        return;
    }
    if let Err(error) = codex_pulse::app::run() {
        eprintln!("Codex Pulse failed: {error:#}");
        std::process::exit(1);
    }
}
