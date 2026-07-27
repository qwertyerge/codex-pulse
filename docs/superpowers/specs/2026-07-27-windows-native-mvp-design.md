# Windows Native MVP Design

## Context

Codex Pulse `0.2.0` at `d4049bb` is intentionally macOS-focused. The Vue
frontend is mostly platform-neutral, but the Rust application cannot compile
for Windows because it imports Unix file metadata and Unix domain sockets
unconditionally and calls a macOS-only Tauri activation API without a target
guard. The repository also has no Windows Rust CI, installer build, icon set,
release asset, support documentation, or Windows desktop acceptance evidence.

The Windows Codex host does not require WSL. Its default native mode runs the
agent through PowerShell and uses `%USERPROFILE%\.codex`, which matches Pulse's
existing `home_dir()/.codex` fallback. WSL introduces a separate Linux session
home, Linux project paths, Linux hook execution, and a cross-environment IPC
boundary. Those concerns are not part of this MVP.

## Goal

Ship a `0.3.0` Windows MVP that runs as a native Windows 11 x64 Tauri
application and preserves the current Codex Pulse behavior:

- discover active native Codex sessions from local SQLite and JSONL data;
- refresh promptly from lifecycle hooks and retain the 60-second fallback;
- open an existing task through `codex://threads/<uuid>`;
- open a native Windows project directory through Explorer;
- retain tray, single-instance, Pin/Unpin, close-to-hide, theme, locale,
  session timing, recent-event, prompt, and weekly-quota behavior; and
- preserve the current macOS implementation and release artifact.

The Windows artifact is an unsigned, per-user NSIS installer uploaded to a
Draft GitHub Release. The workflow does not publish the Draft automatically.

## Platform Contract

The supported Windows MVP environment is:

- Windows 11;
- x86_64 using `x86_64-pc-windows-msvc`;
- the native Codex agent running through PowerShell;
- the default native Codex home `%USERPROFILE%\.codex`, or another local
  Windows `CODEX_HOME`; and
- a per-user NSIS installation under
  `%LOCALAPPDATA%\Codex Pulse`.

Automated acceptance uses `windows-latest` hosted runners with synthetic Codex
homes and native Windows build/install checks. It does not substitute for an
interactive Windows desktop review.

## Non-Goals

- Windows 10 or older Windows versions.
- Windows ARM64.
- WSL1 or WSL2 session discovery, Linux-to-Windows path translation, shared
  Windows/WSL hook commands, or IPC bridging.
- Native Windows Mica or Acrylic effects.
- Launch at login or updater support.
- MSI, Microsoft Store, system-wide, or portable installation.
- Windows code signing or suppressing SmartScreen warnings.
- Automatic publication of a GitHub Draft Release.
- Changing the Codex transcript, registry, timing, quota, or localization
  domain models beyond the runtime monitoring degradation state already
  present in `MonitoringView`.

## Architecture

Platform differences remain behind three narrow Rust boundaries:

1. `FileIdentity` identifies a transcript across incremental scans.
2. The Hook transport accepts a local refresh signal.
3. App startup applies platform-specific native configuration.

Session discovery, JSONL and SQLite parsing, registry reconciliation, Tauri
commands, frontend invokes, and Vue components continue to use one shared data
flow. Windows support does not create a second application or a parallel
session model.

### File Identity

Create `src-tauri/src/file_identity.rs` and make it the only identity source
used by both `codex/discovery.rs` and `monitor.rs`.

The public boundary is a non-cloneable `FileIdentity` value constructed from a
path and the metadata already read by the cache refresh:

- Unix stores the existing `device` and `inode` values from
  `std::os::unix::fs::MetadataExt`.
- Windows stores a `same_file::Handle`. Add `same-file` as a direct dependency
  even though it is currently present transitively through `walkdir`.

The Windows handle safely retains the volume and file-index identity without
using unstable standard-library methods or adding application-owned Win32
`unsafe` code. The cache remains bounded by the existing 32 recent session
candidates and 16 quota candidates. Evicting a cache entry drops its identity
handle.

Identity semantics remain unchanged:

