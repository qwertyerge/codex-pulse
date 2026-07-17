use tauri::Manager;

pub fn product_name() -> &'static str {
    "Codex Pulse"
}

pub fn run() -> anyhow::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_liquid_glass::init())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            crate::tray::show_main_window(app);
        }))
        .manage(crate::commands::AppState::from_environment())
        .setup(|app| {
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            app.manage(crate::tray::TrayState::default());
            crate::tray::setup(app)?;
            create_main_window(app)?;
            crate::hook::start_listener(app.handle().clone())?;
            crate::commands::start_fallback_reconciliation(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_snapshot,
            crate::commands::set_always_on_top,
            crate::commands::set_theme,
            crate::commands::set_locale,
            crate::commands::enable_monitoring,
            crate::commands::open_thread,
            crate::commands::open_external_url
        ])
        .run(tauri::generate_context!())
        .map_err(|error| anyhow::anyhow!(error))
}

fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{LogicalSize, WebviewUrl, WebviewWindowBuilder};
    use tauri_plugin_liquid_glass::{LiquidGlassConfig, LiquidGlassExt};

    let always_on_top = app
        .state::<crate::commands::AppState>()
        .config
        .lock()
        .map(|config| config.always_on_top)
        .unwrap_or_default();

    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title(product_name())
        .inner_size(360.0, 420.0)
        .min_inner_size(320.0, 360.0)
        .max_inner_size(480.0, 10_000.0)
        .transparent(true)
        .decorations(true)
        .always_on_top(always_on_top)
        .resizable(true)
        .build()?;

    // Do not hard-code a desktop height: a utility window may grow to the
    // current display's usable area while leaving a small safety margin.
    if let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) {
        let work_area = monitor.work_area().size;
        let scale_factor = monitor.scale_factor();
        let max_height = ((work_area.height as f64 / scale_factor) - 16.0).max(360.0);
        window.set_max_size(Some(LogicalSize::new(480.0, max_height)))?;
    }
    let _ = window.maximize();

    let _ = app.liquid_glass().set_effect(
        &window,
        LiquidGlassConfig {
            corner_radius: 22.0,
            tint_color: Some("#10131d24".into()),
            ..Default::default()
        },
    );
    let window_for_close = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_for_close.hide();
        }
    });
    Ok(())
}
