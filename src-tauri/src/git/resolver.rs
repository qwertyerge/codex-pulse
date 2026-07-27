use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};

use crate::git::command::{GitCommandOutput, GitRunner};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeIdentity {
    pub repository_key: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRecord {
    pub repository_key: String,
    pub primary_checkout_path: String,
    pub project_name: String,
    pub default_branch: Option<String>,
    pub default_upstream: Option<String>,
    pub remote_url: Option<String>,
    pub updated_at_ms: i64,
}

pub trait GitMetadataSource {
    fn resolve_worktree(&self, cwd: &Path) -> Result<Option<WorktreeIdentity>>;
    fn resolve_repository(
        &self,
        cwd: &Path,
        identity: &WorktreeIdentity,
        now_ms: i64,
    ) -> Result<RepositoryRecord>;
}

pub struct GitRepositoryResolver<R> {
    runner: R,
}

impl<R> GitRepositoryResolver<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: GitRunner> GitMetadataSource for GitRepositoryResolver<R> {
    fn resolve_worktree(&self, cwd: &Path) -> Result<Option<WorktreeIdentity>> {
        let common_dir = self.run_required(
            cwd,
            git_args(
                cwd,
                vec![
                    OsString::from("rev-parse"),
                    OsString::from("--path-format=absolute"),
                    OsString::from("--git-common-dir"),
                ],
            ),
        )?;
        if !common_dir.success() {
            if common_dir
                .stderr_text_lossy()
                .contains("not a git repository")
            {
                return Ok(None);
            }
            return Err(anyhow!(
                "could not resolve Git common directory: {}",
                common_dir.stderr_text_lossy().trim()
            ));
        }

        let common_dir = common_dir
            .stdout_text()
            .context("Git common directory output is not UTF-8")?;
        let repository_key = std::fs::canonicalize(common_dir.trim())
            .context("could not canonicalize Git common directory")?
            .to_string_lossy()
            .into_owned();

        let branch = self.run_required(
            cwd,
            git_args(
                cwd,
                vec![
                    OsString::from("symbolic-ref"),
                    OsString::from("--quiet"),
                    OsString::from("--short"),
                    OsString::from("HEAD"),
                ],
            ),
        )?;
        let branch = match branch.status_code {
            Some(0) => Some(
                branch
                    .stdout_text()
                    .context("Git branch output is not UTF-8")?
                    .trim()
                    .to_owned(),
            ),
            Some(1) => None,
            _ => {
                return Err(anyhow!(
                    "could not resolve Git branch: {}",
                    branch.stderr_text_lossy().trim()
                ));
            }
        };

        Ok(Some(WorktreeIdentity {
            repository_key,
            branch,
        }))
    }

    fn resolve_repository(
        &self,
        cwd: &Path,
        identity: &WorktreeIdentity,
        now_ms: i64,
    ) -> Result<RepositoryRecord> {
        let worktrees = self.run_required(
            cwd,
            git_args(
                cwd,
                vec![
                    OsString::from("worktree"),
                    OsString::from("list"),
                    OsString::from("--porcelain"),
                    OsString::from("-z"),
                ],
            ),
        )?;
        if !worktrees.success() {
            return Err(anyhow!(
                "could not list Git worktrees: {}",
                worktrees.stderr_text_lossy().trim()
            ));
        }
        let primary = parse_worktrees(&worktrees.stdout)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Git did not report a primary worktree"))?;
        let project_name = primary
            .path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| anyhow!("Git primary worktree path has no final component"))?
            .to_string_lossy()
            .into_owned();
        let primary_checkout_path = primary.path.to_string_lossy().into_owned();
        let default_branch = primary.branch;

        let reported_upstream = match default_branch.as_deref() {
            Some(branch) => self.optional_stdout(
                &primary.path,
                git_args(
                    &primary.path,
                    vec![
                        OsString::from("for-each-ref"),
                        OsString::from("--format=%(upstream:short)"),
                        OsString::from(format!("refs/heads/{branch}")),
                    ],
                ),
                &[],
                "for-each-ref",
            )?,
            None => None,
        };
        let configured_remote = match default_branch.as_deref() {
            Some(branch) => self.optional_stdout(
                &primary.path,
                git_args(
                    &primary.path,
                    vec![
                        OsString::from("config"),
                        OsString::from("--get"),
                        OsString::from(format!("branch.{branch}.remote")),
                    ],
                ),
                &[1],
                "config --get branch.<default>.remote",
            )?,
            None => None,
        };
        let configured_merge = match default_branch.as_deref() {
            Some(branch) => self.optional_stdout(
                &primary.path,
                git_args(
                    &primary.path,
                    vec![
                        OsString::from("config"),
                        OsString::from("--get"),
                        OsString::from(format!("branch.{branch}.merge")),
                    ],
                ),
                &[1],
                "config --get branch.<default>.merge",
            )?,
            None => None,
        };
        let default_upstream = match (configured_remote.as_deref(), configured_merge.as_deref()) {
            (Some(remote), Some(merge_ref)) => {
                reported_upstream.or_else(|| configured_upstream(remote, merge_ref))
            }
            _ => None,
        };
        let remote_url = match (default_upstream.as_ref(), configured_remote.as_deref()) {
            (Some(_), Some(remote)) => {
                let configured_url = self.optional_stdout(
                    &primary.path,
                    git_args(
                        &primary.path,
                        vec![
                            OsString::from("config"),
                            OsString::from("--get"),
                            OsString::from(format!("remote.{remote}.url")),
                        ],
                    ),
                    &[1],
                    "config --get remote.<tracking>.url",
                )?;
                match configured_url {
                    Some(_) => self
                        .optional_stdout(
                            &primary.path,
                            git_args(
                                &primary.path,
                                vec![
                                    OsString::from("remote"),
                                    OsString::from("get-url"),
                                    OsString::from(remote),
                                ],
                            ),
                            &[2],
                            "remote get-url",
                        )?
                        .and_then(|url| sanitize_remote_url(&url)),
                    None => None,
                }
            }
            _ => None,
        };

        Ok(RepositoryRecord {
            repository_key: identity.repository_key.clone(),
            primary_checkout_path,
            project_name,
            default_branch,
            default_upstream,
            remote_url,
            updated_at_ms: now_ms,
        })
    }
}

