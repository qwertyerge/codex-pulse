use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[cfg(test)]
use std::sync::mpsc::SyncSender;

#[derive(Debug)]
pub enum GitCommandError {
    Spawn(std::io::Error),
    Wait(std::io::Error),
    Timeout,
    Utf8(std::string::FromUtf8Error),
}

impl std::fmt::Display for GitCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(formatter, "could not start Git: {error}"),
            Self::Wait(error) => write!(formatter, "could not wait for Git: {error}"),
            Self::Timeout => formatter.write_str("Git command timed out"),
            Self::Utf8(error) => write!(formatter, "Git output is not UTF-8: {error}"),
        }
    }
}

impl std::error::Error for GitCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) | Self::Wait(error) => Some(error),
            Self::Utf8(error) => Some(error),
            Self::Timeout => None,
        }
    }
}

#[derive(Debug)]
pub struct GitCommandOutput {
    pub status_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl GitCommandOutput {
    pub fn success(&self) -> bool {
        self.status_code == Some(0)
    }

    pub fn stdout_text(&self) -> Result<String, GitCommandError> {
        String::from_utf8(self.stdout.clone()).map_err(GitCommandError::Utf8)
    }

    pub fn stderr_text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

pub trait GitRunner {
    fn run(&self, cwd: &Path, args: &[OsString]) -> Result<GitCommandOutput, GitCommandError>;
}

pub struct ProcessGitRunner {
    executable: PathBuf,
    timeout: Duration,
    reap_timeout: Duration,
    #[cfg(test)]
    reaper_completed_tx: Option<SyncSender<()>>,
}

impl ProcessGitRunner {
    pub fn new(executable: PathBuf, timeout: Duration) -> Self {
        Self {
            executable,
            timeout,
            reap_timeout: Duration::from_millis(100),
            #[cfg(test)]
            reaper_completed_tx: None,
        }
    }
}

fn read_pipe(mut pipe: impl Read) -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn collect_pipe(
    reader: JoinHandle<Result<Vec<u8>, std::io::Error>>,
) -> Result<Vec<u8>, GitCommandError> {
    reader
        .join()
        .map_err(|_| GitCommandError::Wait(std::io::Error::other("could not collect Git output")))?
        .map_err(GitCommandError::Wait)
}

fn terminate_and_reap(mut child: Child, reap_timeout: Duration) -> Option<Child> {
    let _ = child.kill();
    let deadline = Instant::now() + reap_timeout;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() >= deadline => return Some(child),
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return Some(child),
        }
    }
}

fn finish_reaping(
    child: Option<Child>,
    stdout_reader: JoinHandle<Result<Vec<u8>, std::io::Error>>,
    stderr_reader: JoinHandle<Result<Vec<u8>, std::io::Error>>,
) {
    if let Some(mut child) = child {
        let _ = child.wait();
    }
    let _ = collect_pipe(stdout_reader);
    let _ = collect_pipe(stderr_reader);
}

#[cfg(not(test))]
fn spawn_background_reaper(
    child: Option<Child>,
    stdout_reader: JoinHandle<Result<Vec<u8>, std::io::Error>>,
    stderr_reader: JoinHandle<Result<Vec<u8>, std::io::Error>>,
) {
    std::thread::spawn(move || finish_reaping(child, stdout_reader, stderr_reader));
}

#[cfg(test)]
fn spawn_background_reaper(
    child: Option<Child>,
    stdout_reader: JoinHandle<Result<Vec<u8>, std::io::Error>>,
    stderr_reader: JoinHandle<Result<Vec<u8>, std::io::Error>>,
    completed_tx: Option<SyncSender<()>>,
) {
    std::thread::spawn(move || {
        finish_reaping(child, stdout_reader, stderr_reader);
        if let Some(completed_tx) = completed_tx {
            let _ = completed_tx.send(());
        }
    });
}

