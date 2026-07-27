# Windows Native MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Codex Pulse `0.3.0` as a native Windows 11 x64 application with the existing macOS behavior preserved, an unsigned per-user NSIS installer in a Draft GitHub Release, and explicit separation between hosted-runner evidence and pending interactive Windows acceptance.

**Architecture:** Keep the shared session model and frontend unchanged. Isolate filesystem identity, hook transport, and native application setup behind compile-time Rust modules. Unix retains its local socket, Windows uses a user-scoped named pipe, and both feed the existing refresh scheduler and 60-second fallback. Windows build, install, hook-helper, and uninstall behavior is proven on `windows-latest`; desktop interaction stays `pending-user-eyeball`.

**Tech Stack:** Vue 3, TypeScript, Vitest, Rust 1.82+, Tauri 2, Tokio named pipes, `same-file`, NSIS, PowerShell, GitHub Actions, `tauri-apps/tauri-action@v1`, GitHub CLI.

**Design source:** `docs/superpowers/specs/2026-07-27-windows-native-mvp-design.md`

## Global Constraints

- Support only Windows 11 x64 native Codex/PowerShell in this MVP. Do not add WSL, Windows ARM64, Windows 10, MSI, Store, portable, updater, launch-at-login, Mica/Acrylic, or Windows signing work.
- Preserve the current macOS ARM64 DMG path, Unix hook socket behavior, tray template icon, and all existing session semantics.
- Follow red-green-refactor within every implementation task: add the focused failing assertion, run it and record the expected failure, implement the minimum behavior, rerun the focused test, then run the affected regression suite.
- Never use real local Codex transcripts, credentials, signing material, or user paths in fixtures, logs, screenshots, or workflow artifacts.
- Keep `Frontend` and `Rust` check names unchanged. Add `Rust (Windows)` only after its workflow contract test is RED.
- Do not modify branch protection, push a release tag, publish a release, or merge a pull request without a separate AskHuman approval at the task that authorizes that remote action.
- A hosted Windows runner is authoritative for Windows compilation and automated packaging, but it does not close any item listed as `pending-user-eyeball`.
- Start implementation from the committed design at `6d9fd8d`. Because the current checkout is detached, use `superpowers:using-git-worktrees` to confirm this managed worktree is safe, then create branch `codex/windows-native-mvp` at the current commit. Do not push directly to `main`.

## Execution Amendment

The implementation preflight found that the original plan's new
`windowsPlatform.spec.ts` assertions would test source text and file layout
rather than consumer-visible behavior. The user approved this amendment before
implementation:

- do not create `windowsPlatform.spec.ts`;
- prove Rust platform boundaries with real unit/integration behavior and
  `cargo check --target x86_64-pc-windows-msvc --tests`;
- prove the Windows GUI subsystem from the built PE header in the package
  verification script;
- prove Tauri Windows configuration and icons through Tauri schema/build and
  the generated NSIS artifact;
- treat generated icons, declarative Tauri/GitHub configuration, and human
  documentation as explicit TDD exceptions; and
- retain and update the repository's existing parsed
  `githubWorkflows.spec.ts` and `githubCommunity.spec.ts` governance tests
  rather than removing established gates.

---

### Task 1: Introduce one cross-platform file identity

**Files:**

- Create: `src-tauri/src/file_identity.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/codex/discovery.rs`
- Modify: `src-tauri/src/monitor.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Test: `src-tauri/src/file_identity.rs`
- Test: existing tests in `src-tauri/src/codex/discovery.rs`
- Test: existing tests in `src-tauri/src/monitor.rs`

- [ ] **Step 1: Create the feature branch and prove the baseline**

Run:

```bash
git status --short
git switch -c codex/windows-native-mvp
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: the worktree is clean before the switch; 67 frontend tests and 52
Rust tests pass; the frontend build succeeds. If upstream counts have changed,
record the new exact counts instead of copying these historical counts.

- [ ] **Step 2: Add a failing `FileIdentity` test module**

Add `pub mod file_identity;` to `src-tauri/src/lib.rs`. Create
`src-tauri/src/file_identity.rs` with tests that call the not-yet-implemented
API:

```rust
#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::FileIdentity;

    #[test]
    fn identity_survives_append_and_changes_after_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("session.jsonl");
        std::fs::write(&path, "first\n").unwrap();
        let initial_metadata = std::fs::metadata(&path).unwrap();
        let initial = FileIdentity::from_path(&path, &initial_metadata).unwrap();

        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"second\n")
            .unwrap();
        let appended_metadata = std::fs::metadata(&path).unwrap();
        let appended = FileIdentity::from_path(&path, &appended_metadata).unwrap();
        assert_eq!(initial, appended);

        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, "replacement\n").unwrap();
        let replacement_metadata = std::fs::metadata(&path).unwrap();
        let replacement =
            FileIdentity::from_path(&path, &replacement_metadata).unwrap();
        assert_ne!(initial, replacement);
    }
}
```

