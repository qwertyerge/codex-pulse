# Windows Startup Runtime Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent the Windows GUI from creating its Tokio named-pipe listener outside Tauri's async runtime, and make the installed-package gate exercise the normal GUI startup path.

**Architecture:** Defer `PipeListener` construction by making `bind_at` asynchronous and polling it inside the task already owned by `tauri::async_runtime`. Extend the Windows package verifier to launch the installed application with no arguments, require a main window and continued process survival, capture diagnostics, and terminate only that exact smoke-test process tree.

**Tech Stack:** Rust 2021, Tokio 1, Tauri 2, PowerShell 7/Windows PowerShell, NSIS, cargo-xwin, Parallels Desktop Windows 11.

## Global Constraints

- Preserve the existing Windows named-pipe protocol, endpoint naming, and replacement-instance ordering.
- A hook-listener bind failure must degrade live monitoring without exiting the GUI.
- The package smoke must use the ordinary no-argument production startup path.
- The package smoke must observe a main-window handle within 15 seconds and another 3 seconds of process survival.
- Windows 11 x64 remains the supported contract; PD ARM64 provides x64-emulation evidence only.
- Do not add dependencies, version changes, native ARM64 output, signing, publishing, pushes, or pull requests.
- Follow red-green-refactor: no production change before its failing regression has been observed.

---

### Task 1: Defer Windows named-pipe creation into Tauri's runtime

**Files:**
- Modify: `src-tauri/src/hook/windows.rs:31-43`
- Modify: `src-tauri/src/hook/windows.rs:66-70`
- Modify: `src-tauri/src/hook/windows.rs:100-113`
- Test: `src-tauri/src/hook/windows.rs:122-240`

**Interfaces:**
- Consumes: `tauri::async_runtime::spawn`, `PipeListener::bind_at(endpoint)`, `run_listener_loop`.
- Produces: `async fn PipeListener::bind_at(String) -> std::io::Result<PipeListener>` and non-fatal initial-bind reporting through `MonitoringView.degradedReason`.

- [ ] **Step 1: Preserve a pre-fix Windows package for the package-smoke RED phase**

Use the current `origin/main`-equivalent source before changing Rust:

```bash
task_rust_bin="$(dirname "$(rustup which rustc)")"
PATH="$task_rust_bin:$PATH" pnpm tauri build \
  --runner "$HOME/.cargo/bin/cargo-xwin" \
  --target x86_64-pc-windows-msvc \
  --bundles nsis \
  --ci
task_baseline_dir="$(mktemp -d -t codex-pulse-windows-baseline.XXXXXX)"
cp "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Codex Pulse_0.3.0_x64-setup.exe" \
  "$task_baseline_dir/Codex Pulse_0.3.0_x64-setup.exe"
shasum -a 256 "$task_baseline_dir/Codex Pulse_0.3.0_x64-setup.exe"
```

Record `task_baseline_dir` for Task 2. Do not add generated schema or build
artifacts to Git.

- [ ] **Step 2: Write the failing runtime-boundary test**

Add this synchronous test beside the existing Windows hook tests:

```rust
#[test]
fn constructing_a_listener_future_does_not_require_a_runtime() {
    let unique_id = uuid::Uuid::from_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            ^ ((std::process::id() as u128) << 96),
    );
    let endpoint =
        format!(r"\\.\pipe\com.codexpulse.desktop.test.{unique_id}.events");

    let future = PipeListener::bind_at(endpoint);
    drop(future);
}
```

The production change this test catches is changing `bind_at` back to a
synchronous function that eagerly calls `ServerOptions::create` before a
Tokio reactor exists.

- [ ] **Step 3: Cross-compile and run the new test to verify RED**

Compile the Windows library test executable with the rustup toolchain and
cargo-xwin:

```bash
task_rust_bin="$(dirname "$(rustup which rustc)")"
task_test_exe="$(
  PATH="$task_rust_bin:$PATH" cargo xwin test \
    --manifest-path src-tauri/Cargo.toml \
    --target x86_64-pc-windows-msvc \
    --lib \
    --no-run \
    --message-format=json-render-diagnostics |
    jq -r '
      select(
        .reason == "compiler-artifact" and
        .profile.test == true and
        (.target.kind | index("lib"))
      ) |
      .executable
    ' |
    tail -1
)"
test -n "$task_test_exe"
```

Copy that exact executable into the Windows user's temporary directory and
run:

```powershell
& $copiedTestExecutable `
  "hook::windows::tests::constructing_a_listener_future_does_not_require_a_runtime" `
  --exact `
  --nocapture
```

Expected: FAIL with `there is no reactor running`. A compile error or a
different panic is not the expected RED.

- [ ] **Step 4: Implement the minimal runtime-safe listener startup**

Change `start_listener` so bind happens inside the spawned future:

```rust
pub fn start_listener(app: AppHandle) -> anyhow::Result<()> {
    tauri::async_runtime::spawn(async move {
        let mut listener = match PipeListener::bind_at(endpoint_name()).await {
            Ok(listener) => listener,
            Err(error) => {
                report_listener_unavailable(&app, error);
                return;
            }
        };
        let app_for_refresh = app.clone();
        run_listener_loop(
            &mut listener,
            move || crate::commands::schedule_refresh(app_for_refresh.clone()),
            move |error| report_listener_failure(&app, error),
        )
        .await;
    });
    Ok(())
}
```

Add the initial-bind degradation reporter without changing the existing
terminal-loop message:

```rust
fn report_listener_unavailable(app: &AppHandle, error: std::io::Error) {
    app.state::<crate::commands::AppState>()
        .set_monitoring_degraded_reason(format!(
            "Live hook listener unavailable: {error}"
        ));
    let _ = app.emit(crate::hook::SESSIONS_CHANGED_EVENT, ());
}
```

Make binding lazy:

```rust
async fn bind_at(endpoint: String) -> std::io::Result<Self> {
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&endpoint)?;
    Ok(Self { endpoint, server })
}
```

Update the existing async replacement-instance test to use:

```rust
let mut listener = PipeListener::bind_at(endpoint.clone()).await.unwrap();
```

- [ ] **Step 5: Re-run the focused Windows tests to verify GREEN**

Rebuild the library test executable with the command from Step 3, copy it to
the Windows guest, and run:

```powershell
& $copiedTestExecutable "hook::windows::tests" --nocapture
```

Expected: all Windows hook unit tests pass, including the new synchronous
runtime-boundary test and the two-connection replacement test.

- [ ] **Step 6: Run host regressions and commit Task 1**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
git diff -- src-tauri/src/hook/windows.rs
git add src-tauri/src/hook/windows.rs
git commit -m "fix: initialize Windows hook inside runtime"
```

Expected: macOS Rust tests pass; the Windows-only file remains covered by the
cross-compiled executable run in Step 5.

---

### Task 2: Make the NSIS verifier exercise normal GUI startup

**Files:**
- Modify: `scripts/verify-windows-package.ps1:1-305`

**Interfaces:**
- Consumes: the validated installed path in `$script:app` and the existing guarded cleanup policy.
- Produces: `Invoke-ApplicationStartupSmoke`, which fails on early exit or missing window and always stops only its exact launched process tree.

- [ ] **Step 1: Add diagnostic and startup-smoke helpers**

Add:

```powershell
function Read-ProcessDiagnostic {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    return "<not created>"
  }

  $content = Get-Content -LiteralPath $Path -Raw
  if ([string]::IsNullOrWhiteSpace($content)) {
    return "<empty>"
  }
  return $content.Trim()
}

function Invoke-ApplicationStartupSmoke {
  param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [int]$WindowTimeoutSeconds = 15,

    [int]$SurvivalSeconds = 3
  )

  $diagnosticDirectory = Join-Path `
    $env:TEMP `
    ("codex-pulse-startup-smoke-" + [guid]::NewGuid().ToString("N"))
  New-Item -ItemType Directory -Path $diagnosticDirectory | Out-Null
  $stdout = Join-Path $diagnosticDirectory "stdout.log"
  $stderr = Join-Path $diagnosticDirectory "stderr.log"
  $process = $null

  try {
    $process = Start-Process `
      -FilePath $FilePath `
      -PassThru `
      -RedirectStandardOutput $stdout `
      -RedirectStandardError $stderr

    $windowDeadline = (Get-Date).AddSeconds($WindowTimeoutSeconds)
    $windowObserved = $false
    while ((Get-Date) -lt $windowDeadline) {
      if ($process.HasExited) {
        $process.WaitForExit()
        $diagnostic = Read-ProcessDiagnostic -Path $stderr
        throw "Installed application exited before startup completed with code $($process.ExitCode). stderr: $diagnostic"
      }

      $process.Refresh()
      if ($process.MainWindowHandle -ne [IntPtr]::Zero) {
        $windowObserved = $true
        break
      }
      Start-Sleep -Milliseconds 250
    }

    if (-not $windowObserved) {
      $diagnostic = Read-ProcessDiagnostic -Path $stderr
      throw "Installed application did not create a main window within $WindowTimeoutSeconds seconds. stderr: $diagnostic"
    }

    $survivalDeadline = (Get-Date).AddSeconds($SurvivalSeconds)
    while ((Get-Date) -lt $survivalDeadline) {
      if ($process.HasExited) {
        $process.WaitForExit()
        $diagnostic = Read-ProcessDiagnostic -Path $stderr
        throw "Installed application exited during the $SurvivalSeconds-second survival window with code $($process.ExitCode). stderr: $diagnostic"
      }
      Start-Sleep -Milliseconds 250
    }
  }
  finally {
    if ($null -ne $process -and -not $process.HasExited) {
      $taskkill = Join-Path $env:SystemRoot "System32\taskkill.exe"
      & $taskkill /PID $process.Id /T /F | Out-Null
      $taskkillExitCode = $LASTEXITCODE
      $process.Refresh()
      if ($taskkillExitCode -ne 0 -and -not $process.HasExited) {
        $script:cleanupMutationAllowed = $false
        throw "Failed to stop the startup-smoke process tree for PID $($process.Id); taskkill exited with $taskkillExitCode"
      }

      if (-not $process.WaitForExit(10000)) {
        $script:cleanupMutationAllowed = $false
        throw "Startup-smoke process tree for PID $($process.Id) did not exit within 10 seconds"
      }
    }
  }
}
```