impl GitRunner for ProcessGitRunner {
    fn run(&self, cwd: &Path, args: &[OsString]) -> Result<GitCommandOutput, GitCommandError> {
        let mut child = Command::new(&self.executable)
            .current_dir(cwd)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(GitCommandError::Spawn)?;

        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        let stdout_reader = std::thread::spawn(move || read_pipe(stdout));
        let stderr_reader = std::thread::spawn(move || read_pipe(stderr));

        let deadline = Instant::now() + self.timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    return Ok(GitCommandOutput {
                        status_code: status.code(),
                        stdout: collect_pipe(stdout_reader)?,
                        stderr: collect_pipe(stderr_reader)?,
                    });
                }
                Ok(None) if Instant::now() >= deadline => {
                    let child = terminate_and_reap(child, self.reap_timeout);
                    #[cfg(not(test))]
                    spawn_background_reaper(child, stdout_reader, stderr_reader);
                    #[cfg(test)]
                    spawn_background_reaper(
                        child,
                        stdout_reader,
                        stderr_reader,
                        self.reaper_completed_tx.clone(),
                    );
                    return Err(GitCommandError::Timeout);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let child = terminate_and_reap(child, self.reap_timeout);
                    #[cfg(not(test))]
                    spawn_background_reaper(child, stdout_reader, stderr_reader);
                    #[cfg(test)]
                    spawn_background_reaper(
                        child,
                        stdout_reader,
                        stderr_reader,
                        self.reaper_completed_tx.clone(),
                    );
                    return Err(GitCommandError::Wait(error));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::OsString,
        path::{Path, PathBuf},
        process::Command,
        sync::{mpsc, OnceLock},
        time::{Duration, Instant},
    };

    use super::{GitCommandError, GitRunner, ProcessGitRunner};

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
                    "process-git-runner-fixture{}",
                    env::consts::EXE_SUFFIX
                ));
                let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/process_git_runner.rs");
                let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
                let output = Command::new(rustc)
                    .arg(source)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .expect("process fixture compiler starts");
                assert!(
                    output.status.success(),
                    "process fixture compiles: {}",
                    String::from_utf8_lossy(&output.stderr)
                );

                ProcessFixture {
                    _directory: directory,
                    executable,
                }
            })
            .executable
    }

    fn fixture_runner(timeout: Duration) -> ProcessGitRunner {
        ProcessGitRunner::new(process_fixture().to_path_buf(), timeout)
    }

    #[test]
    fn captures_a_successful_process_without_a_shell() {
        let runner = fixture_runner(Duration::from_secs(1));
        let output = runner
            .run(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &[OsString::from("print")],
            )
            .unwrap();

        assert!(output.success());
        assert_eq!(output.stdout_text().unwrap(), "git-output");
    }

    #[test]
    fn kills_a_process_after_the_deadline() {
        let runner = fixture_runner(Duration::from_millis(20));
        let error = runner
            .run(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &[OsString::from("sleep")],
            )
            .unwrap_err();

        assert!(matches!(error, GitCommandError::Timeout));
    }

    #[test]
    fn timeout_returns_promptly_while_a_background_reaper_finishes_cleanup() {
        let fixture = tempfile::tempdir().unwrap();
        let completion_marker = fixture.path().join("completed");
        let (reaper_completed_tx, reaper_completed_rx) = mpsc::sync_channel(1);
        let runner = ProcessGitRunner {
            executable: process_fixture().to_path_buf(),
            timeout: Duration::from_millis(20),
            reap_timeout: Duration::ZERO,
            reaper_completed_tx: Some(reaper_completed_tx),
        };

        let started_at = Instant::now();
        let error = runner
            .run(fixture.path(), &[OsString::from("spawn-sleeper-then-mark")])
            .unwrap_err();
        let returned_after = started_at.elapsed();

        assert!(matches!(error, GitCommandError::Timeout));
        assert!(
            returned_after < Duration::from_millis(250),
            "timeout returned after {returned_after:?}"
        );
        assert!(!completion_marker.exists());
        reaper_completed_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("background reaper completes");
        assert!(!completion_marker.exists());
    }

    #[test]
    fn captures_large_stdout_and_stderr_without_timing_out() {
        let runner = fixture_runner(Duration::from_secs(1));
        let output = runner
            .run(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &[OsString::from("large-output")],
            )
            .unwrap();

        assert!(output.success());
        assert_eq!(output.stdout.len(), 4 * 1024 * 1024);
        assert_eq!(output.stderr.len(), 4 * 1024 * 1024);
    }

    #[test]
    fn returns_nonzero_exit_status_with_captured_stderr() {
        let runner = fixture_runner(Duration::from_secs(1));
        let output = runner
            .run(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &[OsString::from("fail")],
            )
            .unwrap();

        assert!(!output.success());
        assert_eq!(output.status_code, Some(7));
        assert_eq!(output.stderr_text_lossy(), "failure");
    }
}