- [ ] **Step 3: Run the focused test and record RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml file_identity
```

Expected: compilation fails because `FileIdentity` and `from_path` do not yet
exist.

- [ ] **Step 4: Implement the platform-specific identity**

Add `same-file = "1"` as a direct dependency. Implement a non-cloneable
identity:

```rust
use std::{fs, io, path::Path};

#[derive(Debug, PartialEq, Eq)]
pub struct FileIdentity(PlatformIdentity);

#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
struct PlatformIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Debug, PartialEq, Eq)]
struct PlatformIdentity(same_file::Handle);

impl FileIdentity {
    pub fn from_path(path: &Path, metadata: &fs::Metadata) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let _ = path;
            Ok(Self(PlatformIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            }))
        }
        #[cfg(windows)]
        {
            let _ = metadata;
            same_file::Handle::from_path(path)
                .map(PlatformIdentity)
                .map(Self)
        }
    }
}
```

Do not add a blanket fallback target. Desktop builds outside `unix` and
`windows` should fail explicitly instead of silently receiving path-only
identity.

- [ ] **Step 5: Replace both duplicate identities**

In `codex/discovery.rs`, remove the Unix metadata import and local
`FileIdentity`; import `crate::file_identity::FileIdentity` and construct it
with:

```rust
let identity = FileIdentity::from_path(path, &metadata)?;
```

In `monitor.rs`, remove `QuotaFileIdentity` and the Unix metadata import. Make
`CachedQuotaSource.identity` a `FileIdentity` and use the same constructor.

- [ ] **Step 6: Confirm GREEN on the focused and regression suites**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml file_identity
cargo test --manifest-path src-tauri/Cargo.toml codex::discovery
cargo test --manifest-path src-tauri/Cargo.toml monitor
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all commands succeed. The existing incremental append, truncation,
and quota-cache tests remain unchanged and green.

- [ ] **Step 7: Commit the identity boundary**

```bash
git add src-tauri/src/file_identity.rs src-tauri/src/lib.rs src-tauri/src/codex/discovery.rs src-tauri/src/monitor.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "fix: support Windows file identities"
```

---

### Task 2: Split hook transport and add the Windows named pipe

**Files:**

- Delete: `src-tauri/src/hook.rs`
- Create: `src-tauri/src/hook/mod.rs`
- Create: `src-tauri/src/hook/unix.rs`
- Create: `src-tauri/src/hook/windows.rs`
- Create: `src-tauri/tests/windows_hook_cli.rs`
- Modify: `src-tauri/src/hook_config.rs`
- Test: `src-tauri/src/hook/unix.rs`
- Test: `src-tauri/src/hook/windows.rs`
- Test: `src-tauri/tests/windows_hook_cli.rs`
- Test: `src-tauri/src/hook_config.rs`

#### Task 2 Review Amendment

The user approved all three Important task-review findings:

- preserve the pre-existing Unix public API with
  `#[cfg(unix)] pub use unix::socket_path;`;
- strengthen the rotation test with an observable factory/drop-order probe
  that fails if the connected server is released before its replacement is
  created; and
- derive endpoints through a pure `endpoint_name_for(scope)` boundary, assert
  its deterministic prefix/hash independently, and let the debug-only CLI
  integration test inject a unique synthetic endpoint through
  `CODEX_PULSE_TEST_HOOK_ENDPOINT`. Release builds must ignore the test
  override.

- [ ] **Step 1: Add the Windows command-quoting regression first**

Extend `hook_config.rs` tests with:

```rust
#[test]
fn preserves_a_quoted_windows_executable_and_remains_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let command =
        r#""C:\Users\Pulse Tester\AppData\Local\Codex Pulse\CodexPulse.exe" __hook"#;

    install(temp.path(), command).unwrap();
    install(temp.path(), command).unwrap();

    let document: serde_json::Value = serde_json::from_slice(
        &std::fs::read(temp.path().join("hooks.json")).unwrap(),
    )
    .unwrap();
    for event in EVENTS {
        let groups = document["hooks"][event].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["hooks"][0]["command"], command);
    }
}
```

Expose `EVENTS` to the test through:

```rust
use super::{install, is_installed, EVENTS, STATUS_MESSAGE};
```

- [ ] **Step 2: Add Windows transport tests before the module exists**

Create `src-tauri/tests/windows_hook_cli.rs`:

```rust
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
```

In `windows.rs`, the unit test must bind a unique endpoint containing a UUID,
call `notify_at` twice, and prove `accept` can rotate to a new server instance
before the second connection.

- [ ] **Step 3: Run the Windows target RED before the split**

Run:

```bash
rustup target add x86_64-pc-windows-msvc
cargo test --manifest-path src-tauri/Cargo.toml hook_config
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --tests
```

Expected: the hook-config test passes because the existing merge is already
correct; the Windows target check fails on the unconditional Unix socket
imports in `hook.rs`. Record those exact compiler errors. Errors from the
still-unfixed macOS activation policy are an expected Task 3 RED, but do not
replace the required hook errors.

- [ ] **Step 4: Create the shared module**

Move the shared event name and wrappers into `hook/mod.rs`:

