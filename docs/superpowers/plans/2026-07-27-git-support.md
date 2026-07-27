# Git Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich active Codex session cards with the local Git primary checkout, current worktree branch, default branch, and sanitized tracking-remote URL, backed by a recoverable SQLite cache.

**Architecture:** Codex transcript scanning continues to produce base session snapshots. A bounded local Git CLI resolver enriches those snapshots inside the existing background reconciliation, and an independent SQLite store retains only last-known-good repository-level metadata. A focused Vue component renders the project link, branch context, and accessible floating repository card.

**Tech Stack:** Rust 1.82, Tauri 2, `std::process::Command`, `rusqlite`, `url`, Vue 3 Composition API, TypeScript, Vue I18n, Lucide Vue, Vitest 4.

## Global Constraints

- "Primary checkout" means Git's primary worktree.
- The primary worktree's current local branch is the feature's default branch.
- Resolve that branch's configured tracking upstream and its remote URL; never assume `main` or `origin`.
- Clicking the project-name link must continue to open the session's original `cwd`.
- Non-Git directories keep the existing basename link and render no Git decoration.
- Git context with no current branch means detached HEAD and displays localized "No branch".
- Missing default branch, upstream, or remote values display localized "Not configured".
- Strip HTTP(S) userinfo before a remote URL enters logs, SQLite, snapshots, or the WebView; preserve SCP-style SSH such as `git@host:path`.
- Perform no fetch, network request, hook, repository mutation, or WebView-side Git command.
- Store repository metadata in `git-cache.sqlite3`, separate from `config.json`; do not persist session IDs, raw session CWDs, or current worktree branches.
- A Git or cache failure must never remove a base Codex session or prevent application startup.
- Add no Git library dependency, toast/dialog system, dirty-state/ahead-behind/commit UI, or unrelated refactor.

---

## File Structure

- `src-tauri/src/git/command.rs`: bounded process execution and Git command output/error contract.
- `src-tauri/src/git/resolver.rs`: Git worktree/repository parsing, upstream lookup, and URL sanitization.
- `src-tauri/src/git/store.rs`: SQLite schema and repository-record persistence.
- `src-tauri/src/git/enrichment.rs`: per-reconciliation deduplication and cache fallback.
- `src-tauri/src/git/mod.rs`: focused public exports for the Git boundary.
- `src-tauri/src/model.rs`: serialized session Git context.
- `src-tauri/src/commands.rs`: `AppState` ownership and background-reconciliation wiring.
- `src-tauri/src/lib.rs`: Git module registration.
- `src/types.ts`: frontend mirror of the serialized session Git context.
- `src/components/ProjectIdentity.vue`: project link, branch label, and teleported hover card.
- `src/components/SessionCard.vue`: delegates its second line to `ProjectIdentity`.
- `src/styles.css`: project identity, branch, hover-card, light/dark, and narrow-window rules.
- `src/i18n.ts`: Git labels in all four supported locales.
- `src/__tests__/ProjectIdentity.spec.ts`: component behavior, applied styles, and placement.
- `src/__tests__/SessionCard.spec.ts`: integration with the existing session card and open-project event.
- `src/__tests__/i18n.spec.ts`: complete Git-copy locale contract.

---

### Task 1: Add the serialized session Git contract

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/registry.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src/types.ts`

**Interfaces:**
- Produces: `SessionGitContext` in Rust and TypeScript.
- Produces: `SessionSnapshot.git: Option<SessionGitContext>` / `git?: SessionGitContext`.
- Preserves: every existing session snapshot initially uses `git: None`.

- [ ] **Step 1: Write a failing Rust serialization test**

Add this test module coverage in `src-tauri/src/model.rs`:

```rust
#[test]
fn serializes_git_context_with_camel_case_fields() {
    let snapshot = SessionSnapshot {
        thread_id: "thread".into(),
        title: "Task".into(),
        cwd: "/worktrees/project".into(),
        git: Some(SessionGitContext {
            project_name: "project".into(),
            primary_checkout_path: "/src/project".into(),
            branch: Some("feature/git".into()),
            default_branch: Some("trunk".into()),
            default_upstream: Some("company/trunk".into()),
            remote_url: Some("https://example.com/company/project.git".into()),
        }),
        session_created_at_ms: 1_000,
        current_run_started_at_ms: 2_000,
        recent_event: None,
        last_user_message: None,
    };

    let value = serde_json::to_value(snapshot).unwrap();

    assert_eq!(value["git"]["projectName"], "project");
    assert_eq!(value["git"]["primaryCheckoutPath"], "/src/project");
    assert_eq!(value["git"]["defaultUpstream"], "company/trunk");
}
```

- [ ] **Step 2: Run the Rust test and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml model::tests::serializes_git_context_with_camel_case_fields
```

Expected: compile failure because `SessionGitContext` and `SessionSnapshot.git` do not exist.

- [ ] **Step 3: Add the Rust and TypeScript models**

In `src-tauri/src/model.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGitContext {
    pub project_name: String,
    pub primary_checkout_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_upstream: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}
```

Add this field after `cwd` in `SessionSnapshot`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub git: Option<SessionGitContext>,
```

Add `git: None` to the constructors in:

- `SessionRegistry::snapshot_for_root`
- the registry equality fixture
- `commands.rs` test helper `session`

In `src/types.ts`, add:

```ts
export interface SessionGitContext {
  projectName: string;
  primaryCheckoutPath: string;
  branch?: string;
  defaultBranch?: string;
  defaultUpstream?: string;
  remoteUrl?: string;
}
```

Add this field after `cwd` in `SessionSnapshot`:

```ts
git?: SessionGitContext;
```

- [ ] **Step 4: Run the focused Rust and frontend type checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml model::tests::serializes_git_context_with_camel_case_fields registry::tests::active_descendant_keeps_only_the_root_visible
pnpm build
```