- equal identity, larger length, and non-decreasing modification time means an
  append and continues from the byte cursor;
- equal identity, equal length, and equal modification time means no work;
- a changed identity, truncation, or other metadata change rebuilds the cached
  parse; and
- an identity or metadata error removes only that candidate and does not abort
  the remaining scan.

### Hook Transport

Replace `src-tauri/src/hook.rs` with:

- `src-tauri/src/hook/mod.rs` for the shared Tauri-facing API;
- `src-tauri/src/hook/unix.rs` for the existing user-local Unix socket; and
- `src-tauri/src/hook/windows.rs` for a Windows named pipe.

Both platform modules expose the same internal operations: bind a listener,
accept a notification, and notify a running listener. The shared module keeps
the existing public functions:

```rust
pub fn notify_running_instance();
pub fn start_listener(app: tauri::AppHandle) -> anyhow::Result<()>;
```

The Windows endpoint is a named pipe under
`\\.\pipe\com.codexpulse.desktop.<user-scope>.events`. The user scope is a
deterministic hash of the resolved local application-data directory. This keeps
two Windows users on one machine from contending for the same endpoint without
putting a username or filesystem path in the pipe name. Tests inject a unique
endpoint instead of using the production name.

The first pipe instance is created synchronously during setup so a bind failure
is observable. The accept loop runs on Tauri's async runtime. After a client
connects, the listener creates the next server instance before it schedules a
refresh, avoiding a gap in which a hook cannot find an available pipe.
Connections carry no transcript, path, prompt, or token data; the connection
itself is the refresh signal.

The `__hook` process performs one best-effort client connection and exits. A
missing or busy endpoint does not fail or delay the Codex lifecycle hook. The
next fallback reconciliation or application startup recovers current state.

Hook installation continues to merge the current executable's quoted absolute
path into `$CODEX_HOME/hooks.json` without replacing unrelated groups. The
Windows-native command is stored in the ordinary `command` field. WSL-specific
`commandWindows`/Linux command pairs are outside the platform contract.

### App Startup and Degradation

Move the activation-policy call behind a small helper in `app.rs`:

- macOS sets `ActivationPolicy::Accessory`;
- other platforms perform no activation-policy action.

The Liquid Glass plugin remains registered on every desktop platform. Its
existing non-macOS implementation is a safe no-op, so Windows uses the current
CSS transparency without adding a native material dependency.

Add the standard release-only Windows GUI subsystem attribute to `main.rs`.
Debug builds retain a console for diagnostics. Release app launches and
`CodexPulse.exe __hook` invocations must not create a persistent console
window.

A listener bind failure no longer aborts the entire GUI startup. The app:

1. records a concise listener error in runtime state;
2. continues startup and starts fallback reconciliation;
3. returns the error through the existing
   `MonitoringView.degradedReason`; and
4. lets the existing `MonitoringBanner` disclose degraded live monitoring.

The hooks-installed and user-intent states retain their current meaning.
`degradedReason` describes a runtime transport failure rather than pretending
that installed hooks guarantee a working listener.

### Native Handoffs

The existing deep-link implementation remains unchanged. It accepts only UUID
thread IDs and opens `codex://threads/<uuid>` through
`tauri-plugin-opener`.

The existing project-path command also remains the security boundary. A native
Windows path must be non-empty, exist, and be a directory before it is handed
to Explorer. A WSL `/home/...` path is not translated or guessed; it returns
the existing validation error.

Single-instance, always-on-top, and close-to-hide continue to use Tauri's
cross-platform APIs. Windows taskbar, maximization, and multi-display behavior
remain interactive acceptance items because a hosted runner cannot prove them.

### Windows Icons and Visual Treatment

Generate the standard Tauri platform icon set from the existing Pulse app
artwork and include `icon.ico` in the bundle configuration. Windows tray setup
uses a full-color 32-by-32 Pulse icon. macOS keeps the monochrome template icon
and `icon_as_template(true)`.

Windows retains the current transparent window and CSS translucent surfaces.
The MVP requires readable light and dark themes without black backgrounds,
fully transparent content, or unusable tray contrast. It does not promise
Mica, Acrylic, or pixel-identical macOS Liquid Glass.

