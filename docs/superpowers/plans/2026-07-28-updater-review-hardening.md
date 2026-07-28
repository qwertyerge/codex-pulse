# Updater Review Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the four valid PR #17 updater review findings with a zero-argument install contract, lifecycle-owned asynchronous work, native IPC registration coverage, and temporally accurate follow-up evidence.

**Architecture:** The frontend updater keeps one monotonically increasing lifecycle generation and one unique operation token; every asynchronous continuation must still own both before mutating state or performing the next effect. A generic Tauri builder helper registers the dialog, process, and updater plugins for both production and a MockRuntime IPC regression. Historical acceptance evidence remains immutable, with a separate addendum tied to the final implementation commit.

**Tech Stack:** Vue 3, TypeScript 5.9, Vitest 4, Tauri 2.11, Rust 2021, `@tauri-apps/plugin-updater` / `tauri-plugin-updater` 2.10.1, GitHub CLI.

## Global Constraints

- Keep `UPDATE_CHECK_INTERVAL_MS` exactly `21_600_000`; do not change update cadence.
- Keep `@tauri-apps/plugin-updater` and `tauri-plugin-updater` at `2.10.1`; add no dependency.
- Enable Tauri's empty `test` feature only through `[dev-dependencies]`; do not expose `tauri::test` in ordinary production builds.
- `UpdateCandidate.install` must be exactly `install(): Promise<void>`, followed by the existing explicit process-plugin relaunch.
- `stop()` invalidates pending work, returns visible state to `idle`, closes a retained candidate, and still allows a fresh later `start()`.
- An installer call that began before `stop()` may finish, but its stale continuation must not mutate state, close a newer candidate, or relaunch.
- Use lifecycle generation plus operation-token ownership; do not add `AbortController` or a permanently disposed flag.
- Production `run()` and the Rust regression must register dialog, process, and updater plugins through the same generic helper.
- Keep the native IPC regression enabled on Windows; give Cargo's Windows test harness its own embedded Common Controls v6 manifest instead of platform-gating the test.
- Preserve the original `cfc0e9d` / 122-test acceptance table; append follow-up evidence instead of rewriting history.
- Do not rename `src/__tests__/localUpdaterBuild.spec.ts` or mechanically reformat `src/__tests__/updaterManifest.spec.ts`.
- Do not change updater endpoints, signing configuration, signing secrets, UI copy, release publication policy, or Windows install mode.
- Do not introduce credentials, local transcripts, or user-specific absolute paths.
- Reuse PR #17 and `codex/local-updater-signing-runbook`; do not create another branch or PR, force-push, merge, tag, release, or replace the installed app.
- Preserve the host-managed detached checkout; commit on detached HEAD and push only with a normal fast-forward refspec.

---

## File Map

| File | Responsibility in this change |
| --- | --- |
| `src/composables/useUpdater.ts` | Own the public updater candidate contract and lifecycle/operation state machine. |
| `src/__tests__/useUpdater.spec.ts` | Prove the zero-argument install call and all stop/restart race boundaries. |
| `src/__tests__/AppUpdater.spec.ts` | Prove the production Vue/Tauri integration forwards no unsupported install options. |
| `docs/superpowers/plans/2026-07-28-automatic-updates.md` | Keep the earlier executable snippets aligned with the corrected contract and lifecycle. |
| `src-tauri/Cargo.toml` | Enable the existing Tauri crate's test-only MockRuntime API for Rust test targets. |
| `src-tauri/build.rs` | Embed the Common Controls v6 manifest required by dialog-plugin imports into Windows test targets. |
| `src-tauri/src/app.rs` | Share native updater-related plugin registration and expose the hidden integration-test entry point. |
| `src-tauri/tests/updater_plugin_registration.rs` | Probe production plugin registration through MockRuntime IPC on every CI platform. |
| `docs/superpowers/plans/2026-07-28-updater-review-hardening.md` | Record the approved test-feature correction discovered during the Task 3 RED run. |
| `docs/superpowers/reports/automatic-updates-acceptance.md` | Preserve historical evidence and append fresh review-hardening verification. |
| PR #17 inline review threads | Record the disposition of all eight comments in their original threads. |

---

### Task 1: Align the updater install contract

**Files:**

- Modify: `src/__tests__/useUpdater.spec.ts:241-263`
- Modify: `src/__tests__/AppUpdater.spec.ts:111-148`
- Modify: `src/composables/useUpdater.ts:34-40,152-177`
- Modify: `docs/superpowers/plans/2026-07-28-automatic-updates.md:560-590,728-735,843-868,1430-1460`

**Interfaces:**

- Consumes: `@tauri-apps/plugin-updater@2.10.1` `Update.install(): Promise<void>`.
- Produces: `UpdateCandidate.install(): Promise<void>`.
- Produces: `activate(copy)` calling `await update.install()` and then the existing explicit `runtime.relaunch()`.
- Preserves: `plugins.updater.windows.installMode` as the only Windows installer presentation control.

- [ ] **Step 1: Verify the detached checkout and existing PR ancestry before edits**

Run:

```bash
git status --short --branch
git merge-base --is-ancestor 262052c HEAD
git rev-parse origin/codex/local-updater-signing-runbook
```

Expected:

- status is `## HEAD (no branch)` with no changed paths;
- the ancestry command exits `0`;
- the remote PR branch is still `02b165a28c5555cbd3b2f0c7aa4117d7df3e1376`.

