use codex_pulse::app::register_updater_plugins;
use tauri::{
    ipc::CallbackFn,
    test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
    utils::acl::ExecutionContext,
    webview::InvokeRequest,
    WebviewWindowBuilder,
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