## Windows Bundle

Extend `tauri.conf.json` with an explicit Windows bundle policy:

- NSIS only for the Windows release command;
- `currentUser` installation mode;
- `downloadBootstrapper` WebView2 installation mode; and
- the generated Windows `.ico`.

Windows 11 normally includes WebView2, while the download-bootstrapper policy
retains Tauri's recovery path for a missing runtime without embedding a fixed
WebView2 distribution.

The installer is deliberately unsigned. Documentation and Draft notes must say
that browser downloads can trigger SmartScreen and that the artifact is for
experimental testing. The project does not recommend bypassing organizational
security policy.

## Continuous Integration

Keep the existing required check names `Frontend` and `Rust`. Add a separate
`Rust (Windows)` job on `windows-latest` so current branch protection does not
break before the new check has run.

The Windows job performs:

1. checkout without persisted credentials;
2. Node.js 24 and pnpm 10.33.0 setup;
3. stable Rust setup for `x86_64-pc-windows-msvc`;
4. `pnpm install --frozen-lockfile`;
5. `pnpm test`;
6. `pnpm build`;
7. `cargo test --manifest-path src-tauri/Cargo.toml`;
8. `pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis`; and
9. the Windows package simulation described below.

After the job succeeds on its pull request and merged `main`, add
`Rust (Windows)` to strict required checks while preserving all existing branch
protection settings.

## Hosted Windows Environment Simulation

Create a checked-in PowerShell verification script for the generated NSIS
bundle. It:

1. requires exactly one `*-setup.exe` in the target NSIS bundle directory;
2. installs it silently with `/S`;
3. confirms `%LOCALAPPDATA%\Codex Pulse\CodexPulse.exe` and
   `%LOCALAPPDATA%\Codex Pulse\uninstall.exe` exist;
4. runs the installed executable with `__hook` while no app listener exists
   and requires a successful, bounded exit;
5. uninstalls silently; and
6. confirms the installed executable and uninstaller were removed.

Rust tests on the same runner use temporary native Windows directories as
synthetic Codex homes. They exercise JSONL, SQLite, config persistence,
project-directory validation, incremental parsing, truncation, and quota
discovery with Windows filesystem semantics.

A Windows-only Cargo integration test binds the production-style named pipe,
starts `CARGO_BIN_EXE_CodexPulse __hook`, observes the connection, and requires
the child process to exit successfully. Unit tests use unique pipe names so
parallel tests do not contend.

This is the maximum automated environment simulation in the MVP. A hosted
service runner cannot prove desktop shell registration, logged-in Codex
behavior, visual compositing, tray appearance, or window interaction.

## Release Workflow

Synchronize the version to `0.3.0` in `package.json`,
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

Refactor the tag-driven Release workflow into:

1. one guard job that validates strict SemVer, exact app-version equality, and
   ancestry in fetched `origin/main`; and
2. a two-entry build matrix that depends on the guard.

The matrix preserves:

- `macos-15`, `aarch64-apple-darwin`, DMG, and ad-hoc Apple identity; and
- `windows-latest`, `x86_64-pc-windows-msvc`, NSIS, and no Windows signing
  identity.

Both entries use `tauri-apps/tauri-action@v1` to upload assets to the same
Draft Release, generate release notes, and leave updater JSON disabled. The
workflow never publishes the Draft.

Creating and pushing the `0.3.0` tag remains a separate, explicit user-approved
release action after the implementation is merged to `main`. The completed
Draft must contain both the existing ARM64 DMG and the x64 NSIS setup
executable. Record each asset's exact name, size, and SHA-256.

## Test Strategy

Implementation follows red-green-refactor and adds focused coverage for:

- cross-platform identity equality, append stability, and replacement
  detection;
- unchanged incremental and truncation behavior on Windows;
- Windows named-pipe listener/client behavior;
- the real `CodexPulse.exe __hook` notification path on Windows;
- quoted Windows executable paths and idempotent hook configuration merging;
- listener-bind degradation with fallback startup and snapshot disclosure;
- conditional macOS activation setup and the Windows GUI subsystem source
  contract;