If the remote ref differs, stop before editing and inspect the new commits; do not overwrite or force-push them.

- [ ] **Step 2: Change both install assertions to the zero-argument contract**

In `src/__tests__/useUpdater.spec.ts`, rename the install test and replace the option assertion with the exact call-array assertion:

```ts
it("installs with the plugin's zero-argument contract and relaunches when install returns", async () => {
  const update = makeCandidate();
  const runtime = makeRuntime({
    check: vi.fn().mockResolvedValue(update.candidate)
  });
  const updater = useUpdater(runtime);

  updater.start();
  await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
  await updater.activate(confirmation);

  expect(vi.mocked(update.candidate.install).mock.calls).toEqual([[]]);
  expect(update.candidate.close).toHaveBeenCalledTimes(1);
  expect(runtime.relaunch).toHaveBeenCalledTimes(1);
  expect(updater.state.value).toEqual({
    phase: "installing",
    version: "0.4.0"
  });
});
```

In `src/__tests__/AppUpdater.spec.ts`, retain the surrounding dialog and relaunch assertions but replace the unsupported option assertion with:

```ts
expect(update.install.mock.calls).toEqual([[]]);
```

- [ ] **Step 3: Run the focused tests and verify RED**

Run:

```bash
pnpm test -- src/__tests__/useUpdater.spec.ts src/__tests__/AppUpdater.spec.ts
```

Expected: both install-path assertions fail because the current implementation records one call containing `{ restartAfterInstall: true }` instead of one empty argument list.

- [ ] **Step 4: Make the minimal production contract change**

In `src/composables/useUpdater.ts`, change the candidate interface to:

```ts
export interface UpdateCandidate {
  version: string;
  download(
    onEvent?: (event: UpdaterDownloadEvent) => void
  ): Promise<void>;
  install(): Promise<void>;
  close(): Promise<void>;
}
```

In `activate`, replace the install call with:

```ts
stage = "install";
state.value = { phase: "installing", version };
await update.install();
```

Do not remove the later `runtime.relaunch()` call and do not add a JavaScript installer option.

- [ ] **Step 5: Correct every executable `restartAfterInstall` snippet in the earlier plan**

In `docs/superpowers/plans/2026-07-28-automatic-updates.md`:

- rename the unit-test example to the Task 1 test name;
- use `expect(vi.mocked(update.candidate.install).mock.calls).toEqual([[]]);`;
- declare `install(): Promise<void>;`;
- call `await update.install();`;
- use `expect(update.install.mock.calls).toEqual([[]]);` in the App integration example;
- add one sentence beside the implementation snippet: `Windows installer presentation is configured by plugins.updater.windows.installMode in tauri.conf.json, not by an install() argument.`

Run:

```bash
rg -n "restartAfterInstall" src/composables src/__tests__ docs/superpowers/plans/2026-07-28-automatic-updates.md
```

Expected: no output and exit status `1`. Mentions in the approved design remain valid historical/problem descriptions and are outside this check.

- [ ] **Step 6: Run focused and full verification**

Run:

```bash
pnpm test -- src/__tests__/useUpdater.spec.ts src/__tests__/AppUpdater.spec.ts
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: every command exits `0`; no test count changes in this task.

- [ ] **Step 7: Review and commit the contract correction**

Run:

```bash
git diff -- src/composables/useUpdater.ts src/__tests__/useUpdater.spec.ts src/__tests__/AppUpdater.spec.ts docs/superpowers/plans/2026-07-28-automatic-updates.md
git add src/composables/useUpdater.ts src/__tests__/useUpdater.spec.ts src/__tests__/AppUpdater.spec.ts docs/superpowers/plans/2026-07-28-automatic-updates.md
git diff --cached --check
git commit -m "fix: align updater install contract"
```

Expected: the commit contains only the four listed files.

---

### Task 2: Invalidate stopped updater work by lifecycle ownership

**Files:**

- Modify: `src/__tests__/useUpdater.spec.ts:52-285`
- Modify: `src/composables/useUpdater.ts:63-211`
- Modify: `docs/superpowers/plans/2026-07-28-automatic-updates.md:313-321,756-901`

**Interfaces:**

- Consumes: `useUpdater(runtime?) -> { state, start, stop, activate }` from Task 1.
- Produces: private `lifecycleGeneration: number`.
- Produces: private `operationToken: symbol | undefined`.
- Produces: `ownsOperation(generation: number, token: symbol): boolean`.
- Preserves: the public return type and all existing updater phases/failure stages.

- [ ] **Step 1: Add the five stop/restart regression tests**

Add these tests to `src/__tests__/useUpdater.spec.ts`:

```ts
it("stays idle and closes a late candidate when stopped during a check", async () => {
  const checkGate = deferred<UpdateCandidate | null>();
  const update = makeCandidate();
  const check = vi
    .fn<UpdaterRuntime["check"]>()
    .mockReturnValue(checkGate.promise);
  const updater = useUpdater(makeRuntime({ check }));

  updater.start();
  await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(1));
  updater.stop();

  expect(updater.state.value).toEqual({ phase: "idle" });
  checkGate.resolve(update.candidate);
  await vi.waitFor(() =>
    expect(update.candidate.close).toHaveBeenCalledTimes(1)
  );

  expect(update.candidate.download).not.toHaveBeenCalled();
  expect(updater.state.value).toEqual({ phase: "idle" });
});

