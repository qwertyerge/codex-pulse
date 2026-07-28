use tauri::Manager;

#[cfg(target_os = "macos")]
fn configure_activation_policy(app: &mut tauri::App) {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

#[cfg(not(target_os = "macos"))]
fn configure_activation_policy(_app: &mut tauri::App) {}

pub fn product_name() -> &'static str {
    "Codex Pulse"
}

fn register_updater_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
}

pub fn run() -> anyhow::Result<()> {
    register_updater_plugins(tauri::Builder::default().plugin(tauri_plugin_opener::init()))
        .plugin(tauri_plugin_liquid_glass::init())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            crate::tray::show_main_window(app);
        }))
        .manage(crate::commands::AppState::from_environment())
        .setup(|app| {
            configure_activation_policy(app);
            app.manage(crate::tray::TrayState::default());
            crate::tray::setup(app)?;
            create_main_window(app)?;
            if let Err(error) = crate::hook::start_listener(app.handle().clone()) {
                app.state::<crate::commands::AppState>()
                    .set_monitoring_degraded_reason(format!(
                        "Live hook listener unavailable: {error:#}"
                    ));
            }
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

#[derive(Debug, PartialEq)]
struct MainWindowPlatformPolicy {
    maximizable: bool,
    maximize_on_create: bool,
}

fn main_window_platform_policy(target_os: &str) -> MainWindowPlatformPolicy {
    if target_os == "windows" {
        MainWindowPlatformPolicy {
            maximizable: false,
            maximize_on_create: false,
        }
    } else {
        MainWindowPlatformPolicy {
            maximizable: true,
            maximize_on_create: true,
        }
    }
}

fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    use tauri_plugin_liquid_glass::{LiquidGlassConfig, LiquidGlassExt};

    let platform_policy = main_window_platform_policy(std::env::consts::OS);
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
        .maximizable(platform_policy.maximizable)
        .build()?;

    if platform_policy.maximize_on_create {
        let _ = window.maximize();
    }

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
    use tauri::{
        ipc::CallbackFn,
        test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
        utils::acl::ExecutionContext,
        webview::InvokeRequest,
        LogicalUnit, PixelUnit, WebviewWindowBuilder,
    };

    use super::{
        main_window_platform_policy, main_window_size_constraints, register_updater_plugins,
    };

    #[test]
    fn production_builder_registers_updater_plugin_commands() {
        const COMMANDS: [&str; 3] = [
            "plugin:dialog|message",
            "plugin:process|exit",
            "plugin:updater|download",
        ];

        let mut context = mock_context(noop_assets());
        context
            .config_mut()
            .plugins
            .0
            .insert("updater".into(), serde_json::json!({ "pubkey": "" }));
        for command in COMMANDS {
            context
                .runtime_authority_mut()
                .__allow_command(command.into(), ExecutionContext::Local);
        }
        let app = register_updater_plugins(mock_builder())
            .build(context)
            .expect("mock app should build with production updater plugins");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview should build");
        let local_url = if cfg!(any(windows, target_os = "android")) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        };

        for command in COMMANDS {
            let response = get_ipc_response(
                &webview,
                InvokeRequest {
                    cmd: command.into(),
                    callback: CallbackFn(0),
                    error: CallbackFn(1),
                    url: local_url.parse().expect("local invoke URL should parse"),
                    body: serde_json::json!({}).into(),
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            );
            let error = response.expect_err("malformed command arguments should fail");
            let message = error
                .as_str()
                .expect("IPC rejection should be a string message");

            assert!(
                message.contains("missing required key") || message.contains("invalid args"),
                "registered malformed command should reject its arguments, got: {message}"
            );
            assert!(
                !message.contains("not found"),
                "production plugin command was not registered: {message}"
            );
        }
    }

    #[test]
    fn main_window_uses_pd_measured_bounds() {
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

    #[test]
    fn windows_main_window_starts_compact_and_cannot_maximize() {
        let policy = main_window_platform_policy("windows");

        assert!(!policy.maximizable);
        assert!(!policy.maximize_on_create);
    }

    #[test]
    fn macos_main_window_keeps_existing_maximize_behavior() {
        let policy = main_window_platform_policy("macos");

        assert!(policy.maximizable);
        assert!(policy.maximize_on_create);
    }
}
