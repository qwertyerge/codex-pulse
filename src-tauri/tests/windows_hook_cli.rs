#![cfg(windows)]

use std::time::Duration;

use tokio::net::windows::named_pipe::ServerOptions;

#[tokio::test]
async fn hook_subcommand_connects_to_the_user_pipe_and_exits() {
    let endpoint = codex_pulse::hook::windows_endpoint_name();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&endpoint)
        .unwrap();

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_CodexPulse"))
        .arg("__hook")
        .spawn()
        .unwrap();

    tokio::time::timeout(Duration::from_secs(5), server.connect())
        .await
        .expect("hook should connect within five seconds")
        .unwrap();
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("hook should exit within five seconds")
        .unwrap();
    assert!(status.success());
}