it("ignores download progress and completion after stop", async () => {
  const downloadGate = deferred<void>();
  const update = makeCandidate("0.4.0", downloadGate);
  const updater = useUpdater(
    makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
  );

  updater.start();
  await vi.waitFor(() =>
    expect(updater.state.value.phase).toBe("downloading")
  );
  updater.stop();

  expect(updater.state.value).toEqual({ phase: "idle" });
  update.emit({ event: "Started", data: { contentLength: 200 } });
  update.emit({ event: "Progress", data: { chunkLength: 84 } });
  expect(updater.state.value).toEqual({ phase: "idle" });

  downloadGate.resolve();
  await downloadGate.promise;
  await Promise.resolve();
  expect(update.candidate.close).toHaveBeenCalledTimes(1);
  expect(updater.state.value).toEqual({ phase: "idle" });
});

it("does not install when confirmation resolves after stop", async () => {
  const confirmationGate = deferred<boolean>();
  const update = makeCandidate();
  const runtime = makeRuntime({
    check: vi.fn().mockResolvedValue(update.candidate),
    confirm: vi.fn().mockReturnValue(confirmationGate.promise)
  });
  const updater = useUpdater(runtime);

  updater.start();
  await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
  const activation = updater.activate(confirmation);
  await vi.waitFor(() => expect(runtime.confirm).toHaveBeenCalledTimes(1));
  updater.stop();

  expect(updater.state.value).toEqual({ phase: "idle" });
  confirmationGate.resolve(true);
  await activation;

  expect(update.candidate.install).not.toHaveBeenCalled();
  expect(runtime.relaunch).not.toHaveBeenCalled();
  expect(updater.state.value).toEqual({ phase: "idle" });
});

it("keeps idle and suppresses relaunch when stopped during install", async () => {
  const installGate = deferred<void>();
  const update = makeCandidate();
  vi.mocked(update.candidate.install).mockReturnValue(installGate.promise);
  const runtime = makeRuntime({
    check: vi.fn().mockResolvedValue(update.candidate)
  });
  const updater = useUpdater(runtime);

  updater.start();
  await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
  const activation = updater.activate(confirmation);
  await vi.waitFor(() =>
    expect(update.candidate.install).toHaveBeenCalledTimes(1)
  );
  updater.stop();

  expect(updater.state.value).toEqual({ phase: "idle" });
  installGate.resolve();
  await activation;

  expect(runtime.relaunch).not.toHaveBeenCalled();
  expect(updater.state.value).toEqual({ phase: "idle" });
});

it("starts a fresh lifecycle while an old check is still pending", async () => {
  const firstCheck = deferred<UpdateCandidate | null>();
  const secondCheck = deferred<UpdateCandidate | null>();
  const stale = makeCandidate("0.4.0");
  const check = vi
    .fn<UpdaterRuntime["check"]>()
    .mockReturnValueOnce(firstCheck.promise)
    .mockReturnValueOnce(secondCheck.promise)
    .mockResolvedValue(null);
  const updater = useUpdater(makeRuntime({ check }));

  updater.start();
  await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(1));
  updater.stop();
  updater.start();
  await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(2));

  firstCheck.resolve(stale.candidate);
  await vi.waitFor(() =>
    expect(stale.candidate.close).toHaveBeenCalledTimes(1)
  );
  expect(stale.candidate.download).not.toHaveBeenCalled();

  await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
  expect(check).toHaveBeenCalledTimes(2);

  secondCheck.resolve(null);
  await vi.waitFor(() =>
    expect(updater.state.value).toEqual({ phase: "idle" })
  );
});
```

- [ ] **Step 2: Run the focused suite and verify RED**

Run:

```bash
pnpm test -- src/__tests__/useUpdater.spec.ts
```

Expected: the new tests fail because current `stop()` leaves visible state and `inFlight` ownership active, and stale continuations can still download, install, become ready, or relaunch.

- [ ] **Step 3: Replace boolean in-flight tracking with generation/token ownership**

In `src/composables/useUpdater.ts`, replace the private lifecycle variables and helper functions with:

```ts
let candidate: UpdateCandidate | undefined;
let timer: ReturnType<typeof setInterval> | undefined;
let started = false;
let lifecycleGeneration = 0;
let operationToken: symbol | undefined;

async function closeUpdate(update: UpdateCandidate | undefined) {
  if (!update) return;
  try {
    await update.close();
  } catch {
    // Closing a stale resource must not replace the current state.
  }
}

async function closeCandidate() {
  const stale = candidate;
  candidate = undefined;
  await closeUpdate(stale);
}

function ownsOperation(generation: number, token: symbol) {
  return (
    started &&
    lifecycleGeneration === generation &&
    operationToken === token
  );
}

