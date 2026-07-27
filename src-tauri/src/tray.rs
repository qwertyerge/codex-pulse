use std::sync::Mutex;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    App, Manager,
};

#[cfg(target_os = "macos")]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-template.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");

const SHOW_ID: &str = "show";
const QUIT_ID: &str = "quit";

fn tray_icon_is_template() -> bool {
    cfg!(target_os = "macos")
}

/// Keeps the reference-counted tray icon alive for the life of the app.
/// Tauri removes it from the status bar when its last instance is dropped.
pub struct TrayState(pub Mutex<Option<tauri::tray::TrayIcon<tauri::Wry>>>);

impl Default for TrayState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

pub fn setup(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show Codex Pulse", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit Codex Pulse", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;
    let icon = Image::from_bytes(TRAY_ICON_BYTES)?;

    let tray_icon = TrayIconBuilder::with_id("codex-pulse")
        .icon(icon)
        .icon_as_template(tray_icon_is_template())
        .tooltip("Codex Pulse")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => show_main_window(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, tauri::tray::TrayIconEvent::Click { .. }) {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    if let Ok(mut tray) = app.state::<TrayState>().0.lock() {
        *tray = Some(tray_icon);
    }
    Ok(())
}

pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::{tray_icon_is_template, QUIT_ID, SHOW_ID, TRAY_ICON_BYTES};

    #[test]
    fn tray_actions_have_stable_ids() {
        assert_eq!(SHOW_ID, "show");
        assert_eq!(QUIT_ID, "quit");
    }

    #[test]
    fn tray_icon_uses_the_platform_appropriate_asset() {
        assert_eq!(tray_icon_is_template(), cfg!(target_os = "macos"));
        assert!(tauri::image::Image::from_bytes(TRAY_ICON_BYTES).is_ok());
    }
}
