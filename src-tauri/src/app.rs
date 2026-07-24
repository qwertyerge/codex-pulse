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
            crate::commands::open_project_path,
            crate::commands::open_external_url
        ])
        .run(tauri::generate_context!())
        .map_err(|error| anyhow::anyhow!(error))
}

fn main_window_size_constraints() -> tauri::WindowSizeConstraints {
    use tauri::{LogicalUnit, WindowSizeConstraints};

    WindowSizeConstraints {
        min_width: Some(LogicalUnit::new(320.0).into()),
        min_height: Some(LogicalUnit::new(360.0).into()),
        max_width: Some(LogicalUnit::new(480.0).into()),
        max_height: None,
    }
}

fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
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
        .inner_size_constraints(main_window_size_constraints())
        .transparent(true)
        .decorations(true)
        .always_on_top(always_on_top)
        .resizable(true)
        .build()?;

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

#[cfg(test)]
mod tests {
    use tauri::{LogicalUnit, PixelUnit};

    use super::main_window_size_constraints;

    #[test]
    fn main_window_has_bounded_width_and_no_maximum_height() {
        let constraints = main_window_size_constraints();

        assert_eq!(
            constraints.min_width,
            Some(PixelUnit::Logical(LogicalUnit::new(320.0)))
        );
        assert_eq!(
            constraints.min_height,
            Some(PixelUnit::Logical(LogicalUnit::new(360.0)))
        );
        assert_eq!(
            constraints.max_width,
            Some(PixelUnit::Logical(LogicalUnit::new(480.0)))
        );
        assert_eq!(constraints.max_height, None);
    }
}