function blocksCheck() {
  return (
    operationToken !== undefined ||
    state.value.phase === "downloading" ||
    state.value.phase === "ready" ||
    state.value.phase === "installing"
  );
}
```

- [ ] **Step 4: Make check/download continuations prove ownership**

Replace `checkForUpdate` with:

```ts
async function checkForUpdate() {
  if (!started || !runtime.enabled || blocksCheck()) return;
  const generation = lifecycleGeneration;
  const token = Symbol();
  operationToken = token;
  let stage: UpdaterFailureStage = "check";
  state.value = { phase: "checking" };

  try {
    const update = await runtime.check();
    if (!ownsOperation(generation, token)) {
      await closeUpdate(update ?? undefined);
      return;
    }
    if (!update) {
      state.value = { phase: "idle" };
      return;
    }

    candidate = update;
    stage = "download";
    let downloaded = 0;
    let total: number | undefined;
    state.value = {
      phase: "downloading",
      version: update.version,
      downloaded
    };

    await update.download((event) => {
      if (!ownsOperation(generation, token)) return;

      if (event.event === "Started") {
        downloaded = 0;
        total =
          event.data.contentLength && event.data.contentLength > 0
            ? event.data.contentLength
            : undefined;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
      }

      if (event.event !== "Finished") {
        const percent = total
          ? Math.min(
              100,
              Math.max(0, Math.floor((downloaded / total) * 100))
            )
          : undefined;
        state.value = {
          phase: "downloading",
          version: update.version,
          downloaded,
          ...(total === undefined ? {} : { total }),
          ...(percent === undefined ? {} : { percent })
        };
      }
    });

    if (!ownsOperation(generation, token)) return;
    state.value = { phase: "ready", version: update.version };
  } catch {
    if (!ownsOperation(generation, token)) return;
    await closeCandidate();
    if (!ownsOperation(generation, token)) return;
    state.value = { phase: "failed", stage };
  } finally {
    if (operationToken === token) operationToken = undefined;
  }
}
```

The stale-check branch closes the candidate returned by that stale check directly; it must not call `closeCandidate()` because a newer lifecycle may already own the global candidate.

- [ ] **Step 5: Make confirmation/install/relaunch continuations prove ownership**

Replace `activate` with:

```ts
async function activate(copy: UpdateConfirmationCopy) {
  if (state.value.phase === "failed") {
    await checkForUpdate();
    return;
  }
  if (
    !started ||
    state.value.phase !== "ready" ||
    !candidate ||
    operationToken !== undefined
  ) {
    return;
  }

  const update = candidate;
  const version = state.value.version;
  const generation = lifecycleGeneration;
  const token = Symbol();
  operationToken = token;
  let stage: UpdaterFailureStage = "confirm";

  try {
    const accepted = await runtime.confirm(copy.message, {
      title: copy.title,
      kind: "info"
    });
    if (!ownsOperation(generation, token) || !accepted) return;

    stage = "install";
    state.value = { phase: "installing", version };
    await update.install();
    if (!ownsOperation(generation, token)) return;

    stage = "relaunch";
    await closeCandidate();
    if (!ownsOperation(generation, token)) return;
    await runtime.relaunch();
  } catch {
    if (!ownsOperation(generation, token)) return;
    await closeCandidate();
    if (!ownsOperation(generation, token)) return;
    state.value = { phase: "failed", stage };
  } finally {
    if (operationToken === token) operationToken = undefined;
  }
}
```

- [ ] **Step 6: Make start and stop establish explicit lifecycle boundaries**

Replace `start` and `stop` with:

```ts
function start() {
  if (started || !runtime.enabled) return;
  started = true;
  lifecycleGeneration += 1;
  void checkForUpdate();
  timer = setInterval(() => {
    void checkForUpdate();
  }, UPDATE_CHECK_INTERVAL_MS);
}

function stop() {
  started = false;
  lifecycleGeneration += 1;
  if (timer) clearInterval(timer);
  timer = undefined;
  operationToken = undefined;
  state.value = { phase: "idle" };
  void closeCandidate();
}
```

Do not await from `stop()` and do not expose the ownership values through the composable's public return object.

- [ ] **Step 7: Update the earlier plan's lifecycle contract and copied implementation**

In `docs/superpowers/plans/2026-07-28-automatic-updates.md`:

- replace the old `stop()` interface bullet with: `` `stop()` invalidates the active lifecycle, clears the timer, returns state to `idle`, and closes any retained `UpdateCandidate`. ``;
- in the copied `useUpdater.ts` implementation, replace `inFlight` with `lifecycleGeneration` and `operationToken`;
- include `closeUpdate`, `ownsOperation`, and the exact `blocksCheck`, `checkForUpdate`, `activate`, `start`, and `stop` implementations from Steps 3-6;
- retain the zero-argument `install()` contract from Task 1.

Run:

```bash
rg -n "inFlight|restartAfterInstall" src/composables/useUpdater.ts docs/superpowers/plans/2026-07-28-automatic-updates.md
```

Expected: no output and exit status `1`.

- [ ] **Step 8: Run focused and full verification**

Run:

```bash
pnpm test -- src/__tests__/useUpdater.spec.ts src/__tests__/AppUpdater.spec.ts
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: every command exits `0`; the frontend total is now 25 files and 135 tests.

- [ ] **Step 9: Review and commit lifecycle ownership**

Run:

```bash
git diff -- src/composables/useUpdater.ts src/__tests__/useUpdater.spec.ts docs/superpowers/plans/2026-07-28-automatic-updates.md
git add src/composables/useUpdater.ts src/__tests__/useUpdater.spec.ts docs/superpowers/plans/2026-07-28-automatic-updates.md
git diff --cached --check
git commit -m "fix: invalidate stopped updater work"
```

Expected: the commit contains only the three listed files and all five new regressions.

---

### Task 3: Prove native updater-related plugin registration through IPC

**Files:**

- Modify: `src-tauri/Cargo.toml:40-42`
- Modify: `src-tauri/src/app.rs:1-50`
- Create: `src-tauri/tests/updater_plugin_registration.rs`
- Modify: `docs/superpowers/plans/2026-07-28-updater-review-hardening.md:12-45,606-740`

**Interfaces:**

