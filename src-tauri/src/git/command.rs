use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

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
}

impl ProcessGitRunner {
    pub fn new(executable: PathBuf, timeout: Duration) -> Self {
        Self {
            executable,
            timeout,
        }
    }
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

        let deadline = Instant::now() + self.timeout;
        loop {
            match child.try_wait().map_err(GitCommandError::Wait)? {
                Some(_) => {
                    let output = child.wait_with_output().map_err(GitCommandError::Wait)?;
                    return Ok(GitCommandOutput {
                        status_code: output.status.code(),
                        stdout: output.stdout,
                        stderr: output.stderr,
                    });
                }
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(GitCommandError::Timeout);
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf, time::Duration};

    use super::{GitCommandError, GitRunner, ProcessGitRunner};

    #[test]
    fn captures_a_successful_process_without_a_shell() {
        let runner =
            ProcessGitRunner::new(PathBuf::from("/usr/bin/printf"), Duration::from_secs(1));
        let output = runner
            .run(
                std::path::Path::new("/"),
                &[OsString::from("%s"), OsString::from("git-output")],
            )
            .unwrap();

        assert!(output.success());
        assert_eq!(output.stdout_text().unwrap(), "git-output");
    }

    #[test]
    fn kills_a_process_after_the_deadline() {
        let runner = ProcessGitRunner::new(PathBuf::from("/bin/sleep"), Duration::from_millis(20));
        let error = runner
            .run(std::path::Path::new("/"), &[OsString::from("1")])
            .unwrap_err();

        assert!(matches!(error, GitCommandError::Timeout));
    }
}
