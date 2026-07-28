use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime},
};

use anyhow::{bail, Context};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct AppServerClient {
    _child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next_request_id: u64,
}

impl AppServerClient {
    pub async fn connect(candidates: &[PathBuf]) -> anyhow::Result<Self> {
        let mut failures = Vec::new();
        for candidate in candidates {
            match Self::connect_candidate(candidate).await {
                Ok(client) => return Ok(client),
                Err(error) => failures.push(format!("{}: {error:#}", candidate.display())),
            }
        }

        bail!(
            "no Codex App Server candidate completed the handshake: {}",
            failures.join("; ")
        )
    }

    async fn connect_candidate(executable: &Path) -> anyhow::Result<Self> {
        let mut command = Command::new(executable);
        command
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        configure_hidden_process(&mut command);

        let mut child = command
            .spawn()
            .with_context(|| format!("start {}", executable.display()))?;
        let stdin = child.stdin.take().context("Codex App Server stdin")?;
        let stdout = child.stdout.take().context("Codex App Server stdout")?;
        let mut client = Self {
            _child: child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            next_request_id: 1,
        };

        let initialize_id = client.next_id();
        client
            .send(&json!({
                "id": initialize_id,
                "method": "initialize",
                "params": {
                    "clientInfo": {
                        "name": "codex-pulse",
                        "title": "Codex Pulse",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "experimentalApi": true
                    }
                }
            }))
            .await?;
        timeout(
            HANDSHAKE_TIMEOUT,
            client.wait_for_successful_response(initialize_id),
        )
        .await
        .context("Codex App Server initialize timed out")??;
        client
            .send(&json!({
                "method": "initialized",
                "params": {}
            }))
            .await?;

        Ok(client)
    }

    pub async fn request_rate_limits(&mut self) -> anyhow::Result<u64> {
        let id = self.next_id();
        self.send(&json!({
            "id": id,
            "method": "account/rateLimits/read",
            "params": {}
        }))
        .await?;
        Ok(id)
    }

    pub async fn next_message(&mut self) -> anyhow::Result<Option<String>> {
        self.lines
            .next_line()
            .await
            .context("read Codex App Server stdout")
    }

    async fn wait_for_successful_response(&mut self, expected_id: u64) -> anyhow::Result<()> {
        while let Some(line) = self.next_message().await? {
            let message: Value =
                serde_json::from_str(&line).context("parse initialize response")?;
            if message.get("id").and_then(Value::as_u64) != Some(expected_id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                bail!("initialize failed: {error}");
            }
            if message.get("result").is_none() {
                bail!("initialize response has no result");
            }
            return Ok(());
        }

        bail!("Codex App Server exited during initialize")
    }

    async fn send(&mut self, message: &Value) -> anyhow::Result<()> {
        let mut encoded = serde_json::to_vec(message).context("encode App Server request")?;
        encoded.push(b'\n');
        self.stdin
            .write_all(&encoded)
            .await
            .context("write App Server request")?;
        self.stdin.flush().await.context("flush App Server request")
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        id
    }
}

pub fn default_codex_candidates() -> Vec<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    codex_candidates(local_app_data.as_deref())
}

fn codex_candidates(local_app_data: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("codex")];
    let Some(local_app_data) = local_app_data else {
        return candidates;
    };
    let cache_root = local_app_data.join("OpenAI/Codex/bin");
    let Ok(entries) = std::fs::read_dir(cache_root) else {
        return candidates;
    };
    let cached = entries
        .flatten()
        .filter_map(|entry| {
            let executable = entry.path().join("codex.exe");
            executable.is_file().then(|| {
                let modified = executable
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (modified, executable)
            })
        })
        .collect();
    candidates.extend(sort_cached_candidates(cached));
    candidates
}

fn sort_cached_candidates(mut candidates: Vec<(SystemTime, PathBuf)>) -> Vec<PathBuf> {
    candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    candidates.into_iter().map(|(_, path)| path).collect()
}