- Consumes: `tauri::Builder<R>` for any `R: tauri::Runtime`.
- Consumes: the existing Tauri 2 dependency with its empty `test` feature enabled only for dev/test targets.
- Produces: `#[doc(hidden)] pub register_updater_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R>` for production and the external integration-test crate.
- Registers exactly: dialog, process, and updater plugins in their existing order.
- Probes: `plugin:dialog|message`, `plugin:process|exit`, and `plugin:updater|download`.

- [ ] **Step 1: Add a failing MockRuntime IPC registration integration test**

Create `src-tauri/tests/updater_plugin_registration.rs` and add:

```rust
use tauri::{
    ipc::CallbackFn,
    test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
    utils::acl::ExecutionContext,
    webview::InvokeRequest,
    LogicalUnit, PixelUnit, WebviewWindowBuilder,
};

use codex_pulse::app::register_updater_plugins;

#[test]
fn production_builder_registers_updater_plugin_commands() {
    const COMMANDS: [&str; 3] = [
        "plugin:dialog|message",
        "plugin:process|exit",
        "plugin:updater|download",
    ];

    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .plugins
        .0
        .insert("updater".into(), serde_json::json!({ "pubkey": "" }));
    for command in COMMANDS {
        context
            .runtime_authority_mut()
            .__allow_command(command.into(), ExecutionContext::Local);
    }
    let app = register_updater_plugins(mock_builder())
        .build(context)
        .expect("mock app should build with production updater plugins");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview should build");
    let local_url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };

    for command in COMMANDS {
        let response = get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: local_url.parse().expect("local invoke URL should parse"),
                body: serde_json::json!({}).into(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        );
        let error = response.expect_err("malformed command arguments should fail");
        let message = error
            .as_str()
            .expect("IPC rejection should be a string message");

        assert!(
            message.contains("missing required key") || message.contains("invalid args"),
            "registered malformed command should reject its arguments, got: {message}"
        );
        assert!(
            !message.contains("not found"),
            "production plugin command was not registered: {message}"
        );
    }
}
```

The minimal mock updater config is required because Tauri's default
`mock_context()` represents plugin configuration as `null`, while the updater
plugin requires an object with `pubkey`. Its empty endpoint list cannot contact
the network. The same mock context starts with `Resolved::default()`, so the
test explicitly allows only these three local command strings through
`runtime_authority_mut().__allow_command`. Each IPC body then omits `message`,
`code`, and `rid`/`onEvent`, so the commands must fail during argument decoding
before opening a dialog, exiting, or contacting an updater endpoint. If a
plugin registration is removed, the allowlist remains but the plugin manager
returns command-not-found.

