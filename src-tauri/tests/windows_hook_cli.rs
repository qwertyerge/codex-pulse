#![cfg(windows)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::windows::named_pipe::ServerOptions;

#[tokio::test]
async fn hook_subcommand_connects_to_the_user_pipe_and_exits() {
    let unique_id = uuid::Uuid::from_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            ^ ((std::process::id() as u128) << 96),
    );
    let endpoint = format!(r"\\.\pipe\com.codexpulse.desktop.test.{}.events", unique_id);
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&endpoint)
        .unwrap();

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_CodexPulse"))
        .arg("__hook")
        .env("CODEX_PULSE_TEST_HOOK_ENDPOINT", &endpoint)
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