Expected: both commands pass; existing TypeScript session fixtures remain valid because `git` is optional.

- [ ] **Step 5: Commit the model**

```bash
git add src-tauri/src/model.rs src-tauri/src/registry.rs src-tauri/src/commands.rs src/types.ts
git commit -m "feat: add git session context model"
```

---

### Task 2: Execute bounded local Git commands

**Files:**
- Create: `src-tauri/src/git/command.rs`
- Create: `src-tauri/src/git/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `GitRunner::run(&self, cwd: &Path, args: &[OsString]) -> Result<GitCommandOutput, GitCommandError>`.
- Produces: `ProcessGitRunner::new(executable: PathBuf, timeout: Duration)`.
- Guarantees: direct argument execution, null stdin, local Git environment, and child termination after timeout.

- [ ] **Step 1: Write failing command-runner tests**

Create `src-tauri/src/git/command.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf, time::Duration};

    use super::{GitCommandError, GitRunner, ProcessGitRunner};

    #[test]
    fn captures_a_successful_process_without_a_shell() {
        let runner = ProcessGitRunner::new(PathBuf::from("/usr/bin/printf"), Duration::from_secs(1));
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
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::command::tests
```

Expected: compile failure because the command types and Git module do not exist.

- [ ] **Step 3: Implement the runner**

Implement these contracts in `src-tauri/src/git/command.rs`:

```rust
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
    fn run(
        &self,
        cwd: &Path,
        args: &[OsString],
    ) -> Result<GitCommandOutput, GitCommandError>;
}

pub struct ProcessGitRunner {
    executable: PathBuf,
    timeout: Duration,
}
```

`ProcessGitRunner::run` must:

```rust
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
```

Poll `try_wait()` every 10 ms. On deadline, call `kill()` and `wait()` before
returning `GitCommandError::Timeout`. On completion, collect output and expose
the exit code without converting non-zero statuses into runner errors:

```rust
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
```

Create `src-tauri/src/git/mod.rs`:

```rust
pub mod command;
```

Add to `src-tauri/src/lib.rs`:

```rust
pub mod git;
```

- [ ] **Step 4: Run the command-runner tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::command::tests
cargo fmt --check --manifest-path src-tauri/Cargo.toml
```

Expected: both process tests pass and formatting is clean.

- [ ] **Step 5: Commit the runner**

```bash
git add src-tauri/src/git/command.rs src-tauri/src/git/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add bounded git command runner"
```

---

### Task 3: Resolve worktree, default branch, upstream, and sanitized remote

**Files:**
- Create: `src-tauri/src/git/resolver.rs`
- Modify: `src-tauri/src/git/mod.rs`

**Interfaces:**
- Produces: `WorktreeIdentity { repository_key: String, branch: Option<String> }`.
- Produces: `RepositoryRecord { repository_key, primary_checkout_path, project_name, default_branch, default_upstream, remote_url, updated_at_ms }`.
- Produces: `GitMetadataSource::{resolve_worktree, resolve_repository}` for the enrichment layer.
- Produces: `sanitize_remote_url(value: &str) -> Option<String>`.

- [ ] **Step 1: Write failing pure parser and sanitization tests**

Start `src-tauri/src/git/resolver.rs` with tests for the NUL-delimited
`git worktree list --porcelain -z` format:

```rust
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
    assert_eq!(sanitize_remote_url("https://%zz@example.com/project.git"), None);
}
```

- [ ] **Step 2: Run the resolver tests and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::resolver::tests
```

Expected: compile failure because the resolver module and parser do not exist.

- [ ] **Step 3: Implement the resolver contracts and parsers**

Add these core types:

```rust
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
    fn resolve_worktree(&self, cwd: &Path) -> anyhow::Result<Option<WorktreeIdentity>>;
    fn resolve_repository(
        &self,
        cwd: &Path,
        identity: &WorktreeIdentity,
        now_ms: i64,
    ) -> anyhow::Result<RepositoryRecord>;
}