Call it immediately after PE subsystem validation and before the existing
`__hook` helper:

```powershell
Invoke-ApplicationStartupSmoke -FilePath $script:app
```

- [ ] **Step 2: Run the verifier against the preserved pre-fix bundle to verify RED**

From the Windows guest, invoke the edited script through the shared Mac path
and pass the `task_baseline_dir` bundle directory from Task 1.

Expected: FAIL after approximately one second. The error must report exit code
`101` and stderr containing `there is no reactor running`. The script's outer
`finally` must leave no installed `CodexPulse.exe` or `uninstall.exe`.

- [ ] **Step 3: Build the fixed x64 NSIS bundle**

```bash
task_rust_bin="$(dirname "$(rustup which rustc)")"
PATH="$task_rust_bin:$PATH" pnpm tauri build \
  --runner "$HOME/.cargo/bin/cargo-xwin" \
  --target x86_64-pc-windows-msvc \
  --bundles nsis \
  --ci
shasum -a 256 \
  "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Codex Pulse_0.3.0_x64-setup.exe"
```

Expected: frontend build, Windows release executable, and NSIS bundle finish
with exit code `0`. `lld-link` missing-PDB warnings are non-fatal cross-build
diagnostics and must not be reported as test failures.

- [ ] **Step 4: Run the verifier against the fixed bundle to verify GREEN**

Run `scripts/verify-windows-package.ps1` in the PD guest against the fixed
bundle directory through the shared Mac path.

Expected: the normal app creates a window and survives the 3-second gate,
`__hook` exits `0`, NSIS uninstalls successfully, and no installed app or
uninstaller remains after the verifier.

- [ ] **Step 5: Review script safety and commit Task 2**

Confirm from the diff that:

- the launched path is still the validated per-user `$script:app`;
- only the `Start-Process` object returned by that launch is terminated;
- a failed stop disables cleanup mutation;
- early-exit diagnostics include stderr; and
- existing install/uninstall path guards remain unchanged.

Then:

```bash
git diff --check
git diff -- scripts/verify-windows-package.ps1
git add scripts/verify-windows-package.ps1
git commit -m "test: smoke-test installed Windows app"
```

---

### Task 3: Complete regression verification and PD acceptance

**Files:**
- Verify: `src-tauri/src/hook/windows.rs`
- Verify: `scripts/verify-windows-package.ps1`
- Verify: `docs/superpowers/specs/2026-07-27-windows-startup-runtime-design.md`

**Interfaces:**
- Consumes: the fixed NSIS bundle and the installed Windows application.
- Produces: automated test evidence plus a PD startup/single-instance acceptance record in the task handoff.

- [ ] **Step 1: Run the complete host verification suite**

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

Record exact test counts and exit codes. Do not treat host Rust success as
Windows runtime proof.

- [ ] **Step 2: Recompile all Windows tests and rerun focused hook tests in PD**