- platform-appropriate tray icon selection;
- Windows bundle icons, NSIS, WebView2, and install mode configuration;
- parsed workflow contracts for the new CI job and two-platform Draft release;
  and
- aligned English/Chinese README, contribution, security, and bug-report
  platform claims.

Local macOS verification remains:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Windows CI is the authoritative compilation, test, and package-simulation
proof for the new target. macOS local and CI success is the regression proof
for the existing platform.

## Acceptance Status

The implementation may report the automated Windows gate complete only when:

- `Frontend`, `Rust`, and `Rust (Windows)` are green on the implementation pull
  request and merged `main`;
- the Windows Rust suite, frontend suite/build, NSIS build, silent install,
  helper invocation, and silent uninstall all pass;
- strict branch protection requires `Rust (Windows)` after its successful
  introduction; and
- the `0.3.0` Draft contains both expected artifacts with recorded sizes and
  hashes.

The following remain `pending-user-eyeball` until exercised in a user-controlled
Windows 11 x64 desktop with a logged-in native Codex app:

- discovering a real active Codex task;
- lifecycle Hook refresh latency;
- opening the task through `codex://`;
- opening the project through Explorer;
- tray icon contrast and menu behavior;
- light/dark transparent compositing;
- Pin/Unpin, close-to-hide, and single-instance behavior;
- absence of visible release console windows; and
- startup maximization on single- and multi-display desktops.

Automated success must not be described as final interactive Windows
acceptance while this list remains pending.

## Documentation and Support

Update English and Chinese README files, `CONTRIBUTING.md`, `SECURITY.md`, the
bug report form, and release notes together. They must state:

- Windows 11 x64 native Codex is supported by the MVP;
- WSL, ARM64 Windows, Windows 10, and native Mica/Acrylic are not supported;
- the Windows installer is an unsigned experimental Draft artifact;
- Windows source-build and NSIS output paths;
- the Windows-native prerequisite and verification commands; and
- the distinction between automated hosted-runner proof and pending
  interactive desktop acceptance.

No support claim may be broadened to WSL or another architecture based only on
cross-platform frontend tests.

## Security and Privacy

- Named-pipe notifications contain no user data.
- Session files and SQLite remain read-only.
- Hook installation preserves unrelated user-managed Hook groups.
- Synthetic fixtures, not local transcripts, are used in CI.
- Workflows keep checkout credentials disabled.
- No Windows certificate, token, local path, or Codex transcript is committed
  or uploaded.
- The unsigned installer limitation is disclosed; instructions do not weaken
  SmartScreen or enterprise security policy.

## Alternatives Considered

### Loopback TCP on Every Platform

A single TCP implementation appears portable but requires port discovery,
authentication state, stale endpoint cleanup, and firewall/security-software
handling. It also replaces the already stable macOS transport. Native
compile-time modules have a smaller trust and regression surface.

### Windows Polling Without Hooks

Using only the 60-second fallback would avoid Windows IPC work, but it changes a
core live-monitoring behavior and leaves `Enable hooks` semantically different
between platforms. It does not meet the MVP goal.

### Application-Owned Win32 File Identity

Calling `GetFileInformationByHandle` directly would avoid a new direct crate
entry but requires Windows-only unsafe code and duplicates a safe dependency
already present in the lockfile. `same-file` provides the narrower boundary.

### Native Mica or Acrylic in the MVP

Native Windows material would improve visual fidelity but adds another
platform API, dependency, fallback policy, and interactive QA surface before
core monitoring is proven. The MVP retains readable CSS translucency and
defers native material work.

## References

- Codex Windows app:
  https://learn.chatgpt.com/docs/windows/windows-app
- Codex desktop deep links:
  https://learn.chatgpt.com/docs/reference/commands
- Codex lifecycle hooks:
  https://learn.chatgpt.com/docs/hooks
- Tauri Windows installer:
  https://v2.tauri.app/distribute/windows-installer/
- Tauri application icons:
  https://v2.tauri.app/develop/icons/
- Tauri GitHub pipeline:
  https://v2.tauri.app/distribute/pipelines/github/