pub struct GitRepositoryResolver<R> {
    runner: R,
}
```

`resolve_worktree` must run:

```text
git -C <cwd> rev-parse --path-format=absolute --git-common-dir
git -C <cwd> symbolic-ref --quiet --short HEAD
```

Rules:

- `rev-parse` stderr containing `not a git repository` returns `Ok(None)`.
- Any other failed identity command returns an error.
- Canonicalize the common Git directory before converting it to
  `repository_key`.
- `symbolic-ref` exit code 0 returns its trimmed branch.
- `symbolic-ref` exit code 1 returns `branch: None`.
- Other symbolic-ref failures return an error.

`resolve_repository` must run:

```text
git -C <cwd> worktree list --porcelain -z
git -C <primary> for-each-ref --format=%(upstream:short) refs/heads/<default>
git -C <primary> config --get branch.<default>.remote
git -C <primary> remote get-url <remote>
```

Select the first porcelain worktree as the primary worktree. Strip
`refs/heads/` from its branch. Missing optional commands or empty optional
stdout produce `None`, not a resolver failure. A missing/invalid primary
worktree record is a resolver failure.

Derive `project_name` from the primary worktree path's final component. Pass
remote output through `sanitize_remote_url` before constructing the record.

`sanitize_remote_url` must parse HTTP(S) URLs with the existing `url` crate,
clear username and password, and return `None` for malformed HTTP(S). Return
non-HTTP(S) values unchanged so SCP-style SSH remains usable.

Export the module from `src-tauri/src/git/mod.rs`:

```rust
pub mod resolver;
```

- [ ] **Step 4: Add failing real-repository tests**

In the resolver test module, add a helper that runs Git directly in a temporary
fixture and checks every command status. Build this repository:

```text
<temp>/primary            primary worktree on trunk
<temp>/linked             linked worktree on feature/git
remote company            https://user:secret@example.com/acme/project.git
trunk upstream            company/trunk
```

Create the fixture with:

```text
git init -b trunk <primary>
git -C <primary> config user.name "Codex Pulse Tests"
git -C <primary> config user.email "pulse@example.invalid"
git -C <primary> add README.md
git -C <primary> commit -m initial
git -C <primary> remote add company https://user:secret@example.com/acme/project.git
git -C <primary> update-ref refs/remotes/company/trunk HEAD
git -C <primary> branch --set-upstream-to company/trunk trunk
git -C <primary> worktree add -b feature/git <linked>
```

Add assertions:

```rust
let identity = resolver.resolve_worktree(&linked).unwrap().unwrap();
assert_eq!(identity.branch.as_deref(), Some("feature/git"));

let repository = resolver.resolve_repository(&linked, &identity, 123).unwrap();
assert_eq!(repository.primary_checkout_path, primary.to_string_lossy());
assert_eq!(repository.project_name, "primary");
assert_eq!(repository.default_branch.as_deref(), Some("trunk"));
assert_eq!(repository.default_upstream.as_deref(), Some("company/trunk"));
assert_eq!(
    repository.remote_url.as_deref(),
    Some("https://example.com/acme/project.git")
);
```

Detach the linked worktree and assert a second `resolve_worktree` returns
`branch: None`. Resolve a plain temporary directory and assert `Ok(None)`.

- [ ] **Step 5: Run all resolver tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::resolver::tests
cargo fmt --check --manifest-path src-tauri/Cargo.toml
```

Expected: parser, URL, primary-worktree, non-origin upstream, detached, and
non-Git cases pass.

- [ ] **Step 6: Commit the resolver**

```bash
git add src-tauri/src/git/resolver.rs src-tauri/src/git/mod.rs
git commit -m "feat: resolve git worktree metadata"
```

---

### Task 4: Persist last-known-good repository metadata in SQLite

**Files:**
- Create: `src-tauri/src/git/store.rs`
- Modify: `src-tauri/src/git/mod.rs`

**Interfaces:**
- Produces: `GitCacheStore::open(path: &Path) -> anyhow::Result<Self>`.
- Produces: `GitCacheStore::load(repository_key: &str) -> anyhow::Result<Option<RepositoryRecord>>`.
- Produces: `GitCacheStore::upsert(record: &RepositoryRecord) -> anyhow::Result<()>`.
- Produces: `GitCacheStore::path() -> &Path`.

- [ ] **Step 1: Write failing schema and persistence tests**

Create `src-tauri/src/git/store.rs` and add:

```rust
fn repository_record() -> RepositoryRecord {
    RepositoryRecord {
        repository_key: "common-dir".into(),
        primary_checkout_path: "/src/project".into(),
        project_name: "project".into(),
        default_branch: Some("trunk".into()),
        default_upstream: Some("company/trunk".into()),
        remote_url: Some("https://example.com/acme/project.git".into()),
        updated_at_ms: 100,
    }
}

#[test]
fn creates_version_one_schema_and_reopens_records() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("git-cache.sqlite3");
    let store = GitCacheStore::open(&path).unwrap();
    let record = repository_record();

    store.upsert(&record).unwrap();
    drop(store);

    let reopened = GitCacheStore::open(&path).unwrap();
    assert_eq!(reopened.load("common-dir").unwrap(), Some(record));
    assert_eq!(
        reopened
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn a_successful_null_value_replaces_old_optional_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let store = GitCacheStore::open(&temp.path().join("git-cache.sqlite3")).unwrap();
    let mut record = repository_record();
    store.upsert(&record).unwrap();

    record.default_upstream = None;
    record.remote_url = None;
    record.updated_at_ms = 200;
    store.upsert(&record).unwrap();

    assert_eq!(store.load("common-dir").unwrap(), Some(record));
}
```