```bash
task_rust_bin="$(dirname "$(rustup which rustc)")"
PATH="$task_rust_bin:$PATH" cargo xwin test \
  --manifest-path src-tauri/Cargo.toml \
  --target x86_64-pc-windows-msvc \
  --no-run
```

Copy the Windows library test executable to the guest and run
`hook::windows::tests` with `--nocapture`. Record the passed/failed count.
Native `windows_hook_cli` execution remains CI evidence because its embedded
`CARGO_BIN_EXE_CodexPulse` path is produced on the build host.

- [ ] **Step 3: Install the fixed NSIS through the visible Windows installer**

Launch the fixed setup from the shared Mac bundle directory under the
interactive `loki` user. Do not bypass SmartScreen. Accept the default
per-user destination:

```text
C:\Users\loki\AppData\Local\Codex Pulse
```

Finish with `Run Codex Pulse` selected and capture the visible window.

- [ ] **Step 4: Verify sustained normal startup three times**

For each run, redirect stdout/stderr to a unique user-temp diagnostic
directory, observe the visible window, and poll for at least 10 seconds.

Expected for all three runs:

- `CodexPulse.exe` remains alive;
- a non-zero main-window handle is present;
- stderr does not contain `there is no reactor running`; and
- no run exits with code `101`.

Terminate only the exact process tree between standalone runs.

- [ ] **Step 5: Verify single-instance and hook-helper behavior**

Keep one normal instance running, launch the installed executable again, and
require the second process to exit without creating a second long-lived
instance. Confirm the original window remains available.

Then run:

```powershell
$process = Start-Process `
  -FilePath "C:\Users\loki\AppData\Local\Codex Pulse\CodexPulse.exe" `
  -ArgumentList "__hook" `
  -PassThru `
  -Wait
if ($process.ExitCode -ne 0) {
  throw "__hook exited with $($process.ExitCode)"
}
```

Expected: one long-lived normal process and `__hook` exit code `0`.

- [ ] **Step 6: Audit the final branch and commits**

```bash
git status --short
git log --oneline --decorate origin/main..HEAD
git diff --stat origin/main..HEAD
git diff --check origin/main..HEAD
git diff origin/main..HEAD -- \
  src-tauri/src/hook/windows.rs \
  scripts/verify-windows-package.ps1 \
  docs/superpowers/specs/2026-07-27-windows-startup-runtime-design.md \
  docs/superpowers/plans/2026-07-27-windows-startup-runtime.md
```

Confirm no generated schemas, build outputs, transcripts, credentials, or
unrelated files are tracked. Do not push or open a pull request.

---

### Task 4: Preserve compact Windows utility-window bounds

**Files:**
- Modify: `src-tauri/src/app.rs`
- Modify: `scripts/verify-windows-package.ps1`
- Modify: `docs/superpowers/specs/2026-07-27-windows-startup-runtime-design.md`

**Interfaces:**
- Consumes: the existing 360 by 420 initial size and 320 to 480 width constraints.
- Produces: a Windows-only non-maximizable startup policy plus a native package geometry gate.

- [ ] **Step 1: Add a failing Windows platform-policy test**

Add a pure main-window platform policy and a host unit test requiring Windows
to start without maximization and with maximization disabled.

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  app::tests::windows_main_window_starts_compact_and_cannot_maximize \
  -- --exact
```

Expected RED: the current all-platform maximize behavior violates both Windows
assertions.

- [ ] **Step 2: Add a failing installed-package geometry smoke**

Extend `Invoke-ApplicationStartupSmoke` to inspect the native main window after
it appears. Normalize its client width using the window DPI, then reject a
window that is maximized or wider than 480 logical pixels.

Run the verifier against the already-built package that still contains the
unconditional maximize call.

Expected RED: startup remains alive, but the verifier rejects the maximized
window and reports its measured geometry.

- [ ] **Step 3: Apply the Windows-only window policy**

Use the pure platform policy from the builder:

- Windows: `maximizable(false)` and no create-time maximize call;
- non-Windows: retain the current maximizable and create-time maximize behavior.

Do not change the existing size constraints, transparency, decorations,
resizability, or macOS activation policy.

- [ ] **Step 4: Rebuild and verify GREEN on PD**

Run the host Rust test, the full host suites, rebuild the x64 NSIS package, and
run the package verifier in PD.

Then install the fixed package for the interactive `loki` user and verify:

- a fresh initial window uses the compact 360 by 420 logical-pixel content size;
- it is not maximized;
- its client width cannot exceed 480 logical pixels through ordinary resize;
- it remains alive without the Tokio reactor panic; and
- single-instance and `__hook` behavior remain intact.

- [ ] **Step 5: Review and commit the extension**

```bash
git diff --check
git diff -- \
  src-tauri/src/app.rs \
  scripts/verify-windows-package.ps1 \
  docs/superpowers/specs/2026-07-27-windows-startup-runtime-design.md \
  docs/superpowers/plans/2026-07-27-windows-startup-runtime.md