#[cfg(windows)]
fn configure_hidden_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_hidden_process(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::OsString,
        path::{Path, PathBuf},
        process::Command,
        sync::OnceLock,
        time::{Duration, SystemTime},
    };

    use super::{codex_candidates, sort_cached_candidates, AppServerClient};
    use crate::codex::rate_limits::weekly_quota_from_message;

    struct ProcessFixture {
        _directory: tempfile::TempDir,
        executable: PathBuf,
    }

    fn process_fixture() -> &'static Path {
        static FIXTURE: OnceLock<ProcessFixture> = OnceLock::new();

        &FIXTURE
            .get_or_init(|| {
                let directory = tempfile::tempdir().unwrap();
                let executable = directory.path().join(format!(
                    "codex-app-server-fixture{}",
                    env::consts::EXE_SUFFIX
                ));
                let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/codex_app_server.rs");
                let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
                let output = Command::new(rustc)
                    .arg(source)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .expect("app-server fixture compiler starts");
                assert!(
                    output.status.success(),
                    "app-server fixture compiles: {}",
                    String::from_utf8_lossy(&output.stderr)
                );

                ProcessFixture {
                    _directory: directory,
                    executable,
                }
            })
            .executable
    }

    #[test]
    fn path_candidate_precedes_newest_cached_windows_binaries() {
        let local_app_data = tempfile::tempdir().unwrap();
        let older = local_app_data.path().join("OpenAI/Codex/bin/older");
        let newer = local_app_data.path().join("OpenAI/Codex/bin/newer");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::create_dir_all(&newer).unwrap();
        std::fs::write(older.join("codex.exe"), []).unwrap();
        std::fs::write(newer.join("codex.exe"), []).unwrap();

        let candidates = codex_candidates(Some(local_app_data.path()));

        assert_eq!(candidates.first(), Some(&PathBuf::from("codex")));
        assert!(candidates.contains(&older.join("codex.exe")));
        assert!(candidates.contains(&newer.join("codex.exe")));
    }

    #[test]
    fn cached_candidates_are_sorted_newest_first() {
        let epoch = SystemTime::UNIX_EPOCH;
        let sorted = sort_cached_candidates(vec![
            (epoch + Duration::from_secs(1), PathBuf::from("older")),
            (epoch + Duration::from_secs(3), PathBuf::from("newest")),
            (epoch + Duration::from_secs(2), PathBuf::from("middle")),
        ]);

        assert_eq!(
            sorted,
            vec![
                PathBuf::from("newest"),
                PathBuf::from("middle"),
                PathBuf::from("older")
            ]
        );
    }

    #[tokio::test]
    async fn falls_back_to_a_candidate_that_completes_the_handshake() {
        let mut client = AppServerClient::connect(&[
            PathBuf::from("definitely-missing-codex-pulse-test-binary"),
            process_fixture().to_path_buf(),
        ])
        .await
        .expect("fixture completes handshake");

        client
            .request_rate_limits()
            .await
            .expect("rate-limit request is written");
        let response = client
            .next_message()
            .await
            .expect("response read succeeds")
            .expect("response is present");
        let quota = weekly_quota_from_message(&response, 100)
            .unwrap()
            .expect("response contains quota");

        assert_eq!(quota.remaining_percent, 95);
    }

    #[tokio::test]
    async fn receives_server_sent_rate_limit_notifications() {
        let mut client = AppServerClient::connect(&[process_fixture().to_path_buf()])
            .await
            .expect("fixture completes handshake");

        client.request_rate_limits().await.unwrap();
        client.next_message().await.unwrap().unwrap();
        let notification = client
            .next_message()
            .await
            .expect("notification read succeeds")
            .expect("notification is present");
        let quota = weekly_quota_from_message(&notification, 200)
            .unwrap()
            .expect("notification contains quota");

        assert_eq!(quota.remaining_percent, 94);
    }
}