- [ ] **Step 2: Run the focused Rust test and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test updater_plugin_registration production_builder_registers_updater_plugin_commands -- --exact
```

Expected on the first run: compilation reports that `tauri::test` is gated
behind `feature = "test"` and that `register_updater_plugins` does not exist.
This proves both the missing test-only feature and the intended production
registration gap. Do not weaken the test to register plugins directly inside
the test.

- [ ] **Step 3: Enable Tauri MockRuntime only for dev/test targets and reconfirm RED**

Add to `[dev-dependencies]` in `src-tauri/Cargo.toml`:

```toml
tauri = { version = "2", features = ["test"] }
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test updater_plugin_registration production_builder_registers_updater_plugin_commands -- --exact
```

Expected: the `tauri::test` gate error is gone and compilation now fails only
because `register_updater_plugins` does not exist. The existing normal
dependency's version/features remain unchanged, and Cargo adds no new crate.

- [ ] **Step 4: Extract the generic production registration helper**

Add above `run()`; the hidden public visibility is required because Cargo
compiles `tests/` as a separate crate:

```rust
#[doc(hidden)]
pub fn register_updater_plugins<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
}
```

Start production `run()` through the same helper while retaining opener before it and liquid-glass/single-instance after it:

```rust
pub fn run() -> anyhow::Result<()> {
    register_updater_plugins(
        tauri::Builder::default().plugin(tauri_plugin_opener::init()),
    )
    .plugin(tauri_plugin_liquid_glass::init())
    .plugin(tauri_plugin_single_instance::init(|app, _, _| {
        crate::tray::show_main_window(app);
    }))
```

Leave the existing `.manage`, `.setup`, `.invoke_handler`, `.run`, and error mapping unchanged.

- [ ] **Step 5: Run focused and full verification**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --test updater_plugin_registration production_builder_registers_updater_plugin_commands -- --exact
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected:

- the focused IPC regression passes on all three commands;
- frontend remains 25 files / 135 tests;
- Rust reports 81 library unit tests plus the updater registration integration test, with the existing auxiliary and doc-test targets also passing;
- every command exits `0`.

- [ ] **Step 6: Review and commit the native regression**

Run:

```bash
git diff -- src-tauri/Cargo.toml src-tauri/src/app.rs src-tauri/tests/updater_plugin_registration.rs docs/superpowers/plans/2026-07-28-updater-review-hardening.md
git add src-tauri/Cargo.toml src-tauri/src/app.rs src-tauri/tests/updater_plugin_registration.rs docs/superpowers/plans/2026-07-28-updater-review-hardening.md
git diff --cached --check
git commit -m "test: cover native updater plugin registration"
git rev-parse HEAD
```

Expected: the commit contains only the test-only feature, shared
registration/test, and approved plan correction. Preserve the full
40-character `git rev-parse HEAD` output for the acceptance addendum; this is
the verified implementation commit, not the later evidence-only commit.

---

### Task 4: Record fresh review-hardening evidence without rewriting history

**Files:**

- Modify: `docs/superpowers/reports/automatic-updates-acceptance.md:1-130`

**Interfaces:**

- Consumes: the exact Task 3 implementation commit and fresh command output.
- Produces: `## PR Review Follow-up Verification`.
- Preserves: the original `cfc0e9d5cbd9544f4f378cc3d0574034eba92b0b` 24-file / 122-test table verbatim.

- [ ] **Step 1: Verify the implementation commit from a clean tree**

Run:

```bash
git status --short --branch
git show --stat --oneline HEAD
git rev-parse HEAD
```

Expected: detached clean status; HEAD is `test: cover native updater plugin registration`; record the full hash printed by `git rev-parse HEAD`.

- [ ] **Step 2: Run one fresh full verification pass on that exact implementation commit**

Run:

```bash
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected:

- Vitest: 25 files, 135 tests, 0 failed;
- production build: `vue-tsc --noEmit` and Vite pass;
- Rust: 82 unit tests, 0 failed, with auxiliary/doc-test targets passing;
- formatting and diff checks: no differences/errors.

If an observed count differs, diagnose it before editing the report; never copy the expected number over contradictory command output.

- [ ] **Step 3: Append the follow-up section with the observed implementation hash**

Append `## PR Review Follow-up Verification` after `## Fresh Automated Verification` and before `## Signed Tauri Boundary`. The section must state, in complete prose and a command/result table:

- the original `cfc0e9d` / 122-test row is intentionally preserved as point-in-time evidence;
- the full 40-character implementation commit printed in Step 1;
- `pnpm test`: 25 files, 135 tests, 0 failed;
- `pnpm build`: type-check and Vite production build passed;
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`: no formatting differences;
- `cargo test --manifest-path src-tauri/Cargo.toml`: 82 Rust unit tests, 0 failed, auxiliary/doc-test targets passed;
- `git diff --check`: exit `0`;
- local verification proves source/test/build behavior only;
- GitHub Actions is a separate post-push gate;
- native old-version-to-new-version installation, restart, Windows interaction, publication, and post-publication update checks remain unverified.

Use the exact observed implementation hash; do not cite the evidence-only commit as if it had verified itself.

- [ ] **Step 4: Check that historical evidence was not rewritten**

Run:

```bash
git diff -- docs/superpowers/reports/automatic-updates-acceptance.md
rg -n "cfc0e9d5cbd9544f4f378cc3d0574034eba92b0b|122 tests passed|PR Review Follow-up Verification|135 tests|82 Rust" docs/superpowers/reports/automatic-updates-acceptance.md
git diff --check
```

Expected: the diff only adds the follow-up section; the original hash and 122-test observation remain present.

- [ ] **Step 5: Commit the evidence-only change**

Run:

```bash
git add docs/superpowers/reports/automatic-updates-acceptance.md
git diff --cached --check
git commit -m "docs: record updater review verification"
```

Expected: one-file documentation commit.

---

### Task 5: Fast-forward the existing PR branch and verify CI

**Files:**

- No repository file changes.
- Update: remote branch `codex/local-updater-signing-runbook`.
- Verify: PR #17 GitHub Actions jobs.

**Interfaces:**

- Consumes: detached clean HEAD containing Tasks 1-4.
- Produces: a normal fast-forward update of the existing PR branch.
- Produces: terminal evidence for `Frontend`, `Rust`, and `Rust (Windows)`.

- [ ] **Step 1: Fetch and prove the push is a fast-forward**

Run:

```bash
git fetch origin codex/local-updater-signing-runbook
git status --short --branch
git rev-list --left-right --count origin/codex/local-updater-signing-runbook...HEAD
```

Expected: detached clean status and a count whose left side is `0` and right side is greater than `0`. Any nonzero left side means the remote contains unintegrated work; stop and inspect rather than force-push.

- [ ] **Step 2: Push detached HEAD to the existing PR branch**

Run:

```bash
git push origin HEAD:refs/heads/codex/local-updater-signing-runbook
```

Expected: normal fast-forward push succeeds. Do not use `--force` or create a new ref.

- [ ] **Step 3: Resolve the new CI run for the pushed HEAD**

Run:

```bash
git rev-parse HEAD
gh run list --workflow CI --branch codex/local-updater-signing-runbook --limit 3 --json databaseId,headSha,status,conclusion,url
```

Expected: the newest pull-request run has `headSha` equal to local HEAD. Do not reuse run `30340437172`, which belongs to earlier commit `02b165a`.

- [ ] **Step 4: Wait for the matching run and inspect every job**

Set a task-specific shell variable from the matching run and wait for it:

```bash
UPDATER_REVIEW_RUN_ID="$(gh run list --workflow CI --branch codex/local-updater-signing-runbook --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run watch "$UPDATER_REVIEW_RUN_ID" --exit-status
gh run view "$UPDATER_REVIEW_RUN_ID" --json headSha,status,conclusion,url,jobs
```

Expected: conclusion `success`; `Frontend`, `Rust`, and `Rust (Windows)` all complete successfully. Confirm the Windows job includes frontend tests/build, Rust tests, NSIS build with `--no-sign`, and package verification. Report CodeRabbit separately; a rate-limited CodeRabbit review is not a GitHub Actions failure.

---

### Task 5A: Give the Windows Rust test harness its required Common Controls manifest

**Files:**

- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/src/app.rs`
- Create: `src-tauri/tests/updater_plugin_registration.rs`
- Modify: `docs/superpowers/plans/2026-07-28-updater-review-hardening.md`
- Modify after green CI: `docs/superpowers/reports/automatic-updates-acceptance.md`

**Interfaces:**

- Consumes: Cargo's `cargo:rustc-link-arg-tests` build-script directive.
- Produces on Windows integration-test targets only: an embedded `RT_MANIFEST` selecting `Microsoft.Windows.Common-Controls` version `6.0.0.0`.
- Preserves: the Windows execution of `production_builder_registers_updater_plugin_commands`, the production binary's Tauri-generated resource, and all non-Windows linker behavior.

- [ ] **Step 1: Preserve the Windows loader failure and identify its exact import boundary**

The first pushed run, `30347196303` at `fcc968f1bf95485ca6a0bdbbb1c36b5919eb5e6b`, compiled the Windows test executable but failed to launch it with `0xc0000139 / STATUS_ENTRYPOINT_NOT_FOUND`.

Cross-compile the current library test harness without running it and inspect the PE:

```bash
cargo xwin test --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc --no-run
llvm-readobj --coff-imports <codex_pulse-test.exe>
llvm-readobj --coff-resources <codex_pulse-test.exe>
```

Expected RED evidence:

- imports contain `comctl32.dll!TaskDialogIndirect`;
- resources are empty;
- Microsoft documents `TaskDialogIndirect` as a Common Controls v6 export and documents that applications without a selecting manifest use v5 by default.

- [ ] **Step 2: Move the IPC probe to an integration target and add the narrow linker contract**

Move `production_builder_registers_updater_plugin_commands` from the
`src-tauri/src/app.rs` unit-test module into
`src-tauri/tests/updater_plugin_registration.rs`. Mark the shared production
helper `#[doc(hidden)] pub` so the separate integration-test crate can use the
same registration path.

After `tauri_build::build()` in `src-tauri/build.rs`, check `CARGO_CFG_TARGET_OS` and emit:

```rust
if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' \
         name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
         processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}
```

Do not add a crate, alter Tauri's production manifest, or skip the IPC regression on Windows.

- [ ] **Step 3: Prove the resulting test PE contains a valid manifest**

Repeat the Windows MSVC cross-compile. Inspect both the explicit
`updater_plugin_registration` integration-test executable and the library
unit-test executable.

Expected GREEN evidence:

- the integration-test executable's resource type `MANIFEST` / ID `1` contains an `assemblyIdentity` for `Microsoft.Windows.Common-Controls` version `6.0.0.0`;
- the library unit-test executable no longer imports `TaskDialogIndirect`, so it does not require that manifest;
- the production binary retains only Tauri's existing resource and does not receive a duplicate manifest.

- [ ] **Step 4: Run local verification and commit the repair**

Run:

```bash
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: frontend remains 25 files / 135 tests; Rust remains 82 tests in
total (81 library unit tests plus one updater registration integration test);
all commands exit `0`.

Commit the build-script and plan correction together:

```bash
git add src-tauri/build.rs src-tauri/src/app.rs src-tauri/tests/updater_plugin_registration.rs docs/superpowers/plans/2026-07-28-updater-review-hardening.md
git diff --cached --check
git commit -m "fix: manifest Windows updater plugin tests"
```

- [ ] **Step 5: Fast-forward the existing PR branch and require a new green run**

Repeat Task 5's ancestry proof and normal detached-HEAD push. Wait for the new run whose `headSha` exactly equals the repair commit. Require all three jobs, including Windows Rust tests, NSIS build, and package verification, to pass.

- [ ] **Step 6: Append the CI correction to the acceptance report**

Add a separate dated row or paragraph beneath the existing PR review follow-up evidence. Preserve the earlier local results and explicitly record:

- failed run `30347196303` and its loader boundary;
- the final manifest-fix commit and matching successful run;
- the PE manifest inspection;
- that no signed updater/release/manual installed-app flow was exercised.

Commit this as evidence-only documentation and push it normally. If that documentation-only push triggers another CI run, require that newest SHA to pass before replying to review threads.

---

### Task 6: Reply to all inline review threads and close the audit loop

**Files:**

- No repository file changes.
- Update: eight existing PR #17 inline comment threads.

**Interfaces:**

- Consumes: the three implementation commit hashes, evidence commit hash, and successful latest CI run.
- Produces: one reply under each inline comment ID.
- Preserves: no top-level summary comment solely for the filename nitpick.

- [ ] **Step 1: Resolve the exact commit hashes from subjects**

Run:

```bash
UPDATER_INSTALL_COMMIT="$(git log --format='%H' --grep='^fix: align updater install contract$' -1)"
UPDATER_LIFECYCLE_COMMIT="$(git log --format='%H' --grep='^fix: invalidate stopped updater work$' -1)"
UPDATER_NATIVE_COMMIT="$(git log --format='%H' --grep='^test: cover native updater plugin registration$' -1)"
UPDATER_EVIDENCE_COMMIT="$(git log --format='%H' --grep='^docs: record updater review verification$' -1)"
printf '%s\n' "$UPDATER_INSTALL_COMMIT" "$UPDATER_LIFECYCLE_COMMIT" "$UPDATER_NATIVE_COMMIT" "$UPDATER_EVIDENCE_COMMIT"
```

Expected: one full hash for each exact subject. Use those literal hashes in the replies; do not cite HEAD generically.

- [ ] **Step 2: Reply to the acceptance-evidence and native-plugin threads**

Send each reply through its original thread:

```bash
UPDATER_NATIVE_COMMIT="$(git log --format='%H' --grep='^test: cover native updater plugin registration$' -1)"
UPDATER_EVIDENCE_COMMIT="$(git log --format='%H' --grep='^docs: record updater review verification$' -1)"
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713888/replies -f body="The original cfc0e9d / 122-test row is point-in-time evidence, so I preserved it instead of rewriting history. Evidence commit ${UPDATER_EVIDENCE_COMMIT} adds a separate PR Review Follow-up Verification section tied to implementation commit ${UPDATER_NATIVE_COMMIT}; it records the fresh 25-file / 135-test frontend result, 82 Rust tests, build/format checks, and the local/CI/manual evidence boundaries."
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713895/replies -f body="Resolved in ${UPDATER_NATIVE_COMMIT}. Production run() and the MockRuntime test now share register_updater_plugins<R: Runtime>. Malformed IPC probes for plugin:dialog|message, plugin:process|exit, and plugin:updater|download must reach argument decoding and must not return command-not-found."
```

Expected: both bodies contain literal 40-character hashes resolved from the local history.

- [ ] **Step 3: Reply to the two already-resolved Windows stability threads**

Run:

```bash
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713902/replies -f body='Resolved in 30ec6eaa33d7addd0f46ea6e9e92a2ce73ccd0cc. The non-production App test no longer advances the six-hour timer; it verifies the runtime gate directly. The latest PR Frontend and Rust (Windows) jobs both pass.'
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713948/replies -f body='Resolved in 30ec6eaa33d7addd0f46ea6e9e92a2ce73ccd0cc. The macOS shell runbook suite is now gated by process.platform !== "win32", while the cross-platform workflow/configuration suites still run on Windows. The latest Rust (Windows) job passes through NSIS package verification.'
```

Expected: both API calls create replies with `in_reply_to_id` matching the original comment.

- [ ] **Step 4: Reply to both zero-argument install threads**

```bash
UPDATER_INSTALL_COMMIT="$(git log --format='%H' --grep='^fix: align updater install contract$' -1)"
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713927/replies -f body="Resolved in ${UPDATER_INSTALL_COMMIT}. The custom candidate interface and production call now match @tauri-apps/plugin-updater 2.10.1 install(): Promise<void>; unit and App integration tests assert one empty argument list. Windows install presentation remains in tauri.conf.json installMode."
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713957/replies -f body="Resolved in ${UPDATER_INSTALL_COMMIT}. activate() now calls await update.install() with zero arguments, then explicitly closes the candidate and calls the process plugin relaunch while lifecycle ownership is current."
```

Expected: both bodies contain the same literal 40-character contract commit hash.

- [ ] **Step 5: Reply to the semicolon false positive without changing code**

Run:

```bash
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713954/replies -f body='No code change needed after source verification. The TypeScript statements in updaterManifest.spec.ts are already semicolon-terminated; commas in that hunk separate object properties and array elements, matching the repository style. Mechanical reformatting would not address an actual violation.'
```

Expected: reply created; `src/__tests__/updaterManifest.spec.ts` remains unchanged.

- [ ] **Step 6: Reply to the lifecycle invalidation thread**

```bash
UPDATER_LIFECYCLE_COMMIT="$(git log --format='%H' --grep='^fix: invalidate stopped updater work$' -1)"
gh api repos/qwertyerge/codex-pulse/pulls/17/comments/3663713964/replies -f body="Resolved in ${UPDATER_LIFECYCLE_COMMIT}. The composable now pairs a lifecycleGeneration with a unique operationToken and checks ownership after every await and download callback. New regressions cover stop during check, download, confirmation, and installation plus stop/start while the stale check and finally are still pending; stale work cannot publish state, install, close a newer candidate, or relaunch."
```

Expected: the body contains the literal 40-character lifecycle commit hash.

- [ ] **Step 7: Verify all eight replies in place**

Run:

```bash
gh api repos/qwertyerge/codex-pulse/pulls/17/comments --paginate --jq '.[] | select(.in_reply_to_id == 3663713888 or .in_reply_to_id == 3663713895 or .in_reply_to_id == 3663713902 or .in_reply_to_id == 3663713927 or .in_reply_to_id == 3663713948 or .in_reply_to_id == 3663713954 or .in_reply_to_id == 3663713957 or .in_reply_to_id == 3663713964) | [.in_reply_to_id, .id, .user.login, .body] | @json'
```

Expected: exactly eight replies by the authenticated repository user, one for each original ID. The review-body-only `localUpdaterBuild.spec.ts` filename nitpick has no inline thread; record its evidence-based non-adoption in the final report and do not add a generic top-level comment.

- [ ] **Step 8: Perform final read-only PR and repository verification**

Run:

```bash
gh pr checks 17
gh pr view 17 --json number,state,isDraft,mergeable,headRefName,headRefOid,baseRefName,url
git status --short --branch
git rev-parse HEAD
git ls-remote origin refs/heads/codex/local-updater-signing-runbook
```

Expected:

- PR #17 remains open, non-Draft, and targets `main`;
- head ref is `codex/local-updater-signing-runbook`;
- GitHub Actions checks are successful;
- local detached HEAD equals the remote ref;
- the worktree is clean;
- no merge, tag, release, or installed-app replacement occurred.
