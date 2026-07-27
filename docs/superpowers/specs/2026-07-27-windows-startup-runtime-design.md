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

The package smoke runs in the `Rust (Windows)` job. Its Windows execution is
the authoritative behavioral proof; no source-text assertion or simulated
cross-platform PowerShell test is added.

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

## Windows Utility-Window Behavior

PD review found a second Windows-specific startup defect after the runtime
panic was fixed. The builder declares a 360 by 420 logical-pixel window with a
maximum logical width of 480, but `create_main_window` then unconditionally
maximizes it. Windows maximization bypasses the ordinary resize constraint, so
the first visible window covers the work area and exceeds the intended maximum
width.

Windows uses a compact utility-window policy:

- initial inner size remains 360 by 420 logical pixels;
- minimum inner size remains 320 by 360 logical pixels on Windows and macOS;
- maximum inner width remains 480 logical pixels, with no maximum height;
- ordinary resizing remains enabled; and
- maximization is disabled, and the window is not maximized during creation.

macOS and other existing platform behavior remain unchanged. The policy is
represented as a small pure value so host tests can assert the Windows branch
without depending on a live window manager.

The installed-package startup smoke also rejects a Windows window that is
maximized or whose DPI-normalized client width exceeds 480 logical pixels.
This turns the PD finding into a native Windows package gate instead of relying
only on a source-level policy test.

## Narrow Top-Bar Measurement

The Windows 360-pixel initial viewport exposed two interacting responsive
defects. First, the controls container was allowed to shrink even though its
icon buttons cannot. At a 330-pixel browser viewport, the control container
shrunk to 151 pixels while its content required 155 pixels, and the top bar's
scroll width exceeded its client width. Second, a 360-pixel media query changed
the shell padding, brand font, mark size, control gap, and button padding.
Resizing therefore passed through four discrete states: large brand without
ellipsis, large brand with ellipsis, small brand without ellipsis, and small
brand with ellipsis. The two small-brand states visibly collapsed in PD, and
crossing 360 to 361 logical pixels shifted the controls by 3 logical pixels.
At 250% scaling that jump is approximately 7.5 physical pixels.

The single-row behavior is:

- the pulse mark, `Codex Pulse` name, and complete controls never shrink;
- only the localized active-count text may shrink and use ellipsis; and
- TopBar size, spacing, and right-edge inset do not change at 360 pixels.

Real-browser measurement after applying that flex priority found:

- the brand remains 16 pixels and the mark remains 19 pixels at every tested
  width;
- the controls remain 157 pixels wide and 14 pixels from the right viewport
  edge;
- controls move exactly 1 pixel to the right when the viewport moves from 360
  to 361 pixels; and
- 316 pixels clips the brand edge while 320 pixels preserves the full brand.

PD then confirmed the boundary at its actual 240 DPI / 250% scaling. A
360-logical-pixel client area measured exactly 900 physical pixels. At 312
logical pixels the rendered name was visibly clipped to `Codex Puls`, 316
pixels was the rasterization boundary, and 320 pixels preserved the full brand
and every control. The Windows minimum inner width is therefore 320 logical
pixels, leaving 4 logical pixels / 10 physical pixels over the observed
boundary.

## Terminal and Shortcut Boundary

The release executable remains a Windows GUI-subsystem binary and does not
launch a terminal. The terminal observed during PD review came from an
interactive PowerShell measurement task. That task is not part of the product
startup path and must remain hidden or be avoided in subsequent visual runs.

A scheduled-install harness also created desktop and Start Menu shortcuts whose
targets inherited the harness's `systemprofile` path even though the
application was installed for the interactive test account. The PD shortcuts
are repaired to point directly at the per-user `CodexPulse.exe`.

The installed-package gate verifies that both shortcuts:

- exist;
- point directly to the validated per-user executable; and
- contain no arguments that introduce a shell launcher.

The startup smoke also rejects direct terminal-process children of
`CodexPulse.exe`. Final PD acceptance launches through the repaired shortcut
and requires no `pwsh`, PowerShell, `cmd`, OpenConsole, or Windows Terminal
process in the interactive session.

## Delivery Boundary

Implementation starts from `origin/main` on
`codex/fix-windows-startup`. The task may commit the design, plan, source,
tests, and verifier changes locally. It does not push, open a pull request,
modify the Draft Release, or publish an artifact without separate approval.