- [ ] **Step 2: Run the store tests and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::store::tests
```

Expected: compile failure because `GitCacheStore` does not exist.

- [ ] **Step 3: Implement schema migration and atomic upsert**

Implement:

```rust
pub struct GitCacheStore {
    path: PathBuf,
    connection: rusqlite::Connection,
}
```

Expose the persisted location without exposing the connection:

```rust
pub fn path(&self) -> &Path {
    &self.path
}
```

`open` must create the parent directory, open the database, and execute:

```sql
CREATE TABLE IF NOT EXISTS repositories (
  repository_key TEXT PRIMARY KEY NOT NULL,
  primary_checkout_path TEXT NOT NULL,
  project_name TEXT NOT NULL,
  default_branch TEXT,
  default_upstream TEXT,
  remote_url TEXT,
  updated_at_ms INTEGER NOT NULL
);
PRAGMA user_version = 1;
```

Reject `PRAGMA user_version` values greater than 1 instead of guessing a
downgrade. Use one `INSERT ... ON CONFLICT(repository_key) DO UPDATE` statement
for every field:

```sql
INSERT INTO repositories (
  repository_key, primary_checkout_path, project_name, default_branch,
  default_upstream, remote_url, updated_at_ms
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(repository_key) DO UPDATE SET
  primary_checkout_path = excluded.primary_checkout_path,
  project_name = excluded.project_name,
  default_branch = excluded.default_branch,
  default_upstream = excluded.default_upstream,
  remote_url = excluded.remote_url,
  updated_at_ms = excluded.updated_at_ms;
```

Map `load` rows back into `RepositoryRecord` without normalizing or
re-sanitizing values; the resolver is the only write source.

Export the module:

```rust
pub mod store;
```

- [ ] **Step 4: Run store tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::store::tests
cargo fmt --check --manifest-path src-tauri/Cargo.toml
```

Expected: schema version, reopen, complete upsert, and NULL replacement pass.

- [ ] **Step 5: Commit the SQLite store**

```bash
git add src-tauri/src/git/store.rs src-tauri/src/git/mod.rs
git commit -m "feat: persist git repository metadata"
```

---

### Task 5: Enrich sessions with repository deduplication and cache fallback

**Files:**
- Create: `src-tauri/src/git/enrichment.rs`
- Modify: `src-tauri/src/git/mod.rs`

**Interfaces:**
- Produces: `GitSessionEnricher<S: GitMetadataSource>`.
- Produces: `enrich(&mut self, sessions: &mut [SessionSnapshot], now_ms: i64)`.
- Guarantees: one stable repository resolution per repository key per
  reconciliation and no propagation of per-repository failures.

- [ ] **Step 1: Write failing deduplication and non-Git tests**

Create this `FakeSource` and session fixture in
`src-tauri/src/git/enrichment.rs` tests:

```rust
struct FakeSource {
    repository_calls: Cell<usize>,
    fail_repository: bool,
}

impl FakeSource {
    fn repository_call_count(&self) -> usize {
        self.repository_calls.get()
    }
}

impl GitMetadataSource for FakeSource {
    fn resolve_worktree(&self, cwd: &Path) -> anyhow::Result<Option<WorktreeIdentity>> {
        let branch = match cwd.to_str().unwrap() {
            "/repo/worktree-a" => Some("feature/a"),
            "/repo/worktree-b" => Some("feature/b"),
            "/plain" => return Ok(None),
            other => anyhow::bail!("unexpected fixture path: {other}"),
        };
        Ok(Some(WorktreeIdentity {
            repository_key: "repo".into(),
            branch: branch.map(str::to_owned),
        }))
    }

    fn resolve_repository(
        &self,
        _cwd: &Path,
        identity: &WorktreeIdentity,
        now_ms: i64,
    ) -> anyhow::Result<RepositoryRecord> {
        self.repository_calls.set(self.repository_calls.get() + 1);
        if self.fail_repository {
            anyhow::bail!("stable metadata unavailable");
        }
        Ok(RepositoryRecord {
            repository_key: identity.repository_key.clone(),
            primary_checkout_path: "/src/project".into(),
            project_name: "project".into(),
            default_branch: Some("trunk".into()),
            default_upstream: Some("company/trunk".into()),
            remote_url: Some("https://example.com/acme/project.git".into()),
            updated_at_ms: now_ms,
        })
    }
}

fn session(cwd: &str) -> SessionSnapshot {
    SessionSnapshot {
        thread_id: cwd.into(),
        title: "Task".into(),
        cwd: cwd.into(),
        git: None,
        session_created_at_ms: 1_000,
        current_run_started_at_ms: 2_000,
        recent_event: None,
        last_user_message: None,
    }
}
```

Test:

```rust
let source = FakeSource {
    repository_calls: Cell::new(0),
    fail_repository: false,
};
let mut sessions = vec![
    session("/repo/worktree-a"),
    session("/repo/worktree-b"),
    session("/plain"),
];
let mut enricher = GitSessionEnricher::new(source, None);

enricher.enrich(&mut sessions, 500);

assert_eq!(enricher.source().repository_call_count(), 1);
assert_eq!(sessions[0].git.as_ref().unwrap().branch.as_deref(), Some("feature/a"));
assert_eq!(sessions[1].git.as_ref().unwrap().branch.as_deref(), Some("feature/b"));
assert!(sessions[2].git.is_none());
```

- [ ] **Step 2: Run the enrichment test and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::enrichment::tests::deduplicates_repository_metadata_and_keeps_worktree_branches
```

Expected: compile failure because `GitSessionEnricher` does not exist.

- [ ] **Step 3: Implement the enrichment loop**

Implement:

```rust
pub struct GitSessionEnricher<S> {
    source: S,
    store: Option<GitCacheStore>,
}
```

For every session:

1. Reset `session.git` to `None`.
2. Call `source.resolve_worktree(Path::new(&session.cwd))`.
3. Ignore `Ok(None)` and errors.
4. Use a `HashMap<String, Option<RepositoryRecord>>` keyed by
   `identity.repository_key`.
5. On the first key, load the cached row, then call
   `source.resolve_repository`.
6. On live success, best-effort upsert and use the live record, including
   intentionally absent optional values.
7. On live failure, use the cached row.
8. Convert a selected record plus the live `identity.branch` into:

```rust
SessionGitContext {
    project_name: record.project_name.clone(),
    primary_checkout_path: record.primary_checkout_path.clone(),
    branch: identity.branch,
    default_branch: record.default_branch.clone(),
    default_upstream: record.default_upstream.clone(),
    remote_url: record.remote_url.clone(),
}
```

Add `source(&self) -> &S` under `#[cfg(test)]` so tests can inspect call counts.

Export the module:

```rust
pub mod enrichment;
```

- [ ] **Step 4: Write a failing last-known-good test**

Prepopulate a temporary `GitCacheStore` with a complete record. Configure the
fake source so worktree identity succeeds but repository resolution returns an
error. Assert:

```rust
let temp = tempfile::tempdir().unwrap();
let cache_path = temp.path().join("git-cache.sqlite3");
let store = GitCacheStore::open(&cache_path).unwrap();
let original_record = RepositoryRecord {
    repository_key: "repo".into(),
    primary_checkout_path: "/src/project".into(),
    project_name: "project".into(),
    default_branch: Some("trunk".into()),
    default_upstream: Some("company/trunk".into()),
    remote_url: Some("https://example.com/acme/project.git".into()),
    updated_at_ms: 100,
};
store.upsert(&original_record).unwrap();
let source = FakeSource {
    repository_calls: Cell::new(0),
    fail_repository: true,
};
let mut sessions = vec![session("/repo/worktree-a")];
let mut enricher = GitSessionEnricher::new(source, Some(store));

enricher.enrich(&mut sessions, 600);
let git = sessions[0].git.as_ref().unwrap();
assert_eq!(git.project_name, "project");
assert_eq!(git.branch.as_deref(), Some("feature/a"));
assert_eq!(
    git.remote_url.as_deref(),
    Some("https://example.com/acme/project.git")
);
drop(enricher);
let store_reopened = GitCacheStore::open(&cache_path).unwrap();
assert_eq!(store_reopened.load("repo").unwrap(), Some(original_record));
```

This proves failure does not write blank values over the stored row.

- [ ] **Step 5: Run all enrichment and store tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml git::enrichment::tests git::store::tests
cargo fmt --check --manifest-path src-tauri/Cargo.toml
```

Expected: repository deduplication, distinct worktree branches, non-Git
degradation, cache fallback, and last-known-good retention pass.

- [ ] **Step 6: Commit the enrichment service**

```bash
git add src-tauri/src/git/enrichment.rs src-tauri/src/git/mod.rs
git commit -m "feat: enrich sessions with cached git context"
```

---

### Task 6: Wire Git enrichment into background reconciliation

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: `GitSessionEnricher<GitRepositoryResolver<ProcessGitRunner>>`.
- Produces: enriched `CachedSnapshot.sessions`.
- Persists: `<config-directory>/git-cache.sqlite3`.

- [ ] **Step 1: Write a failing AppState persistence-location test**

Inside `commands.rs` tests, add:

```rust
#[test]
fn app_state_places_git_cache_beside_user_config() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("settings/config.json");
    let _state = AppState::new(
        temp.path().join("codex-home"),
        ConfigStore::new(config_path.clone()),
    )
    .unwrap();

    assert!(config_path.with_file_name("git-cache.sqlite3").is_file());
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands::tests::app_state_places_git_cache_beside_user_config
```

Expected: assertion failure because `AppState` does not create the Git cache.

- [ ] **Step 3: Add production AppState ownership**

Add imports for the command runner, resolver, store, and enricher. Add:

```rust
git_enricher: Mutex<
    GitSessionEnricher<GitRepositoryResolver<ProcessGitRunner>>,
>,
```

Refactor construction through a private `AppState::with_config` helper so both
`new` and `from_environment` build identical Git state. Construct:

```rust
let cache_path = store.path().with_file_name("git-cache.sqlite3");
let cache = GitCacheStore::open(&cache_path).ok();
let runner = ProcessGitRunner::new(PathBuf::from("git"), Duration::from_secs(2));
let resolver = GitRepositoryResolver::new(runner);
let git_enricher = Mutex::new(GitSessionEnricher::new(resolver, cache));
```

Cache-open failure must result in an enricher without a store, not an
`AppState` error.

- [ ] **Step 4: Wire enrichment after Codex scanning**

In the `spawn_blocking` reconciliation, after
`scan_active_sessions_with_cache` and before returning `scan`, add:

```rust
if let Ok(mut enricher) = state.git_enricher.lock() {
    enricher.enrich(&mut scan.sessions, now_ms);
}
```

Do not add Git work to `get_snapshot`; it must continue to clone the already
enriched cached snapshot only.

- [ ] **Step 5: Run backend integration gates**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands::tests::app_state_places_git_cache_beside_user_config
cargo test --manifest-path src-tauri/Cargo.toml git::
cargo fmt --check --manifest-path src-tauri/Cargo.toml
```

Expected: cache location and all Git boundary tests pass; `get_snapshot` stays
free of process execution.

- [ ] **Step 6: Commit reconciliation wiring**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: enrich reconciled sessions with git"
```

---

### Task 7: Render project identity, branch, and repository hover card

**Files:**
- Create: `src/components/ProjectIdentity.vue`
- Create: `src/__tests__/ProjectIdentity.spec.ts`
- Modify: `src/components/SessionCard.vue`
- Modify: `src/__tests__/SessionCard.spec.ts`
- Modify: `src/styles.css`
- Modify: `src/i18n.ts`
- Modify: `src/__tests__/i18n.spec.ts`

**Interfaces:**
- Consumes: `cwd: string` and `git?: SessionGitContext`.
- Produces: `open-project(path: string)`.
- Displays: project name, optional branch, and a teleported `role="tooltip"`
  repository card.

- [ ] **Step 1: Write failing project identity behavior tests**

Create `src/__tests__/ProjectIdentity.spec.ts`, import the real stylesheet, and
add cleanup after each test:

```ts
import "../styles.css";

const originalInnerHeight = window.innerHeight;

afterEach(() => {
  document.body.innerHTML = "";
  i18n.global.locale.value = "en";
  Object.defineProperty(window, "innerHeight", {
    configurable: true,
    value: originalInnerHeight
  });
  vi.restoreAllMocks();
});
```

Add a Git fixture:

```ts
const git = {
  projectName: "codex-pulse",
  primaryCheckoutPath: "/src/codex-pulse",
  branch: "feature/git-context",
  defaultBranch: "trunk",
  defaultUpstream: "company/trunk",
  remoteUrl: "https://example.com/acme/codex-pulse.git"
};
```

Test the required behavior:

```ts
it("renders git project and branch while opening the original cwd", async () => {
  const wrapper = mount(ProjectIdentity, {
    attachTo: document.body,
    props: { cwd: "/worktrees/9b55/codex-pulse", git },
    global: { plugins: [i18n] }
  });

  const link = wrapper.get("a.session-card__path");
  expect(link.text()).toBe("codex-pulse");
  expect(link.attributes("title")).toBeUndefined();
  expect(wrapper.get(".session-card__branch").text()).toContain("feature/git-context");

  await link.trigger("click");
  expect(wrapper.emitted("open-project")).toEqual([["/worktrees/9b55/codex-pulse"]]);
});

it("shows repository metadata on hover and keyboard focus", async () => {
  const wrapper = mount(ProjectIdentity, {
    attachTo: document.body,
    props: { cwd: "/worktrees/project", git },
    global: { plugins: [i18n] }
  });
  const link = wrapper.get("a.session-card__path");

  await link.trigger("mouseenter");
  await nextTick();
  let popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
  expect(popup.textContent).toContain("Default branch");
  expect(popup.textContent).toContain("trunk");
  expect(popup.textContent).toContain("https://example.com/acme/codex-pulse.git");

  await link.trigger("mouseleave");
  await link.trigger("focus");
  await nextTick();
  popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
  expect(popup).not.toBeNull();
});
```

Add the detached, missing-metadata, and non-Git cases:

```ts
it("distinguishes detached HEAD from a non-Git directory", async () => {
  const detached = mount(ProjectIdentity, {
    attachTo: document.body,
    props: { cwd: "/worktrees/project", git: { ...git, branch: undefined } },
    global: { plugins: [i18n] }
  });
  expect(detached.get(".session-card__branch").text()).toContain("No branch");
  detached.unmount();

  const plain = mount(ProjectIdentity, {
    attachTo: document.body,
    props: { cwd: "/tmp/plain-directory" },
    global: { plugins: [i18n] }
  });
  expect(plain.get(".session-card__path").text()).toBe("plain-directory");
  expect(plain.find(".session-card__branch").exists()).toBe(false);
  await plain.get(".session-card__path").trigger("mouseenter");
  expect(document.body.querySelector('[role="tooltip"]')).toBeNull();
});

it("labels unavailable repository fields as not configured", async () => {
  const wrapper = mount(ProjectIdentity, {
    attachTo: document.body,
    props: {
      cwd: "/worktrees/project",
      git: { ...git, defaultBranch: undefined, remoteUrl: undefined }
    },
    global: { plugins: [i18n] }
  });

  await wrapper.get(".session-card__path").trigger("mouseenter");
  await nextTick();
  const popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
  expect(popup.textContent?.match(/Not configured/g)).toHaveLength(2);
});
```

- [ ] **Step 2: Run the component test and verify RED**

Run:

```bash
pnpm test -- src/__tests__/ProjectIdentity.spec.ts
```

Expected: failure because `ProjectIdentity.vue` does not exist.

- [ ] **Step 3: Implement `ProjectIdentity.vue`**

Use `<script setup lang="ts">` with:

```ts
import { GitBranch } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { projectName } from "../lib/projectName";
import type { SessionGitContext } from "../types";

const props = defineProps<{ cwd: string; git?: SessionGitContext }>();
defineEmits<{ "open-project": [path: string] }>();
const { t } = useI18n();
const displayedProjectName = computed(
  () => props.git?.projectName || projectName(props.cwd)
);
const displayedBranch = computed(
  () => props.git?.branch || t("session.noBranch")
);
```

The template must:

```vue
<span class="session-card__project">
  <a
    ref="anchor"
    class="session-card__path"
    href="#"
    :aria-describedby="git ? popupId : undefined"
    @click.prevent="$emit('open-project', cwd)"
    @mouseenter="showPopup"
    @mouseleave="hidePopup"
    @focus="showPopup"
    @blur="hidePopup"
  >{{ displayedProjectName }}</a>
  <span v-if="git" class="session-card__branch">
    <GitBranch aria-hidden="true" />
    <span>{{ displayedBranch }}</span>
  </span>
</span>
<Teleport to="body">
  <aside
    v-if="git && popupOpen"
    :id="popupId"
    ref="popup"
    class="project-hover-card"
    role="tooltip"
    :data-placement="placement"
    :style="popupStyle"
  >
    <strong>{{ displayedProjectName }}</strong>
    <dl>
      <div>
        <dt>{{ t("session.defaultBranch") }}</dt>
        <dd>{{ git.defaultBranch || t("session.notConfigured") }}</dd>
      </div>
      <div>
        <dt>{{ t("session.remoteRepository") }}</dt>
        <dd>{{ git.remoteUrl || t("session.notConfigured") }}</dd>
      </div>
    </dl>
  </aside>
</Teleport>
```

Use `useId()` for `popupId`. `showPopup` sets the boolean and calls
`positionPopup` after `nextTick`. Implement the positioning state and lifecycle
with:

```ts
const anchor = ref<HTMLElement>();
const popup = ref<HTMLElement>();
const popupOpen = ref(false);
const popupId = `project-${useId()}`;
const placement = ref<"above" | "below">("below");
const popupStyle = ref<Record<string, string>>({});

function positionPopup() {
  if (!popupOpen.value || !anchor.value) return;
  const rect = anchor.value.getBoundingClientRect();
  const padding = 12;
  const gap = 8;
  const width = Math.max(0, Math.min(280, window.innerWidth - padding * 2));
  const height = popup.value?.offsetHeight || 112;
  const fitsBelow = rect.bottom + gap + height <= window.innerHeight - padding;
  placement.value = fitsBelow ? "below" : "above";
  const top = fitsBelow
    ? rect.bottom + gap
    : Math.max(padding, rect.top - gap - height);
  const left = Math.min(
    Math.max(padding, rect.left),
    Math.max(padding, window.innerWidth - padding - width)
  );
  popupStyle.value = {
    top: `${Math.round(top)}px`,
    left: `${Math.round(left)}px`,
    width: `${Math.round(width)}px`
  };
}

async function showPopup() {
  if (!props.git) return;
  popupOpen.value = true;
  await nextTick();
  positionPopup();
}

function hidePopup() {
  popupOpen.value = false;
}

onMounted(() => {
  window.addEventListener("resize", positionPopup);
  window.addEventListener("scroll", positionPopup, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", positionPopup);
  window.removeEventListener("scroll", positionPopup, true);
});
```

The tooltip has no controls and `pointer-events: none`.

- [ ] **Step 4: Write and pass placement tests**

Mock the anchor rectangle near the window bottom:

```ts
vi.spyOn(link.element, "getBoundingClientRect").mockReturnValue({
  x: 16, y: 180, top: 180, right: 140, bottom: 200, left: 16,
  width: 124, height: 20, toJSON: () => ({})
});
Object.defineProperty(window, "innerHeight", { configurable: true, value: 240 });
```

After hover and `nextTick`, assert:

```ts
expect(document.body.querySelector('[role="tooltip"]')?.getAttribute("data-placement"))
  .toBe("above");
```

Run:

```bash
pnpm test -- src/__tests__/ProjectIdentity.spec.ts
```

Expected: Git, detached, missing metadata, non-Git, hover, focus, and placement
tests pass.

- [ ] **Step 5: Integrate the component into `SessionCard`**

Replace the existing project-name helper import/computed state and `<a>` with:

```ts
import ProjectIdentity from "./ProjectIdentity.vue";
```

```vue
<ProjectIdentity
  :cwd="session.cwd"
  :git="session.git"
  @open-project="$emit('open-project', $event)"
/>
```

Update `SessionCard.spec.ts` so the first test asserts:

```ts
expect(projectLink.text()).toBe("project");
expect(projectLink.attributes("title")).toBeUndefined();
```

Add this SessionCard integration test:

```ts
it("passes git context to the project identity without changing the open path", async () => {
  const wrapper = mount(SessionCard, {
    props: {
      session: {
        threadId: "00000000-0000-4000-8000-000000000001",
        title: "Git task",
        cwd: "/worktrees/feature/project",
        git: {
          projectName: "project",
          primaryCheckoutPath: "/src/project",
          branch: "feature/git",
          defaultBranch: "trunk",
          defaultUpstream: "company/trunk",
          remoteUrl: "https://example.com/acme/project.git"
        },
        sessionCreatedAtMs: 1_000,
        currentRunStartedAtMs: 2_000
      },
      nowMs: 3_000
    },
    global: { plugins: [i18n] }
  });

  expect(wrapper.get(".session-card__branch").text()).toContain("feature/git");
  await wrapper.get(".session-card__path").trigger("click");
  expect(wrapper.emitted("open-project")).toEqual([["/worktrees/feature/project"]]);
});
```

- [ ] **Step 6: Add localized copy and its contract test**

Add these `session` keys:

```text
en:    noBranch "No branch", defaultBranch "Default branch",
       remoteRepository "Remote repository", notConfigured "Not configured"
zh-CN: noBranch "无分支", defaultBranch "默认分支",
       remoteRepository "远程仓库", notConfigured "未配置"
fr:    noBranch "Aucune branche", defaultBranch "Branche par défaut",
       remoteRepository "Dépôt distant", notConfigured "Non configuré"
de:    noBranch "Kein Branch", defaultBranch "Standardbranch",
       remoteRepository "Remote-Repository", notConfigured "Nicht konfiguriert"
```

In `i18n.spec.ts`, add:

```ts
it.each([
  ["en", ["No branch", "Default branch", "Remote repository", "Not configured"]],
  ["zh-CN", ["无分支", "默认分支", "远程仓库", "未配置"]],
  ["fr", ["Aucune branche", "Branche par défaut", "Dépôt distant", "Non configuré"]],
  ["de", ["Kein Branch", "Standardbranch", "Remote-Repository", "Nicht konfiguriert"]]
] as const)("defines complete Git copy for %s", (locale, expected) => {
  i18n.global.locale.value = locale;
  const actual = [
    i18n.global.t("session.noBranch"),
    i18n.global.t("session.defaultBranch"),
    i18n.global.t("session.remoteRepository"),
    i18n.global.t("session.notConfigured")
  ];
  i18n.global.locale.value = "en";
  expect(actual).toEqual(expected);
});
```

- [ ] **Step 7: Add the project identity and hover-card styles**

Replace the single block-only path layout with:

```css
.session-card__project { position: relative; display: flex; min-width: 0; align-items: center; gap: 7px; margin-top: 5px; }
.session-card__path { min-width: 0; overflow: hidden; color: #2467cc; font-size: 12px; text-decoration-line: underline; text-decoration-thickness: 1px; text-underline-offset: 2px; text-overflow: ellipsis; white-space: nowrap; }
.session-card__branch { display: inline-flex; min-width: 0; max-width: 48%; align-items: center; gap: 3px; color: #7a8598; font-size: 11px; white-space: nowrap; }
.session-card__branch svg { width: 12px; height: 12px; flex: 0 0 12px; stroke-width: 1.7; }
.session-card__branch span { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
.project-hover-card { position: fixed; z-index: 20; width: min(280px, calc(100vw - 24px)); padding: 10px; border: 1px solid rgba(32, 55, 91, 0.16); border-radius: 10px; color: #39465a; background: rgba(248, 250, 255, 0.94); box-shadow: 0 10px 28px rgba(31, 48, 78, 0.2); -webkit-backdrop-filter: blur(20px) saturate(1.3); backdrop-filter: blur(20px) saturate(1.3); pointer-events: none; }
.project-hover-card strong { display: block; overflow: hidden; color: #1f2b3d; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.project-hover-card dl { display: grid; gap: 7px; margin: 8px 0 0; }
.project-hover-card dl div { display: grid; grid-template-columns: 92px minmax(0, 1fr); gap: 7px; }
.project-hover-card dt { color: #7a8598; font-size: 10px; }
.project-hover-card dd { min-width: 0; margin: 0; overflow-wrap: anywhere; color: #46546a; font-size: 10px; }
```

Add exact dark rules while keeping the existing project-link colors and focus
ring:

```css
:root[data-theme="dark"] .session-card__branch { color: #98a7bc; }
:root[data-theme="dark"] .project-hover-card { border-color: rgba(218, 230, 255, 0.18); color: #c2ccdc; background: rgba(19, 30, 49, 0.96); box-shadow: 0 12px 30px rgba(2, 7, 17, 0.38); }
:root[data-theme="dark"] .project-hover-card strong { color: #edf3ff; }
:root[data-theme="dark"] .project-hover-card dt { color: #98a7bc; }
:root[data-theme="dark"] .project-hover-card dd { color: #c2ccdc; }
```

Add an applied-style test to `ProjectIdentity.spec.ts`:

```ts
it("applies compact identity layout and an unclipped fixed hover card", async () => {
  const wrapper = mount(ProjectIdentity, {
    attachTo: document.body,
    props: { cwd: "/worktrees/project", git },
    global: { plugins: [i18n] }
  });

  const project = getComputedStyle(wrapper.get(".session-card__project").element);
  const branch = getComputedStyle(wrapper.get(".session-card__branch").element);
  expect(project.display).toBe("flex");
  expect(project.minWidth).toBe("0px");
  expect(branch.maxWidth).toBe("48%");

  await wrapper.get(".session-card__path").trigger("mouseenter");
  await nextTick();
  const popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
  const popupStyle = getComputedStyle(popup);
  expect(popupStyle.position).toBe("fixed");
  expect(popupStyle.zIndex).toBe("20");
  expect(popupStyle.pointerEvents).toBe("none");
});
```

- [ ] **Step 8: Run focused frontend tests and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/ProjectIdentity.spec.ts src/__tests__/SessionCard.spec.ts src/__tests__/i18n.spec.ts
pnpm build
```

Expected: all component, localization, CSS-contract, and Vue type checks pass.

- [ ] **Step 9: Commit the session-card UI**

```bash
git add src/components/ProjectIdentity.vue src/components/SessionCard.vue src/__tests__/ProjectIdentity.spec.ts src/__tests__/SessionCard.spec.ts src/styles.css src/i18n.ts src/__tests__/i18n.spec.ts
git commit -m "feat: show git context on session cards"
```

---

## Final Verification

- [ ] **Step 1: Run the complete automated gates**

```bash
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: every command passes with no warnings or formatting errors.

- [ ] **Step 2: Verify commit and worktree scope**

```bash
git status --short --branch
git log --oneline --decorate -8
git diff d4049bb704299bcaa0187fd127aaea651759f208..HEAD --stat
```

Expected: only the design, plan, Git backend, session model, and project
identity UI are changed; generated directories remain untouched.

- [ ] **Step 3: Perform desktop visual verification**

Run:

```bash
pnpm tauri dev
```

Using active sessions rooted in:

1. a normal branch checkout;
2. a linked worktree;
3. detached HEAD;
4. a non-Git directory;

verify:

- project names come from the primary checkout for Git sessions;
- clicking opens the exact session cwd;
- the native full-path title tooltip is absent;
- branch labels are subdued and truncate safely;
- detached shows localized "No branch";
- non-Git shows neither branch nor hover card;
- hover and keyboard focus show the repository card;
- a bottom-list card flips the hover card above;
- light and dark surfaces remain legible.

Record manual visual evidence separately from the automated gate result. If the
desktop environment cannot exercise one fixture, report that path as
unverified rather than treating automated coverage as visual acceptance.
