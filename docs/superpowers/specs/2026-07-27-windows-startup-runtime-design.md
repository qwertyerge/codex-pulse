# Windows Startup Runtime Fix Design

## Context

Codex Pulse `0.3.0` installs successfully on Windows, but its normal GUI
startup path exits about one second after launch with code `101`. Repeated
launches on a Parallels Desktop Windows 11 ARM64 guest running the released
x64 executable produced the same panic:

```text
there is no reactor running, must be called from the context of a Tokio 1.x runtime
```

The application creates its main window before it starts the Windows hook
listener. The window can therefore appear briefly before
`tokio::net::windows::named_pipe::ServerOptions::create` panics on Tauri's
synchronous setup thread.

The Windows hook unit tests do not expose the defect because they run inside
`#[tokio::test]`. The package verifier also misses it because it launches only
`CodexPulse.exe __hook`, which exits before `app::run()` and never creates the
named-pipe server.

This design supersedes the synchronous initial Windows pipe binding described
in the Windows Native MVP design. The published platform contract remains
Windows 11 x64; the Parallels ARM64 guest provides x64-emulation regression
evidence, not native ARM64 support.

## Goal

Make the Windows GUI startup path initialize its named-pipe listener only
inside Tauri's Tokio runtime, preserve degraded monitoring when the listener
cannot bind, and add a package gate that fails if the installed GUI exits or
never creates its main window.

## Non-Goals

- Adding a native ARM64 Windows target or installer.
- Expanding support to Windows 10, WSL, MSI, or portable packages.
- Changing the hook protocol, endpoint naming, session model, or fallback
  reconciliation interval.
- Changing app version numbers, signing artifacts, publishing a release, or
  updating the existing `0.3.0` Draft.
- Completing the broader interactive Windows acceptance checklist.

## Runtime Design

`PipeListener::bind_at` becomes an `async fn`. Constructing its future performs
no named-pipe operation; `ServerOptions::create` runs only when Tauri's async
runtime polls that future.

`start_listener` submits one task to `tauri::async_runtime::spawn`. The task:

1. resolves the per-user endpoint;
2. awaits `PipeListener::bind_at`;
3. runs the existing notification accept loop after a successful bind; or
4. records a listener-unavailable degradation reason after a failed bind.

The main window and fallback reconciliation remain available even if the live
hook listener cannot start. Existing accepted-connection behavior remains
unchanged: create the replacement pipe instance before dropping the connected
instance, schedule a refresh, and report any terminal loop failure.

The Windows `start_listener` function retains the shared
`anyhow::Result<()>` signature for parity with Unix. On Windows it returns
after scheduling the task; runtime bind failures are reported through
`MonitoringView.degradedReason` instead of unwinding GUI setup.

## Error Handling

An initial bind failure uses the existing degradation channel with the message
prefix `Live hook listener unavailable`. A failure after the listener has
started retains the prefix `Live hook listener stopped`.

Neither error exits the application. No dialog, retry loop, new log file, or
new public API is introduced. The existing 60-second fallback reconciliation
continues to provide eventual refresh.

## Automated Regression Coverage

### Rust

A synchronous `#[test]` constructs and drops the `PipeListener::bind_at`
future without a Tokio runtime. This proves that creating the startup task no
longer eagerly accesses a reactor.

The existing Windows `#[tokio::test]` awaits `bind_at` and continues to prove
that two sequential notifications are accepted through replacement pipe
instances.

### Installed Package Smoke

After silent NSIS installation and PE validation,
`scripts/verify-windows-package.ps1` launches the installed
`CodexPulse.exe` with no arguments and redirects stdout and stderr to a
task-specific diagnostic directory.

The verifier must:

1. observe a non-zero main-window handle within 15 seconds;
2. require the process to remain alive for another 3 seconds;
3. fail with the exit code and captured stderr if the process exits early;
4. fail with diagnostics if no main window appears before the deadline; and
5. terminate only the exact launched process tree in a `finally` block before
   the existing `__hook` and uninstall checks continue.

The smoke uses the ordinary production startup path. It does not add a
test-only application argument that could bypass future initialization bugs.

The existing package-verifier source contract test is extended so removal of
the normal-launch gate is visible in the cross-platform frontend suite. The
Windows CI execution remains the authoritative behavioral proof.

## Parallels Desktop Acceptance

Build an x64 NSIS package from the fix branch and install it over the existing
`0.3.0` per-user installation in the same Windows 11 ARM64 guest.

Acceptance requires:

- normal launch creates a visible Codex Pulse window and remains alive for at
  least 10 seconds;
- stderr contains no missing-reactor panic;
- three repeated normal launches do not exit with code `101`;
- a second launch while the first instance runs does not create a second
  long-lived application process and leaves the main window available; and
- `CodexPulse.exe __hook` still exits successfully.

These checks establish the startup fix under Windows x64 emulation. They do
not claim native ARM64 support or close unrelated interactive acceptance
items.

## Delivery Boundary

Implementation starts from `origin/main` on
`codex/fix-windows-startup`. The task may commit the design, plan, source,
tests, and verifier changes locally. It does not push, open a pull request,
modify the Draft Release, or publish an artifact without separate approval.