impl<R: GitRunner> GitRepositoryResolver<R> {
    fn run_required(&self, cwd: &Path, args: Vec<OsString>) -> Result<GitCommandOutput> {
        self.runner.run(cwd, &args).map_err(anyhow::Error::from)
    }

    fn optional_stdout(
        &self,
        cwd: &Path,
        args: Vec<OsString>,
        missing_statuses: &[i32],
        command: &str,
    ) -> Result<Option<String>> {
        let output = self.run_required(cwd, args)?;
        if !output.success() {
            if matches!(output.status_code, Some(status) if missing_statuses.contains(&status)) {
                return Ok(None);
            }
            return Err(anyhow!(
                "could not resolve optional Git metadata with {command}: {}",
                output.stderr_text_lossy().trim()
            ));
        }
        let value = output
            .stdout_text()
            .context("optional Git metadata output is not UTF-8")?;
        let value = value.trim();
        Ok((!value.is_empty()).then(|| value.to_owned()))
    }
}

fn configured_upstream(remote: &str, merge_ref: &str) -> Option<String> {
    let branch = merge_ref
        .strip_prefix("refs/heads/")
        .filter(|branch| !branch.is_empty())?;
    Some(if remote == "." {
        branch.to_owned()
    } else {
        format!("{remote}/{branch}")
    })
}