```rust
use tauri::AppHandle;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

pub const SESSIONS_CHANGED_EVENT: &str = "sessions-changed";

pub fn notify_running_instance() {
    platform::notify_running_instance();
}

pub fn start_listener(app: AppHandle) -> anyhow::Result<()> {
    platform::start_listener(app)
}

#[cfg(unix)]
pub use unix::socket_path;

#[cfg(windows)]
#[doc(hidden)]
pub fn windows_endpoint_name() -> String {
    windows::endpoint_name()
}
```

- [ ] **Step 5: Move the current Unix behavior without changing semantics**

`hook/unix.rs` must retain:

- `dirs::data_local_dir().or_else(dirs::data_dir)` and `events.sock`;
- stale-socket deletion before bind;
- one named listener thread;
- nonblocking `accept` with a 100 ms wait; and
- best-effort `UnixStream::connect` from the hook helper.

Keep the existing stale-socket unit test in this file. The only visible change
should be the module boundary.

- [ ] **Step 6: Implement the Windows endpoint and listener**

Use a deterministic `DefaultHasher` over the resolved local application-data
path:

```rust
pub(super) fn endpoint_name() -> String {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let scope = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(std::env::temp_dir);
    endpoint_name_for(&scope)
}

fn endpoint_name_for(scope: &std::path::Path) -> String {
    let mut hasher = DefaultHasher::new();
    scope.hash(&mut hasher);
    format!(
        r"\\.\pipe\com.codexpulse.desktop.{:016x}.events",
        hasher.finish()
    )
}
```

The debug build may read `CODEX_PULSE_TEST_HOOK_ENDPOINT` before falling back
to `endpoint_name()`; compile that branch only under `debug_assertions`.
Production release builds must always use the derived per-user endpoint. The
integration test sets this variable to a UUID-bearing synthetic pipe and
binds that exact pipe before starting `CodexPulse.exe __hook`.

Implement a private listener whose first instance is bound synchronously and
whose `accept` method creates the replacement server before returning:

```rust
struct PipeListener {
    endpoint: String,
    server: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl PipeListener {
    fn bind_at(endpoint: String) -> std::io::Result<Self> {
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&endpoint)?;
        Ok(Self { endpoint, server })
    }

    async fn accept(&mut self) -> std::io::Result<()> {
        self.server.connect().await?;
        let next = ServerOptions::new().create(&self.endpoint)?;
        let connected = std::mem::replace(&mut self.server, next);
        drop(connected);
        Ok(())
    }
}
```

`start_listener` binds before spawning. The async loop awaits `accept`, calls
`crate::commands::schedule_refresh(app.clone())`, and ends on an accept error.
`notify_running_instance` calls a synchronous `notify_at` using
`std::fs::OpenOptions::new().read(true).write(true).open(endpoint)` and ignores
all errors. Do not add payload serialization, retries, sleeps, TCP, or an ACL
relaxation.

- [ ] **Step 7: Run the available GREEN checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml hook
cargo test --manifest-path src-tauri/Cargo.toml hook_config
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --tests
```

Expected on macOS: Unix hook and hook-config tests pass and the full Rust suite
remains green. The Windows target check must no longer report any Hook
transport error; it may remain RED only at the Task 3 activation-policy
boundary. The named-pipe tests type-check here but remain
`pending-windows-ci` for execution.

- [ ] **Step 8: Commit the transport split**

```bash
git add src-tauri/src/hook.rs src-tauri/src/hook src-tauri/src/hook_config.rs src-tauri/tests/windows_hook_cli.rs
git commit -m "fix: add Windows hook transport"
```

---

### Task 3: Make startup platform-safe and disclose listener degradation

**Files:**

- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/app.rs`
- Test: `src-tauri/src/commands.rs`

#### Task 3 Review Amendment

Tauri 2.11.5 defines `App::set_activation_policy` on `&mut self`, and the
`setup` callback supplies `&mut App`. The user approved correcting both
platform helper signatures below from `&tauri::App` to `&mut tauri::App`;
this is a type correction only and does not change the approved behavior.

- [ ] **Step 1: Add RED tests for runtime degradation**

Add methods to the test call sites before implementing them:

```rust
#[test]
fn listener_failure_is_exposed_without_disabling_fallback_monitoring() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::new(
        temp.path().to_owned(),
        ConfigStore::new(temp.path().join("config.json")),
    )
    .unwrap();

    state.set_monitoring_degraded_reason(
        "Live hook listener unavailable: pipe is busy".into(),
    );
    let snapshot = super::snapshot_for_state(&state).unwrap();
    assert_eq!(
        snapshot.monitoring.degraded_reason.as_deref(),
        Some("Live hook listener unavailable: pipe is busy")
    );
    assert_eq!(FALLBACK_RECONCILIATION_SECONDS, 60);
}
```

Extract the body of `get_snapshot` into
`fn snapshot_for_state(state: &AppState) -> Result<AppSnapshot, String>` so
the command remains a thin `State` adapter and the test does not require a
mock Tauri runtime.