git add \
  src-tauri/src/app.rs \
  scripts/verify-windows-package.ps1 \
  docs/superpowers/specs/2026-07-27-windows-startup-runtime-design.md \
  docs/superpowers/plans/2026-07-27-windows-startup-runtime.md
git commit -m "fix: preserve compact Windows window bounds"
```

Do not push, open a pull request, merge, or update the release without a new
delivery decision.

---

### Task 5: Measure and preserve the narrow Windows header

**Files:**
- Modify: `src/components/TopBar.vue`
- Modify: `src/styles.css`
- Create: `src/__tests__/topBarLayout.spec.ts`
- Modify: `src-tauri/src/app.rs`
- Modify: `scripts/verify-windows-package.ps1`
- Modify: `docs/superpowers/specs/2026-07-27-windows-startup-runtime-design.md`

**Interfaces:**
- Consumes: localized active-count text, the complete icon-control group, and native shortcuts.
- Produces: a measurement-derived Windows minimum width, a non-collapsing single-row header, and terminal-free shortcut startup evidence.

- [ ] **Step 1: Record browser and Rust RED**

At 330 pixels, record that `.top-bar__controls` has a 151-pixel client width
against a 155-pixel scroll width and that the top bar overflows horizontally.

Add stylesheet contract tests requiring controls, pulse mark, and brand name
to remain non-shrinking while the active count retains ellipsis. Also reject
TopBar sizing, spacing, or padding overrides inside the 360-pixel media query.
Run the frontend suite and require the new tests to fail.

After PD boundary measurement, update the shared constraint test to expect a
320-pixel minimum and require it to fail against the provisional 300-pixel
value.

- [ ] **Step 2: Apply the single-row flex priority**

Give the brand name an explicit class. Keep the mark, name, and controls at
their intrinsic widths. Leave the active-count span as the only shrinking,
overflow-hidden flex item. Remove the narrow media query's alternate TopBar
size mode so resizing has only a continuous full-count to ellipsis transition.

Do not hide controls, change icon sizes, wrap the header, or change the macOS
minimum width.

- [ ] **Step 3: Measure the post-fix boundary**

Use a real browser layout engine at bounded widths from 280 through 400 pixels.
Confirm that controls remain 157 pixels wide and 14 pixels from the right edge
on both sides of the 360-pixel breakpoint. Record the first width with the
complete 16-pixel brand name, 19-pixel mark, full controls, and no horizontal
page overflow.

Repeat boundary measurement in PD at its actual 240 DPI / 250% display scale.
Request exact DPI-normalized client widths with a hidden native probe and
visually compare 312, 316, and 320 logical pixels. Use 320 pixels for Windows:
312 visibly clips the name, 316 is the rasterization boundary, and 320 leaves
4 logical pixels / 10 physical pixels of margin.

- [ ] **Step 4: Guard shortcuts and terminal-free startup**

Extend the installed package verifier to require desktop and Start Menu
shortcuts that directly target the validated per-user executable with no
arguments.

After the normal startup survival and geometry checks, reject direct
`CodexPulse.exe` children named `cmd.exe`, `OpenConsole.exe`,
`powershell.exe`, `pwsh.exe`, or `WindowsTerminal.exe`.

- [ ] **Step 5: Rebuild and run final PD acceptance**

Build the x64 NSIS package, run the full package verifier, then install under
the interactive `loki` profile with correct user environment variables.
Visually verify the 360-pixel initial header, resize down to the 320-pixel
minimum, and launch from the repaired desktop shortcut.

Acceptance requires no header overlap, only active-count truncation, disabled
maximize state, no terminal window, one long-lived app process, and no terminal
process in the interactive session.

- [ ] **Step 6: Complete full verification and commit**

Run frontend tests/build, host Rust tests, formatting, Windows cross-test
compilation, package verifier, and final diff/status checks. Commit locally on
`codex/fix-windows-startup`; do not push, merge, create a pull request, or
update the release without a new delivery decision.