fn git_args(cwd: &Path, command: Vec<OsString>) -> Vec<OsString> {
    let mut args = vec![OsString::from("-C"), cwd.as_os_str().to_os_string()];
    args.extend(command);
    args
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorktreeEntry {
    path: PathBuf,
    branch: Option<String>,
}

fn parse_worktrees(bytes: &[u8]) -> Result<Vec<WorktreeEntry>> {
    let text = std::str::from_utf8(bytes).map_err(|error| anyhow!(error))?;
    let mut entries = Vec::new();
    let mut fields = Vec::new();

    for field in text.split('\0') {
        if field.is_empty() {
            if !fields.is_empty() {
                entries.push(parse_worktree(&fields)?);
                fields.clear();
            }
        } else {
            fields.push(field);
        }
    }

    if !fields.is_empty() {
        entries.push(parse_worktree(&fields)?);
    }

    Ok(entries)
}

fn parse_worktree(fields: &[&str]) -> Result<WorktreeEntry> {
    let path = fields
        .first()
        .and_then(|field| field.strip_prefix("worktree "))
        .ok_or_else(|| anyhow!("worktree record is missing its path"))?;
    let branch = match fields
        .iter()
        .find_map(|field| field.strip_prefix("branch "))
    {
        Some(reference) => Some(
            reference
                .strip_prefix("refs/heads/")
                .filter(|branch| !branch.is_empty())
                .ok_or_else(|| anyhow!("worktree record has an invalid branch reference"))?
                .to_owned(),
        ),
        None => None,
    };

    Ok(WorktreeEntry {
        path: PathBuf::from(path),
        branch,
    })
}

pub fn sanitize_remote_url(value: &str) -> Option<String> {
    let scheme = value.split_once(':').map(|(scheme, _)| scheme);
    if !matches!(scheme, Some(scheme) if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
    {
        return Some(value.to_owned());
    }

    if !has_valid_userinfo_escapes(value) {
        return None;
    }

    let mut url = url::Url::parse(value).ok()?;
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    Some(url.into())
}

fn has_valid_userinfo_escapes(value: &str) -> bool {
    let Some(authority) = value.split_once("//").map(|(_, rest)| rest) else {
        return true;
    };
    let authority = authority.split('/').next().unwrap_or(authority);
    let Some((userinfo, _)) = authority.rsplit_once('@') else {
        return true;
    };

    let bytes = userinfo.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        collections::VecDeque,
        ffi::OsString,
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    };

    use super::{parse_worktrees, sanitize_remote_url, GitMetadataSource, GitRepositoryResolver};
    use crate::git::command::{GitCommandError, GitCommandOutput, GitRunner, ProcessGitRunner};

    #[test]
    fn parses_primary_and_linked_worktrees() {
        let bytes = b"worktree /src/project\0HEAD abc\0branch refs/heads/trunk\0\0\
                      worktree /tmp/project-feature\0HEAD def\0branch refs/heads/feature/git\0\0";
        let entries = parse_worktrees(bytes).unwrap();

        assert_eq!(entries[0].path, PathBuf::from("/src/project"));
        assert_eq!(entries[0].branch.as_deref(), Some("trunk"));
        assert_eq!(entries[1].branch.as_deref(), Some("feature/git"));
    }

    #[test]
    fn strips_https_userinfo_and_preserves_scp_ssh() {
        assert_eq!(
            sanitize_remote_url("https://user:secret@example.com/acme/project.git").as_deref(),
            Some("https://example.com/acme/project.git")
        );
        assert_eq!(
            sanitize_remote_url("git@example.com:acme/project.git").as_deref(),
            Some("git@example.com:acme/project.git")
        );
        assert_eq!(
            sanitize_remote_url("https://%zz@example.com/project.git"),
            None
        );
    }

    #[test]
    fn rejects_malformed_porcelain_branch_fields() {
        for branch in ["refs/tags/v1", "refs/heads/"] {
            let bytes = format!("worktree /src/project\0HEAD abc\0branch {branch}\0\0");
            assert!(parse_worktrees(bytes.as_bytes()).is_err());
        }
    }

    #[test]
    fn returns_an_error_for_unexpected_optional_git_failures() {
        let resolver = GitRepositoryResolver::new(ScriptedGitRunner::new(vec![
            git_output(
                Some(0),
                b"worktree /src/project\0HEAD abc\0branch refs/heads/trunk\0\0".to_vec(),
                Vec::new(),
            ),
            git_output(
                Some(128),
                Vec::new(),
                b"fatal: corrupt configuration".to_vec(),
            ),
            git_output(Some(1), Vec::new(), Vec::new()),
        ]));
        let identity = super::WorktreeIdentity {
            repository_key: "repository".to_owned(),
            branch: Some("feature/git".to_owned()),
        };

        let error = resolver
            .resolve_repository(Path::new("/src/project"), &identity, 123)
            .unwrap_err();

        assert!(error.to_string().contains("for-each-ref"));
    }

    #[test]
    fn does_not_resolve_a_remote_without_a_configured_tracking_upstream() {
        let (_fixture, primary) = initialized_repository();
        run_git(
            &primary,
            &[
                "remote",
                "add",
                "company",
                "https://example.com/acme/project.git",
            ],
        );
        run_git(&primary, &["config", "branch.trunk.remote", "company"]);

        let repository = resolve_repository(&primary);

        assert_eq!(repository.default_branch.as_deref(), Some("trunk"));
        assert_eq!(repository.default_upstream, None);
        assert_eq!(repository.remote_url, None);
        assert_eq!(repository.primary_checkout_path, primary.to_string_lossy());
        assert_eq!(repository.project_name, "primary");
    }

    #[test]
    fn keeps_the_configured_upstream_when_its_remote_does_not_exist() {
        let (_fixture, primary) = initialized_repository();
        run_git(&primary, &["config", "branch.trunk.remote", "missing"]);
        run_git(
            &primary,
            &["config", "branch.trunk.merge", "refs/heads/trunk"],
        );

        let repository = resolve_repository(&primary);

        assert_eq!(repository.default_branch.as_deref(), Some("trunk"));
        assert_eq!(
            repository.default_upstream.as_deref(),
            Some("missing/trunk")
        );
        assert_eq!(repository.remote_url, None);
        assert_eq!(repository.primary_checkout_path, primary.to_string_lossy());
        assert_eq!(repository.project_name, "primary");
    }

    #[test]
    fn does_not_treat_a_remote_name_as_its_missing_url() {
        let (_fixture, primary) = initialized_repository();
        run_git(
            &primary,
            &[
                "config",
                "remote.company.fetch",
                "+refs/heads/*:refs/remotes/company/*",
            ],
        );
        run_git(&primary, &["config", "branch.trunk.remote", "company"]);
        run_git(
            &primary,
            &["config", "branch.trunk.merge", "refs/heads/trunk"],
        );

        let repository = resolve_repository(&primary);

        assert_eq!(repository.default_branch.as_deref(), Some("trunk"));
        assert_eq!(
            repository.default_upstream.as_deref(),
            Some("company/trunk")
        );
        assert_eq!(repository.remote_url, None);
        assert_eq!(repository.primary_checkout_path, primary.to_string_lossy());
        assert_eq!(repository.project_name, "primary");
    }

    #[test]
    fn resolves_a_linked_worktree_and_its_primary_repository() {
        let fixture = tempfile::tempdir().unwrap();
        let fixture_path = git_fixture_path(&fixture);
        let primary = fixture_path.join("primary");
        let linked = fixture_path.join("linked");

        run_git(
            &fixture_path,
            &["init", "-b", "trunk", primary.to_str().unwrap()],
        );
        run_git(&primary, &["config", "user.name", "Codex Pulse Tests"]);
        run_git(&primary, &["config", "user.email", "pulse@example.invalid"]);
        std::fs::write(primary.join("README.md"), "initial\n").unwrap();
        run_git(&primary, &["add", "README.md"]);
        run_git(&primary, &["commit", "-m", "initial"]);
        run_git(
            &primary,
            &[
                "remote",
                "add",
                "company",
                "https://user:secret@example.com/acme/project.git",
            ],
        );
        run_git(
            &primary,
            &["update-ref", "refs/remotes/company/trunk", "HEAD"],
        );
        run_git(
            &primary,
            &["branch", "--set-upstream-to", "company/trunk", "trunk"],
        );
        run_git(
            &primary,
            &[
                "worktree",
                "add",
                "-b",
                "feature/git",
                linked.to_str().unwrap(),
            ],
        );

        let resolver = GitRepositoryResolver {
            runner: ProcessGitRunner::new(OsString::from("git").into(), Duration::from_secs(1)),
        };
        let identity = resolver.resolve_worktree(&linked).unwrap().unwrap();
        assert_eq!(identity.branch.as_deref(), Some("feature/git"));

        let repository = resolver
            .resolve_repository(&linked, &identity, 123)
            .unwrap();
        assert_eq!(repository.primary_checkout_path, primary.to_string_lossy());
        assert_eq!(repository.project_name, "primary");
        assert_eq!(repository.default_branch.as_deref(), Some("trunk"));
        assert_eq!(
            repository.default_upstream.as_deref(),
            Some("company/trunk")
        );
        assert_eq!(
            repository.remote_url.as_deref(),
            Some("https://example.com/acme/project.git")
        );

        run_git(&linked, &["checkout", "--detach"]);
        let detached_identity = resolver.resolve_worktree(&linked).unwrap().unwrap();
        assert_eq!(detached_identity.branch, None);

        let plain_directory = fixture_path.join("plain");
        std::fs::create_dir(&plain_directory).unwrap();
        assert!(resolver
            .resolve_worktree(&plain_directory)
            .unwrap()
            .is_none());
    }

    fn initialized_repository() -> (tempfile::TempDir, PathBuf) {
        let fixture = tempfile::tempdir().unwrap();
        let fixture_path = git_fixture_path(&fixture);
        let primary = fixture_path.join("primary");
        run_git(
            &fixture_path,
            &["init", "-b", "trunk", primary.to_str().unwrap()],
        );
        run_git(&primary, &["config", "user.name", "Codex Pulse Tests"]);
        run_git(&primary, &["config", "user.email", "pulse@example.invalid"]);
        run_git(&primary, &["commit", "--allow-empty", "-m", "initial"]);
        (fixture, primary)
    }

    fn git_fixture_path(fixture: &tempfile::TempDir) -> PathBuf {
        #[cfg(windows)]
        {
            // Git for Windows rejects the verbatim `\\?\` path returned by canonicalize.
            fixture.path().to_path_buf()
        }
        #[cfg(not(windows))]
        {
            std::fs::canonicalize(fixture.path()).unwrap()
        }
    }

    fn resolve_repository(primary: &Path) -> super::RepositoryRecord {
        let resolver = GitRepositoryResolver {
            runner: ProcessGitRunner::new(OsString::from("git").into(), Duration::from_secs(1)),
        };
        let identity = resolver.resolve_worktree(primary).unwrap().unwrap();
        resolver
            .resolve_repository(primary, &identity, 123)
            .unwrap()
    }

    fn run_git(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {} failed", args.join(" "));
    }

    struct ScriptedGitRunner {
        outputs: RefCell<VecDeque<GitCommandOutput>>,
    }

    impl ScriptedGitRunner {
        fn new(outputs: Vec<GitCommandOutput>) -> Self {
            Self {
                outputs: RefCell::new(outputs.into()),
            }
        }
    }

    impl GitRunner for ScriptedGitRunner {
        fn run(
            &self,
            _cwd: &Path,
            _args: &[OsString],
        ) -> Result<GitCommandOutput, GitCommandError> {
            Ok(self
                .outputs
                .borrow_mut()
                .pop_front()
                .expect("scripted Git output is available"))
        }
    }

    fn git_output(status_code: Option<i32>, stdout: Vec<u8>, stderr: Vec<u8>) -> GitCommandOutput {
        GitCommandOutput {
            status_code,
            stdout,
            stderr,
        }
    }
}