- [ ] **Step 2: Confirm the Windows startup RED**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --tests
```

Expected: compilation fails at the unguarded macOS activation-policy call.
The Task 2 Hook errors must already be absent.

- [ ] **Step 3: Add the Windows GUI subsystem attribute**

The first line of `src-tauri/src/main.rs` must be:

```rust
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
```

Leave debug builds on the console subsystem.

- [ ] **Step 4: Guard macOS activation policy**

Add:

```rust
#[cfg(target_os = "macos")]
fn configure_activation_policy(app: &mut tauri::App) {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

#[cfg(not(target_os = "macos"))]
fn configure_activation_policy(_app: &mut tauri::App) {}
```

Replace the unconditional call in `setup` with
`configure_activation_policy(app)`. Keep the Liquid Glass plugin registered;
do not add a Windows material plugin.

- [ ] **Step 5: Store and expose a listener degradation reason**

Add `monitoring_degraded_reason: RwLock<Option<String>>` to both `AppState`
constructors and these methods:

```rust
pub fn set_monitoring_degraded_reason(&self, reason: String) {
    if let Ok(mut current) = self.monitoring_degraded_reason.write() {
        *current = Some(reason);
    }
}

fn monitoring_degraded_reason(&self) -> Option<String> {
    self.monitoring_degraded_reason
        .read()
        .ok()
        .and_then(|current| current.clone())
}
```

Set `snapshot.monitoring.degraded_reason` from this method in
`snapshot_for_state`.

In `app.rs`, replace the startup-aborting `?` with:

```rust
if let Err(error) = crate::hook::start_listener(app.handle().clone()) {
    app.state::<crate::commands::AppState>()
        .set_monitoring_degraded_reason(format!(
            "Live hook listener unavailable: {error:#}"
        ));
}
crate::commands::start_fallback_reconciliation(app.handle().clone());
```

The fallback call must remain outside the error branch.

- [ ] **Step 6: Confirm GREEN and regression behavior**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands::tests
cargo test --manifest-path src-tauri/Cargo.toml app::tests
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --tests
pnpm test
```

Expected: the degradation test and all existing tests pass; the full Windows
target check is now green. PE subsystem behavior is verified from the release
binary in Task 5, not by reading `main.rs`.

- [ ] **Step 7: Commit startup compatibility**

```bash
git add src-tauri/src/main.rs src-tauri/src/app.rs src-tauri/src/commands.rs
git commit -m "fix: make app startup Windows compatible"
```

---

### Task 4: Add Windows icons and NSIS configuration

**Files:**

- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/tauri.conf.json`
- Create: generated files under `src-tauri/icons/`, including `icon.ico`,
  `icon.icns`, `32x32.png`, `128x128.png`, and `128x128@2x.png`
- Test: `src-tauri/src/tray.rs`

- [ ] **Step 1: Add the tray-selection behavior test and record RED**

Extend the existing Rust tray test before adding the selector:

```rust
assert_eq!(tray_icon_is_template(), cfg!(target_os = "macos"));
assert!(tauri::image::Image::from_bytes(TRAY_ICON_BYTES).is_ok());
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tray::tests
```

Expected: compilation fails because `tray_icon_is_template` and
`TRAY_ICON_BYTES` do not exist.

- [ ] **Step 2: Generate the standard Tauri icon set**

Run:

```bash
pnpm tauri icon src-tauri/icons/icon.png
```

Inspect `icon.ico` and `32x32.png` visually. They must contain the existing blue
Pulse mark with a transparent background; do not replace the brand artwork or
use the black macOS tray template for Windows.

- [ ] **Step 3: Configure the bundle**

Set the bundle block to include the generated platform icons and:

```json
"windows": {
  "webviewInstallMode": {
    "type": "downloadBootstrapper",
    "silent": true
  },
  "nsis": {
    "installMode": "currentUser"
  }
}
```

Keep `"targets": "all"` for default local behavior; release and CI commands
select `nsis` explicitly.

- [ ] **Step 4: Select the tray icon at compile time**

Add:

```rust
#[cfg(target_os = "macos")]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-template.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/32x32.png");

fn tray_icon_is_template() -> bool {
    cfg!(target_os = "macos")
}
```

Use `TRAY_ICON_BYTES` and
`.icon_as_template(tray_icon_is_template())`. Extend the tray test:

```rust
assert_eq!(tray_icon_is_template(), cfg!(target_os = "macos"));
assert!(tauri::image::Image::from_bytes(TRAY_ICON_BYTES).is_ok());
```

- [ ] **Step 5: Confirm GREEN and config validity**

Generated icon files and declarative bundle configuration use the approved TDD
exception. Validate them through their actual Tauri consumer:

```bash
cargo test --manifest-path src-tauri/Cargo.toml tray::tests
pnpm tauri info
pnpm tauri build -- --debug --bundles app
```

Expected on macOS: the tray behavior test, Tauri config loading, and debug app
bundle build succeed. The NSIS bundle itself remains `pending-windows-ci`.

- [ ] **Step 6: Commit icons and configuration**

```bash
git add src-tauri/icons src-tauri/src/tray.rs src-tauri/tauri.conf.json
git commit -m "feat: add Windows bundle assets"
```

---

### Task 5: Add automated Windows CI and package simulation

**Files:**

- Create: `scripts/verify-windows-package.ps1`
- Modify: `.github/workflows/ci.yml`
- Modify: `src/__tests__/githubWorkflows.spec.ts`
- Test: `src/__tests__/githubWorkflows.spec.ts`

- [ ] **Step 1: Make the workflow contract RED**

Change the expected CI job keys to:

```ts
expect(Object.keys(workflow.jobs)).toEqual([
  "frontend",
  "rust",
  "rust_windows",
]);
```

Assert `rust_windows` has name `Rust (Windows)`, runner `windows-latest`,
checkout credentials disabled, pnpm `10.33.0`, Node `24`, stable Rust target
`x86_64-pc-windows-msvc`, the Rust cache, and these commands in order:

```text
pnpm install --frozen-lockfile
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build -- --target x86_64-pc-windows-msvc --bundles nsis
pwsh -NoProfile -File scripts/verify-windows-package.ps1 -BundleDirectory src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis
```

Run the focused test. Expected: it fails because `rust_windows` is absent.

- [ ] **Step 2: Create the PowerShell verifier**

The script takes one mandatory `BundleDirectory`, requires exactly one
`*-setup.exe`, and uses `$env:LOCALAPPDATA` without hard-coded user paths:

```powershell
param(
  [Parameter(Mandatory = $true)]
  [string]$BundleDirectory
)

$ErrorActionPreference = "Stop"
$setups = @(Get-ChildItem -Path $BundleDirectory -Filter "*-setup.exe" -File)
if ($setups.Count -ne 1) {
  throw "Expected exactly one NSIS setup executable, found $($setups.Count)"
}

$installDirectory = Join-Path $env:LOCALAPPDATA "Codex Pulse"
$app = Join-Path $installDirectory "CodexPulse.exe"
$uninstaller = Join-Path $installDirectory "uninstall.exe"

Start-Process -FilePath $setups[0].FullName -ArgumentList "/S" -Wait
if (-not (Test-Path $app) -or -not (Test-Path $uninstaller)) {
  throw "NSIS did not install CodexPulse.exe and uninstall.exe"
}

$image = [System.IO.File]::ReadAllBytes($app)
$peOffset = [BitConverter]::ToInt32($image, 0x3c)
$signature = [System.Text.Encoding]::ASCII.GetString($image, $peOffset, 4)
if ($signature -ne "PE`0`0") {
  throw "Installed CodexPulse.exe does not have a valid PE signature"
}
$subsystem = [BitConverter]::ToUInt16($image, $peOffset + 24 + 68)
if ($subsystem -ne 2) {
  throw "Expected Windows GUI subsystem 2, found $subsystem"
}

$hook = Start-Process -FilePath $app -ArgumentList "__hook" -PassThru -Wait
if ($hook.ExitCode -ne 0) {
  throw "Installed __hook helper exited with $($hook.ExitCode)"
}

Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait
$deadline = (Get-Date).AddSeconds(15)
while ((Test-Path $app -or Test-Path $uninstaller) -and (Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 250
}
if (Test-Path $app -or Test-Path $uninstaller) {
  throw "NSIS uninstall did not remove the installed application"
}
```

- [ ] **Step 3: Add `Rust (Windows)` to CI**

Add a separate `rust_windows` job. Use the same checkout, pnpm, Node, Rust, and
Rust-cache action versions as the existing jobs. Set:

```yaml
name: Rust (Windows)
runs-on: windows-latest
```

Set the Rust toolchain target to `x86_64-pc-windows-msvc`, then run the six
commands from Step 1 as separate named steps. Keep workflow permissions
`contents: read`.

- [ ] **Step 4: Confirm local GREEN**

Run:

```bash
pnpm exec vitest run src/__tests__/githubWorkflows.spec.ts
pnpm test
pnpm build
```

Expected: workflow parsing, the full frontend suite, and the frontend build
succeed. Do not claim that the PowerShell script or NSIS package ran locally on
macOS.

- [ ] **Step 5: Commit the CI gate**

```bash
git add scripts/verify-windows-package.ps1 .github/workflows/ci.yml src/__tests__/githubWorkflows.spec.ts
git commit -m "ci: validate Windows builds and installer"
```

---

### Task 6: Synchronize `0.3.0` and build a two-platform Draft workflow

**Files:**

- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `.github/workflows/release.yml`
- Modify: `src/__tests__/githubWorkflows.spec.ts`
- Test: `src/__tests__/githubWorkflows.spec.ts`

- [ ] **Step 1: Record the declarative version-metadata exception**

Before editing, record the current values:

```bash
jq -r '.version' package.json src-tauri/tauri.conf.json
sed -n '/^\[package\]/,/^\[/p' src-tauri/Cargo.toml | sed -n '1,8p'
```

Expected: all versions are `0.2.0` and the Cargo description is macOS-only.
These declarative edits use the approved TDD exception; the release guard and
actual build consume them.

- [ ] **Step 2: Make the release-workflow contract RED**

Update the workflow types to allow `needs` and `strategy.matrix.include`.
Require exactly `guard` and `release` jobs. Assert:

- `guard` runs on `ubuntu-latest`, checks out with `fetch-depth: 0` and
  credentials disabled, validates strict SemVer, extracts all three app
  versions, requires equality, authenticates only the ancestry fetch, and
  checks the tagged SHA is in `origin/main`;
- `release.needs` is `guard`, `strategy.fail-fast` is false, and the matrix has
  one macOS ARM64/DMG entry and one Windows x64/NSIS entry;
- every release checkout disables persisted credentials;
- the release job repeats frozen install, frontend tests, Rust tests, and uses
  `tauri-apps/tauri-action@v1`; and
- action inputs keep `releaseDraft: true`, `uploadUpdaterJson: false`, and use
  matrix target/bundle values.

Run the focused test. Expected: it fails against the one-job macOS workflow.

- [ ] **Step 3: Synchronize version metadata**

Set `0.3.0` in `package.json`, `src-tauri/Cargo.toml`, and
`src-tauri/tauri.conf.json`. Change only the Cargo description's platform word
from `macOS` to `desktop`. Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

This updates the root package entry in `src-tauri/Cargo.lock`.

- [ ] **Step 4: Extract a platform-neutral guard job**

The `guard` job must run on `ubuntu-latest`, checkout full history without
persisted credentials, and use one Bash step. In addition to the existing
strict SemVer and authenticated `origin/main` fetch, extract:

```bash
package_version="$(jq -r '.version' package.json)"
tauri_version="$(jq -r '.version' src-tauri/tauri.conf.json)"
cargo_version="$(sed -n '/^\[package\]/,/^\[/s/^version = \"\\([^\"]*\\)\"/\\1/p' src-tauri/Cargo.toml | head -n 1)"
```

Require all three values to equal `GITHUB_REF_NAME` before fetching main.

- [ ] **Step 5: Add the release matrix**

Use:

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - label: macOS ARM64
        platform: macos-15
        target: aarch64-apple-darwin
        bundles: dmg
        apple-signing-identity: "-"
      - label: Windows x64
        platform: windows-latest
        target: x86_64-pc-windows-msvc
        bundles: nsis
        apple-signing-identity: ""
```

The release job runs on `${{ matrix.platform }}`, depends on `guard`, installs
the matrix Rust target, runs frozen install plus frontend/Rust tests, and calls
the action with:

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  APPLE_SIGNING_IDENTITY: ${{ matrix.apple-signing-identity }}
with:
  tagName: ${{ github.ref_name }}
  releaseName: "Codex Pulse __VERSION__"
  releaseCommitish: ${{ github.sha }}
  generateReleaseNotes: true
  releaseDraft: true
  prerelease: false
  uploadUpdaterJson: false
  args: "--target ${{ matrix.target }} --bundles ${{ matrix.bundles }}"
```

Do not add updater keys, Windows certificate secrets, release publication, or
the NSIS install script to this release job.

- [ ] **Step 6: Confirm GREEN**

Run:

```bash
pnpm exec vitest run src/__tests__/githubWorkflows.spec.ts
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri info
```

Expected: all local contracts and regressions pass.

- [ ] **Step 7: Commit version and release workflow**

```bash
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json .github/workflows/release.yml src/__tests__/githubWorkflows.spec.ts
git commit -m "ci: build Windows draft releases"
```

---

### Task 7: Align public platform and support documentation

**Files:**

- Modify: `README.md`
- Modify: `docs/README.zh-CN.md`
- Modify: `CONTRIBUTING.md`
- Modify: `SECURITY.md`
- Modify: `.github/ISSUE_TEMPLATE/bug_report.yml`
- Modify: `src/__tests__/githubCommunity.spec.ts`
- Test: `src/__tests__/githubCommunity.spec.ts`

- [ ] **Step 1: Make the community contract RED**

Replace macOS-only identity assertions with aligned platform assertions. Require
both READMEs to contain:

- `Windows 11 x64`;
- `native Codex` / `原生 Codex`;
- `WSL` marked unsupported;
- `unsigned` / `未签名`;
- `pending-user-eyeball`; and
- the existing independent-project, signing, build, and license disclosures.

Require `CONTRIBUTING.md` to mention `Rust (Windows)` and the Windows Tauri
build command. Require `SECURITY.md` to request OS/version/architecture rather
than only macOS. Change the issue-form contract to expect IDs:

```text
version, operating_system, architecture, codex_environment,
problem, steps, expected, actual, logs, privacy
```

Run:

```bash
pnpm exec vitest run src/__tests__/githubCommunity.spec.ts
```

Expected: the new platform assertions fail.

- [ ] **Step 2: Update English and Chinese README together**

Describe Codex Pulse as a compact macOS and Windows desktop companion. Add a
support table:

| Environment | Status |
| --- | --- |
| macOS ARM64 | Existing experimental DMG |
| Windows 11 x64 native Codex | `0.3.0` MVP, unsigned experimental NSIS |
| WSL, Windows ARM64, Windows 10 | Unsupported |

Add the exact Windows source build:

```powershell
pnpm install --frozen-lockfile
pnpm tauri build -- --target x86_64-pc-windows-msvc --bundles nsis
```

State the output root
`src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis`, the
`downloadBootstrapper` network requirement when WebView2 is missing, and that
SmartScreen/enterprise policy must not be bypassed. Keep the Gatekeeper text
for macOS.

- [ ] **Step 3: Update contributor, security, and issue guidance**

`CONTRIBUTING.md` must list both platform CI checks and commands.
`SECURITY.md` must request sanitized OS, architecture, and Codex environment.
The bug form must use:

- a required OS version input;
- an architecture dropdown with Apple Silicon, Intel macOS, Windows x64, and
  Other;
- a Codex environment dropdown with native macOS, native Windows, WSL, and
  Other; and
- the existing privacy confirmation.

Do not describe a WSL selection as supported; it only improves issue triage.

- [ ] **Step 4: Confirm GREEN**

Run:

```bash
pnpm exec vitest run src/__tests__/githubCommunity.spec.ts
pnpm test
```

Expected: all public-identity and full frontend tests pass.

- [ ] **Step 5: Commit the documentation**

```bash
git add README.md docs/README.zh-CN.md CONTRIBUTING.md SECURITY.md .github/ISSUE_TEMPLATE/bug_report.yml src/__tests__/githubCommunity.spec.ts
git commit -m "docs: document Windows native MVP"
```

---

### Task 8: Run local regression verification and independent review

**Files:**

- Review all files changed in Tasks 1-7
- Do not create acceptance screenshots on macOS and label them as Windows proof

- [ ] **Step 1: Run the complete local verification from a clean process**

Run exactly:

```bash
pnpm install --frozen-lockfile
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm tauri build -- --debug --bundles app
git diff --check
git status --short
```

Expected: every command succeeds; status contains no uncommitted implementation
files.

- [ ] **Step 2: Inspect compile-time boundaries**

Run:

```bash
rg -n "std::os::unix|UnixListener|UnixStream|ActivationPolicy" src-tauri/src
rg -n "named_pipe|windows_subsystem|same_file::Handle|degraded_reason" src-tauri/src src-tauri/tests
```

Expected: Unix symbols occur only in `hook/unix.rs` or `#[cfg(unix)]` identity
code; `ActivationPolicy` occurs only under the macOS helper; Windows symbols
occur only behind Windows target guards.

- [ ] **Step 3: Invoke `superpowers:requesting-code-review`**

Request an independent review of the exact branch diff against its merge base.
The review must report severity, file and line, repair advice, and a
merge-readiness verdict. Resolve every material finding with a new RED test and
focused commit; do not waive a Windows compile concern based on a macOS build.

- [ ] **Step 4: Repeat the full verification after review fixes**

Rerun all commands from Step 1 and record the new exact test counts, build
result, HEAD, and `git status --short`.

---

### Task 9: Open the pull request and prove Windows CI

**Files:**

- No repository file is modified unless a CI failure requires a reviewed fix
- Remote: branch `codex/windows-native-mvp`
- Remote: implementation pull request

- [ ] **Step 1: AskHuman gate for remote delivery**

Attach the final diff summary, independent review, exact local verification,
and the list of still-unverified Windows items. Ask for permission to push the
feature branch and create a pull request. Do not infer approval from design or
plan approval.

- [ ] **Step 2: Push only after approval**

Run:

```bash
git push -u origin codex/windows-native-mvp
gh pr create --base main --head codex/windows-native-mvp --title "feat: add Windows native MVP" --body-file /tmp/codex-pulse-windows-pr.md
```

The PR body must include:

- Windows 11 x64 native scope and explicit WSL exclusion;
- user-visible behavior;
- local verification commands and exact results;
- unsigned NSIS/SmartScreen limitation;
- automated hosted-runner scope; and
- the full `pending-user-eyeball` list.

- [ ] **Step 3: Wait for all three checks**

Run:

```bash
gh pr checks --watch
```

Required success names are exactly `Frontend`, `Rust`, and `Rust (Windows)`.
Inspect the complete Windows job trace, not only the conclusion. Confirm that
the Windows Rust tests, frontend tests/build, NSIS build, silent install,
installed `__hook`, and uninstall steps each ran and passed.

- [ ] **Step 4: Repair failures through TDD**

For any failure, reproduce locally when the platform permits; otherwise use the
full Windows trace to add a focused regression test, commit the minimum fix,
push, and wait for a new full check run. Do not use `continue-on-error` or
remove an acceptance step to make CI green.

- [ ] **Step 5: Obtain merge approval**

Send the green PR URL and check evidence through AskHuman. Merge only if the
user explicitly authorizes it. After merge, fetch `origin/main` and verify the
merge commit contains the reviewed implementation.

---

### Task 10: Require the new Windows check without disturbing protection

**Files:**

- No repository files
- Remote: `main` required status checks

- [ ] **Step 1: Prove the check exists on merged `main`**

Run:

```bash
git fetch origin main
main_sha="$(git rev-parse origin/main)"
main_ci_run_id="$(gh run list --commit "$main_sha" --workflow CI --limit 1 --json databaseId --jq '.[0].databaseId')"
test -n "$main_ci_run_id"
gh run view "$main_ci_run_id" --json headSha,status,conclusion,jobs,url
gh run view "$main_ci_run_id" --log
```

Verify the returned run's `headSha` is exactly `main_sha` and `Rust (Windows)`
succeeded there.

- [ ] **Step 2: Read the current protection source of truth**

Run:

```bash
gh api repos/qwertyerge/codex-pulse/branches/main/protection
gh api repos/qwertyerge/codex-pulse/branches/main/protection/required_status_checks
```

Save the exact JSON in the AskHuman review. Confirm strict mode and the existing
required contexts before proposing the change. Abort the fixed command below
if the existing contexts are not exactly `Frontend` and `Rust`; return to
AskHuman with the actual JSON instead of replacing an unreviewed context.

- [ ] **Step 3: AskHuman gate for branch-protection mutation**

Ask permission to add only `Rust (Windows)` while keeping `Frontend`, `Rust`,
strict mode, and every unrelated protection field unchanged.

- [ ] **Step 4: Patch only required status checks after approval**

Run:

```bash
gh api --method PATCH repos/qwertyerge/codex-pulse/branches/main/protection/required_status_checks \
  -F strict=true \
  -f 'contexts[]=Frontend' \
  -f 'contexts[]=Rust' \
  -f 'contexts[]=Rust (Windows)'
```

Immediately read both protection endpoints again. Expected: strict is true,
the three contexts are required, and unrelated protection remains identical.

---

### Task 11: Create and verify the `0.3.0` Draft Release

**Files:**

- Create after evidence exists:
  `docs/superpowers/reports/windows-0.3.0-acceptance.md`
- Remote: annotated tag `0.3.0`
- Remote: Draft GitHub Release `0.3.0`

- [ ] **Step 1: AskHuman gate for tag creation and push**

Show:

- the exact fetched `origin/main` SHA;
- all three green required checks on that SHA;
- synchronized `0.3.0` values from the three version files;
- the current branch-protection JSON; and
- confirmation that the workflow leaves the release as Draft.

Ask separately for permission to create and push tag `0.3.0`. Plan approval,
PR approval, and merge approval do not authorize the tag.

- [ ] **Step 2: Create the tag on fetched main after approval**

Run:

```bash
git fetch origin main --tags
git tag -a 0.3.0 origin/main -m "Codex Pulse 0.3.0"
git show --no-patch --decorate 0.3.0
git push origin refs/tags/0.3.0
```

Expected: the annotated tag resolves exactly to the verified `origin/main`
commit.

- [ ] **Step 3: Watch both release matrix entries**

Run:

```bash
release_run_id="$(gh run list --workflow Release --limit 20 --json databaseId,headBranch,event --jq '.[] | select(.headBranch == "0.3.0" and .event == "push") | .databaseId' | head -n 1)"
test -n "$release_run_id"
gh run watch "$release_run_id" --exit-status
gh run view "$release_run_id" --json headSha,status,conclusion,jobs,url
gh run view "$release_run_id" --log
```

Inspect the full guard, macOS ARM64, and Windows x64 logs. Require both
`tauri-action` matrix entries to upload successfully to one Draft.

- [ ] **Step 4: Download and hash both assets**

Create a temporary directory with `mktemp -d`, then run:

```bash
gh release view 0.3.0 --json isDraft,isPrerelease,tagName,targetCommitish,assets,url
gh release download 0.3.0 --dir "$RELEASE_EVIDENCE_DIR"
find "$RELEASE_EVIDENCE_DIR" -maxdepth 1 -type f -exec shasum -a 256 {} \;
find "$RELEASE_EVIDENCE_DIR" -maxdepth 1 -type f -exec stat -f "%N %z" {} \;
```

Use a task-specific `RELEASE_EVIDENCE_DIR` created from `mktemp -d`; do not use
`$HOME` or a repository directory for downloads. Expected: one ARM64 DMG and
one x64 NSIS setup executable; the release is Draft and not a prerelease.

- [ ] **Step 5: Record durable evidence without overstating acceptance**

Create `docs/superpowers/reports/windows-0.3.0-acceptance.md` with:

- tag and exact commit SHA;
- PR and CI run URLs;
- Draft Release URL;
- exact asset names, byte sizes, and SHA-256 values;
- every automatic gate with its evidence link;
- every interactive item below marked `pending-user-eyeball`:
  real active task discovery, hook refresh latency, `codex://` open, Explorer
  open, tray contrast/menu, transparent light/dark rendering, Pin/Unpin,
  close-to-hide, single instance, no console flash, and single/multi-display
  maximization; and
- a statement that the Draft is unsigned and unpublished.

Open a focused follow-up documentation PR for this evidence. Do not amend or
retag `0.3.0`, and do not publish the Draft.

- [ ] **Step 6: Final verification report**

Use `superpowers:verification-before-completion`. Report separately:

1. local macOS regression evidence;
2. Windows hosted-runner compilation/test/install evidence;
3. Draft asset evidence;
4. branch-protection state; and
5. the unchanged `pending-user-eyeball` list.

Do not call Windows MVP final interactive acceptance complete until a
user-controlled Windows 11 x64 desktop supplies that evidence.
