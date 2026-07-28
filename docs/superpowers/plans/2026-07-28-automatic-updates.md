# Automatic Updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add signed, enabled-by-default automatic updates for macOS ARM64 and Windows 11 x64, with background download, a compact restart-confirmation badge, and a Draft-gated GitHub release manifest.

**Architecture:** A frontend `useUpdater` state machine owns Tauri's in-process `Update` resource and keeps update failures independent from session monitoring. Official Tauri updater, dialog, and process plugins provide the native boundary; the existing serialized release matrix signs and merges a static `latest.json`, then a separate job validates both supported platforms.

**Tech Stack:** Vue 3 Composition API, TypeScript 5.9, Vue I18n 11, Vitest 4, Tauri 2.11, `tauri-plugin-updater` 2.10.1, `tauri-plugin-dialog` 2.7.2, `tauri-plugin-process` 2.3.1, Rust 1.82, pnpm 10.33.0, GitHub Actions, macOS Keychain.

## Global Constraints

- Keep `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` at version `0.3.2`.
- Support only macOS Apple Silicon and Windows 11 x64 updater artifacts.
- Use `https://github.com/qwertyerge/codex-pulse/releases/latest/download/latest.json` as the static endpoint.
- Check after the first `pulse.load()` settles and every `21_600_000` milliseconds thereafter.
- Enable network checks only in a production Tauri runtime; development, browser-only, and Vitest runs make no update request.
- Automatically download and verify a newer artifact, then require native OK/Cancel confirmation before install and restart.
- Keep update state independent from `AppSnapshot`, `usePulse`, and `pulse.error`.
- Keep only one updater transition in flight; do not recheck while downloading, ready, or installing.
- Do not persist an update download; a process exit requires a fresh check and download.
- Show no badge while idle, checking, or current; replace the active count while downloading, ready, installing, or failed.
- Keep the full `Codex Pulse` name at 320 pixels; the waveform mark may hide only in the narrow updating layout.
- Localize visible text, title, and ARIA text in Simplified Chinese, English, French, and German.
- Grant only `updater:allow-check`, `updater:allow-download`, `updater:allow-install`, `dialog:allow-message`, and `process:allow-restart` in addition to `core:default`.
- Store the encrypted private key only at `/Users/loki/.tauri/codex-pulse-updater.key` with mode `0600`.
- Store its random passphrase only in macOS Keychain service `Codex Pulse Updater Signing`, account `qwertyerge/codex-pulse`.
- Configure GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` without printing either value.
- Keep `releaseDraft: true`, `max-parallel: 1`, and manual publication.
- Require updater-plugin and manifest signatures; do not treat them as Apple notarization or Windows Authenticode.
- Do not push, create a pull request, change version, tag, create a Draft, publish, or mutate `main`.
- Preserve the Codex-managed detached worktree. Commit checkpoints on detached HEAD and report that provenance.
- Treat local tests, local signed macOS artifacts, GitHub Secrets, future Draft assets, interactive platform proof, and publication as separate evidence.
- Test executable behavior through the real state machine, mounted components, and manifest process; mock only Tauri network/IPC/native-dialog boundaries.
- Treat parsed Tauri/GitHub declarations as the approved configuration-class TDD exception, and review human prose semantically instead of adding exact-copy tests.

---

## File Map

| File | Responsibility |
| --- | --- |
| `package.json`, `pnpm-lock.yaml` | JavaScript bindings for updater, dialog, and process plugins. |
| `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` | Native plugin crates at the approved compatible versions. |
| `src-tauri/src/app.rs` | Register the three official Tauri plugins. |
| `src-tauri/tauri.conf.json` | Create signed updater artifacts and declare public key, endpoint, and passive Windows install. |
| `src-tauri/capabilities/default.json` | Grant the five precise updater/dialog/process permissions. |
| `src/__tests__/updaterConfiguration.spec.ts` | Contract-test dependencies, native registration, configuration, and permissions. |
| `src/composables/useUpdater.ts` | Own the update state machine, resource lifetime, schedule, download, confirmation, install, and relaunch. |
| `src/__tests__/useUpdater.spec.ts` | Prove state transitions, progress, scheduling, single-flight behavior, retry, and cleanup. |
| `src/components/TopBar.vue` | Render the localized update badge and emit its one activation event. |
| `src/i18n.ts` | Hold update badge and native-confirmation copy in all four locales. |
| `src/styles.css` | Style light/dark badge states and preserve the 320-pixel layout. |
| `src/__tests__/TopBar.spec.ts` | Test badge text, visibility, disabled state, accessibility, and activation. |
| `src/__tests__/topBarLayout.spec.ts` | Lock the active-count replacement and narrow updating layout. |
| `src/__tests__/i18n.spec.ts` | Require the updater key set to be complete and non-empty in every locale. |
| `src/App.vue` | Start and stop the updater and pass localized confirmation text. |
| `src/__tests__/AppUpdater.spec.ts` | Mount the real App/updater and prove production timing, teardown, and the non-production network gate. |
| `scripts/verify-updater-manifest.mjs` | Validate an authenticated Draft `latest.json` without trusting shell string matches. |
| `src/__tests__/updaterManifest.spec.ts` | Test valid and invalid updater manifests. |
| `.github/workflows/release.yml` | Sign both builds, upload signatures/manifest, and run the validation job. |
| `src/__tests__/githubWorkflows.spec.ts` | Lock signing inputs, Draft behavior, serialized uploads, and the validation job. |
| `README.md`, `docs/README.zh-CN.md` | Correct release status and disclose updater network, privacy, bootstrap, and trust behavior. |
| `src/__tests__/githubCommunity.spec.ts` | Keep English and Chinese public updater claims aligned. |
| `docs/superpowers/reports/automatic-updates-acceptance.md` | Record fresh local evidence and explicitly pending release/runtime gates. |

---

### Task 1: Establish the signed Tauri updater boundary

**Files:**

- Create: `src/__tests__/updaterConfiguration.spec.ts`
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- External create: `/Users/loki/.tauri/codex-pulse-updater.key`
- External create: `/Users/loki/.tauri/codex-pulse-updater.key.pub`
- External create: macOS Keychain generic-password item

**Interfaces:**

- Produces: official Tauri updater, dialog, and process IPC commands.
- Produces: a committed updater public key and two uncommitted encrypted key files.
- Produces: signed updater artifacts when `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are present.
- Preserves: application version `0.3.2`, current bundle targets, and existing native plugins.

- [ ] **Step 1: Install the clean baseline and prove it is green**

Run:

```bash
pnpm install --frozen-lockfile
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
git status --short --branch
```

Expected: both suites PASS; status is detached and clean. If a baseline test
fails, record the exact failure and stop before adding updater changes.

- [ ] **Step 2: Write the failing configuration contract**

Create `src/__tests__/updaterConfiguration.spec.ts`:

```ts
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string) {
  return readFileSync(resolve(process.cwd(), path), "utf8");
}

describe("automatic updater configuration", () => {
  it("declares signed artifacts at the approved static endpoint", () => {
    const config = JSON.parse(read("src-tauri/tauri.conf.json")) as {
      version: string;
      bundle: { createUpdaterArtifacts?: boolean };
      plugins?: {
        updater?: {
          pubkey?: string;
          endpoints?: string[];
          windows?: { installMode?: string };
        };
      };
    };
    const updater = config.plugins?.updater;

    expect(config.version).toBe("0.3.2");
    expect(config.bundle.createUpdaterArtifacts).toBe(true);
    expect(updater?.endpoints).toEqual([
      "https://github.com/qwertyerge/codex-pulse/releases/latest/download/latest.json"
    ]);
    expect(updater?.windows).toEqual({ installMode: "passive" });
    expect(updater?.pubkey).toMatch(/^[A-Za-z0-9+/=]{100,}$/);
  });

  it("grants the precise updater surface to the main window", () => {
    const capability = JSON.parse(
      read("src-tauri/capabilities/default.json")
    ) as { permissions: string[] };

    expect(capability.permissions).toEqual([
      "core:default",
      "updater:allow-check",
      "updater:allow-download",
      "updater:allow-install",
      "dialog:allow-message",
      "process:allow-restart"
    ]);
    expect(capability.permissions).not.toContain("updater:default");
    expect(capability.permissions).not.toContain("dialog:default");
    expect(capability.permissions).not.toContain("process:default");
  });
});
```

- [ ] **Step 3: Run the contract and verify RED**

Run:

```bash
pnpm test -- src/__tests__/updaterConfiguration.spec.ts
```

Expected: FAIL because the updater configuration, public key, and permissions
do not exist. This parsed JSON
contract is the explicitly approved configuration-class TDD exception; native
plugin registration is verified by compilation and the signed bundle rather
than by grepping `app.rs`.

- [ ] **Step 4: Add the approved plugin dependencies**

Run:

```bash
pnpm add @tauri-apps/plugin-updater@^2.10.1 @tauri-apps/plugin-dialog@^2.7.2 @tauri-apps/plugin-process@^2.3.1
```

Add these exact direct dependencies to `src-tauri/Cargo.toml` beside the other
Tauri plugins:

```toml
tauri-plugin-dialog = "2.7.2"
tauri-plugin-process = "2.3.1"
tauri-plugin-updater = "2.10.1"
```

Run `cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1
--no-deps` once to resolve and update `src-tauri/Cargo.lock`. Do not change the
Rust version floor or any existing dependency.

- [ ] **Step 5: Generate the encrypted updater key without overwriting state**

First prove that neither the files nor the Keychain item already exist:

```bash
test ! -e /Users/loki/.tauri/codex-pulse-updater.key
test ! -e /Users/loki/.tauri/codex-pulse-updater.key.pub
! security find-generic-password -a qwertyerge/codex-pulse -s "Codex Pulse Updater Signing" >/dev/null 2>&1
```

Expected: all three checks succeed. If any check fails, stop and use AskHuman
before choosing whether to reuse, rotate, or replace existing signing state.

Run the following in one non-echoing shell. Do not enable `set -x`:

```bash
set -euo pipefail
UPDATER_KEY_DIR=/Users/loki/.tauri
UPDATER_KEY_PATH=/Users/loki/.tauri/codex-pulse-updater.key
UPDATER_KEYCHAIN_SERVICE="Codex Pulse Updater Signing"
UPDATER_KEYCHAIN_ACCOUNT=qwertyerge/codex-pulse
umask 077
mkdir -p "$UPDATER_KEY_DIR"
UPDATER_KEY_PASSWORD="$(openssl rand -base64 48)"
pnpm tauri signer generate --ci --password "$UPDATER_KEY_PASSWORD" --write-keys "$UPDATER_KEY_PATH"
chmod 600 "$UPDATER_KEY_PATH"
security add-generic-password -U -a "$UPDATER_KEYCHAIN_ACCOUNT" -s "$UPDATER_KEYCHAIN_SERVICE" -w "$UPDATER_KEY_PASSWORD"
unset UPDATER_KEY_PASSWORD
```

Verify metadata without printing either secret:

```bash
test -s /Users/loki/.tauri/codex-pulse-updater.key
test -s /Users/loki/.tauri/codex-pulse-updater.key.pub
test "$(stat -f '%Lp' /Users/loki/.tauri/codex-pulse-updater.key)" = "600"
security find-generic-password -a qwertyerge/codex-pulse -s "Codex Pulse Updater Signing" >/dev/null
```

- [ ] **Step 6: Register and configure the native boundary**

In `src-tauri/src/app.rs`, add these plugins to the existing builder without
changing the single-instance callback:

```rust
.plugin(tauri_plugin_dialog::init())
.plugin(tauri_plugin_process::init())
.plugin(tauri_plugin_updater::Builder::new().build())
```

Read the public file, trim its terminal newline, and use those exact bytes for
`plugins.updater.pubkey`. The public key may be displayed; never read the
private key for this edit.

In `src-tauri/tauri.conf.json`:

- add `"createUpdaterArtifacts": true` inside `bundle`;
- add a root `plugins.updater` object with that generated public key;
- set `endpoints` to the one approved GitHub URL; and
- set `windows.installMode` to `"passive"`.

Replace `src-tauri/capabilities/default.json` permissions with this exact list:

```json
[
  "core:default",
  "updater:allow-check",
  "updater:allow-download",
  "updater:allow-install",
  "dialog:allow-message",
  "process:allow-restart"
]
```

- [ ] **Step 7: Run the focused contract and native compile to verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/updaterConfiguration.spec.ts
cargo test --manifest-path src-tauri/Cargo.toml app::tests
pnpm build
```

Expected: the new contract PASSES, Rust compiles with all registered plugins,
and the frontend production build resolves all plugin bindings.

- [ ] **Step 8: Inspect and commit the signed boundary**

Run:

```bash
git diff --check
git status --short
git diff -- package.json src-tauri/Cargo.toml src-tauri/src/app.rs src-tauri/tauri.conf.json src-tauri/capabilities/default.json
if git ls-files | rg -q 'codex-pulse-updater\.key'; then
  echo "private updater key is tracked" >&2
  exit 1
fi
git add package.json pnpm-lock.yaml src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/app.rs src-tauri/tauri.conf.json src-tauri/capabilities/default.json src/__tests__/updaterConfiguration.spec.ts
git commit -m "feat: establish signed updater boundary"
```

Expected: only the public key is in the commit; neither key file is tracked.

---

### Task 2: Implement the updater state machine

**Files:**

- Create: `src/composables/useUpdater.ts`
- Create: `src/__tests__/useUpdater.spec.ts`

**Interfaces:**

- Produces: `UPDATE_CHECK_INTERVAL_MS = 21_600_000`.
- Produces: `UpdaterState`, `UpdaterFailureStage`, `UpdateCandidate`, `UpdaterRuntime`, and `UpdateConfirmationCopy`.
- Produces: `useUpdater(runtime?) -> { state, start, stop, activate }`.
- `start()` is idempotent, checks immediately, and owns the six-hour timer.
- `activate(copy)` retries a failed check or confirms and installs a ready update.
- `stop()` clears the timer and closes any retained `UpdateCandidate`.

- [ ] **Step 1: Write the state-machine test harness and lifecycle tests**

Create `src/__tests__/useUpdater.spec.ts` with these helpers:

```ts
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi
} from "vitest";

import {
  UPDATE_CHECK_INTERVAL_MS,
  useUpdater,
  type UpdateCandidate,
  type UpdaterDownloadEvent,
  type UpdaterRuntime
} from "../composables/useUpdater";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function makeCandidate(
  version = "0.4.0",
  downloadGate?: ReturnType<typeof deferred<void>>
) {
  let listener: ((event: UpdaterDownloadEvent) => void) | undefined;
  const candidate: UpdateCandidate = {
    version,
    download: vi.fn(async (onEvent) => {
      listener = onEvent;
      if (downloadGate) await downloadGate.promise;
    }),
    install: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined)
  };
  return {
    candidate,
    emit(event: UpdaterDownloadEvent) {
      if (!listener) throw new Error("download listener is not registered");
      listener(event);
    }
  };
}

function makeRuntime(
  overrides: Partial<UpdaterRuntime> = {}
): UpdaterRuntime {
  return {
    enabled: true,
    check: vi.fn().mockResolvedValue(null),
    confirm: vi.fn().mockResolvedValue(true),
    relaunch: vi.fn().mockResolvedValue(undefined),
    ...overrides
  };
}

const confirmation = {
  title: "Install Codex Pulse update",
  message: "Version 0.4.0 is ready. Install it and restart Codex Pulse?"
};

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});
```

After those helpers and global timer hooks, add these exact lifecycle tests:

```ts
it("checks immediately, skips overlapping ticks, repeats after six hours, and stops", async () => {
  const firstCheck = deferred<null>();
  const check = vi
    .fn<UpdaterRuntime["check"]>()
    .mockReturnValueOnce(firstCheck.promise)
    .mockResolvedValue(null);
  const updater = useUpdater(makeRuntime({ check }));

  updater.start();
  updater.start();
  await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(1));

  await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
  expect(check).toHaveBeenCalledTimes(1);

  firstCheck.resolve(null);
  await vi.waitFor(() => expect(updater.state.value).toEqual({ phase: "idle" }));
  await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
  expect(check).toHaveBeenCalledTimes(2);

  updater.stop();
  await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
  expect(check).toHaveBeenCalledTimes(2);
});

it("does nothing when the production runtime gate is disabled", async () => {
  const check = vi.fn<UpdaterRuntime["check"]>().mockResolvedValue(null);
  const updater = useUpdater(makeRuntime({ enabled: false, check }));

  updater.start();
  await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS * 2);

  expect(check).not.toHaveBeenCalled();
  expect(updater.state.value).toEqual({ phase: "idle" });
});

it("returns to idle when the current version is latest", async () => {
  const updater = useUpdater(makeRuntime());

  updater.start();

  await vi.waitFor(() =>
    expect(updater.state.value).toEqual({ phase: "idle" })
  );
});

it("retries a failed check on the next six-hour tick", async () => {
  const check = vi
    .fn<UpdaterRuntime["check"]>()
    .mockRejectedValueOnce(new Error("synthetic outage"))
    .mockResolvedValue(null);
  const updater = useUpdater(makeRuntime({ check }));

  updater.start();
  await vi.waitFor(() =>
    expect(updater.state.value).toEqual({
      phase: "failed",
      stage: "check"
    })
  );

  await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);

  expect(check).toHaveBeenCalledTimes(2);
  expect(updater.state.value).toEqual({ phase: "idle" });
});

it("does not recheck while a verified update is ready", async () => {
  const update = makeCandidate();
  const check = vi.fn<UpdaterRuntime["check"]>().mockResolvedValue(
    update.candidate
  );
  const updater = useUpdater(makeRuntime({ check }));

  updater.start();
  await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
  await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);

  expect(check).toHaveBeenCalledTimes(1);
  expect(updater.state.value).toEqual({
    phase: "ready",
    version: "0.4.0"
  });
});
```

- [ ] **Step 2: Add progress, readiness, confirmation, and failure tests**

Add these cases to the same `describe`:

```ts
it("reports known progress and becomes ready only after download resolves", async () => {
  const downloadGate = deferred<void>();
  const update = makeCandidate("0.4.0", downloadGate);
  const updater = useUpdater(
    makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
  );

  updater.start();
  await vi.waitFor(() =>
    expect(updater.state.value.phase).toBe("downloading")
  );
  update.emit({ event: "Started", data: { contentLength: 200 } });
  update.emit({ event: "Progress", data: { chunkLength: 84 } });

  expect(updater.state.value).toEqual({
    phase: "downloading",
    version: "0.4.0",
    downloaded: 84,
    total: 200,
    percent: 42
  });

  downloadGate.resolve();
  await vi.waitFor(() =>
    expect(updater.state.value).toEqual({
      phase: "ready",
      version: "0.4.0"
    })
  );
});

it("keeps progress indeterminate when content length is unavailable", async () => {
  const downloadGate = deferred<void>();
  const update = makeCandidate("0.4.0", downloadGate);
  const updater = useUpdater(
    makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
  );

  updater.start();
  await vi.waitFor(() =>
    expect(updater.state.value.phase).toBe("downloading")
  );
  update.emit({ event: "Started", data: {} });
  update.emit({ event: "Progress", data: { chunkLength: 32 } });

  expect(updater.state.value).toEqual({
    phase: "downloading",
    version: "0.4.0",
    downloaded: 32
  });
  downloadGate.resolve();
});

it("keeps a verified update ready when confirmation is cancelled", async () => {
  const update = makeCandidate();
  const runtime = makeRuntime({
    check: vi.fn().mockResolvedValue(update.candidate),
    confirm: vi.fn().mockResolvedValue(false)
  });
  const updater = useUpdater(runtime);

  updater.start();
  await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
  await updater.activate(confirmation);

  expect(runtime.confirm).toHaveBeenCalledWith(confirmation.message, {
    title: confirmation.title,
    kind: "info"
  });
  expect(update.candidate.install).not.toHaveBeenCalled();
  expect(update.candidate.close).not.toHaveBeenCalled();
  expect(updater.state.value).toEqual({ phase: "ready", version: "0.4.0" });
});

it("installs with Windows restart enabled and relaunches when install returns", async () => {
  const update = makeCandidate();
  const runtime = makeRuntime({
    check: vi.fn().mockResolvedValue(update.candidate)
  });
  const updater = useUpdater(runtime);

  updater.start();
  await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
  await updater.activate(confirmation);

  expect(update.candidate.install).toHaveBeenCalledWith({
    restartAfterInstall: true
  });
  expect(update.candidate.close).toHaveBeenCalledTimes(1);
  expect(runtime.relaunch).toHaveBeenCalledTimes(1);
  expect(updater.state.value).toEqual({
    phase: "installing",
    version: "0.4.0"
  });
});

it("closes a failed download and retries from a fresh check", async () => {
  const broken = makeCandidate();
  vi.mocked(broken.candidate.download).mockRejectedValue(
    new Error("synthetic download failure")
  );
  const recovered = makeCandidate("0.4.1");
  const check = vi
    .fn<UpdaterRuntime["check"]>()
    .mockResolvedValueOnce(broken.candidate)
    .mockResolvedValueOnce(recovered.candidate);
  const updater = useUpdater(makeRuntime({ check }));

  updater.start();
  await vi.waitFor(() =>
    expect(updater.state.value).toEqual({
      phase: "failed",
      stage: "download"
    })
  );
  expect(broken.candidate.close).toHaveBeenCalledTimes(1);

  await updater.activate(confirmation);
  await vi.waitFor(() =>
    expect(updater.state.value).toEqual({
      phase: "ready",
      version: "0.4.1"
    })
  );
  expect(check).toHaveBeenCalledTimes(2);
});

it.each([
  ["check", "check"],
  ["confirm", "confirm"],
  ["install", "install"],
  ["relaunch", "relaunch"]
] as const)("records a retryable %s failure without exposing its error", async (operation, stage) => {
  const update = makeCandidate();
  const runtime = makeRuntime({
    check:
      operation === "check"
        ? vi.fn().mockRejectedValue(new Error("private check detail"))
        : vi.fn().mockResolvedValue(update.candidate),
    confirm:
      operation === "confirm"
        ? vi.fn().mockRejectedValue(new Error("private dialog detail"))
        : vi.fn().mockResolvedValue(true),
    relaunch:
      operation === "relaunch"
        ? vi.fn().mockRejectedValue(new Error("private relaunch detail"))
        : vi.fn().mockResolvedValue(undefined)
  });
  if (operation === "install") {
    vi.mocked(update.candidate.install).mockRejectedValue(
      new Error("private installer detail")
    );
  }
  const updater = useUpdater(runtime);

  updater.start();
  if (operation !== "check") {
    await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
    await updater.activate(confirmation);
  }

  await vi.waitFor(() =>
    expect(updater.state.value).toEqual({ phase: "failed", stage })
  );
  expect(JSON.stringify(updater.state.value)).not.toContain("private");
});

it("closes a retained ready update when stopped", async () => {
  const update = makeCandidate();
  const updater = useUpdater(
    makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
  );

  updater.start();
  await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
  updater.stop();

  await vi.waitFor(() =>
    expect(update.candidate.close).toHaveBeenCalledTimes(1)
  );
});
```

- [ ] **Step 3: Run the state-machine tests and verify RED**

Run:

```bash
pnpm test -- src/__tests__/useUpdater.spec.ts
```

Expected: FAIL because `useUpdater.ts` and all exported contracts are absent.

- [ ] **Step 4: Implement the minimal testable updater**

Create `src/composables/useUpdater.ts`:

```ts
import { isTauri } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  check,
  type DownloadEvent
} from "@tauri-apps/plugin-updater";
import { readonly, ref } from "vue";

export const UPDATE_CHECK_INTERVAL_MS = 21_600_000;

export type UpdaterDownloadEvent = DownloadEvent;
export type UpdaterFailureStage =
  | "check"
  | "download"
  | "confirm"
  | "install"
  | "relaunch";

export type UpdaterState =
  | { phase: "idle" }
  | { phase: "checking" }
  | {
      phase: "downloading";
      version: string;
      downloaded: number;
      total?: number;
      percent?: number;
    }
  | { phase: "ready"; version: string }
  | { phase: "installing"; version: string }
  | { phase: "failed"; stage: UpdaterFailureStage };

export interface UpdateCandidate {
  version: string;
  download(
    onEvent?: (event: UpdaterDownloadEvent) => void
  ): Promise<void>;
  install(options?: { restartAfterInstall?: boolean }): Promise<void>;
  close(): Promise<void>;
}

export interface UpdaterRuntime {
  enabled: boolean;
  check(): Promise<UpdateCandidate | null>;
  confirm(
    message: string,
    options: { title: string; kind: "info" }
  ): Promise<boolean>;
  relaunch(): Promise<void>;
}

export interface UpdateConfirmationCopy {
  title: string;
  message: string;
}

const productionRuntime: UpdaterRuntime = {
  enabled: import.meta.env.PROD && isTauri(),
  check: async () => await check(),
  confirm,
  relaunch
};

export function useUpdater(runtime: UpdaterRuntime = productionRuntime) {
  const state = ref<UpdaterState>({ phase: "idle" });
  let candidate: UpdateCandidate | undefined;
  let timer: ReturnType<typeof setInterval> | undefined;
  let started = false;
  let inFlight = false;

  async function closeCandidate() {
    const stale = candidate;
    candidate = undefined;
    if (!stale) return;
    try {
      await stale.close();
    } catch {
      // Closing a stale resource must not replace the original state.
    }
  }

  function blocksCheck() {
    return (
      inFlight ||
      state.value.phase === "downloading" ||
      state.value.phase === "ready" ||
      state.value.phase === "installing"
    );
  }

  async function checkForUpdate() {
    if (!runtime.enabled || blocksCheck()) return;
    inFlight = true;
    let stage: UpdaterFailureStage = "check";
    state.value = { phase: "checking" };

    try {
      const update = await runtime.check();
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
            ? Math.min(100, Math.max(0, Math.floor((downloaded / total) * 100)))
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

      state.value = { phase: "ready", version: update.version };
    } catch {
      await closeCandidate();
      state.value = { phase: "failed", stage };
    } finally {
      inFlight = false;
    }
  }

  async function activate(copy: UpdateConfirmationCopy) {
    if (state.value.phase === "failed") {
      await checkForUpdate();
      return;
    }
    if (state.value.phase !== "ready" || !candidate || inFlight) return;

    const update = candidate;
    const version = state.value.version;
    inFlight = true;
    let stage: UpdaterFailureStage = "confirm";

    try {
      const accepted = await runtime.confirm(copy.message, {
        title: copy.title,
        kind: "info"
      });
      if (!accepted) return;

      stage = "install";
      state.value = { phase: "installing", version };
      await update.install({ restartAfterInstall: true });

      stage = "relaunch";
      await closeCandidate();
      await runtime.relaunch();
    } catch {
      await closeCandidate();
      state.value = { phase: "failed", stage };
    } finally {
      inFlight = false;
    }
  }

  function start() {
    if (started || !runtime.enabled) return;
    started = true;
    void checkForUpdate();
    timer = setInterval(() => {
      void checkForUpdate();
    }, UPDATE_CHECK_INTERVAL_MS);
  }

  function stop() {
    started = false;
    if (timer) clearInterval(timer);
    timer = undefined;
    void closeCandidate();
  }

  return {
    state: readonly(state),
    start,
    stop,
    activate
  };
}
```

- [ ] **Step 5: Run the focused suite and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/useUpdater.spec.ts
pnpm build
```

Expected: all state-machine tests PASS and TypeScript accepts the official
plugin bindings. If TypeScript reports structural incompatibility for
`check()`, retain the public `UpdateCandidate` interface and explicitly return
the official value through a local `UpdateCandidate | null` annotation; do not
weaken tests or use `any`.

- [ ] **Step 6: Commit the state machine**

```bash
git add src/composables/useUpdater.ts src/__tests__/useUpdater.spec.ts
git commit -m "feat: add automatic update state machine"
```

---

### Task 3: Add the localized TopBar update badge

**Files:**

- Modify: `src/components/TopBar.vue`
- Modify: `src/i18n.ts`
- Modify: `src/styles.css`
- Modify: `src/__tests__/TopBar.spec.ts`
- Modify: `src/__tests__/topBarLayout.spec.ts`
- Modify: `src/__tests__/i18n.spec.ts`

**Interfaces:**

- Consumes: `updateState?: UpdaterState`, defaulting to `{ phase: "idle" }`.
- Produces: `activate-update` with no payload for ready and failed badge clicks.
- Preserves: all existing pin, theme, locale, and active-count events.
- Produces: `updater.*` translation keys with the same shape in all locales.

- [ ] **Step 1: Extend TopBar tests with badge behavior**

Update the `mountTopBar` helper in `src/__tests__/TopBar.spec.ts` to accept an
optional `updateState` and pass `{ phase: "idle" }` by default. Import
`UpdaterState` as a type from `../composables/useUpdater`.

```ts
interface TopBarTestProps {
  activeCount: number;
  alwaysOnTop: boolean;
  theme: "system" | "light" | "dark";
  locale: "system" | "zh-CN" | "en" | "fr" | "de";
  updateState?: UpdaterState;
}

function mountTopBar(props: TopBarTestProps) {
  return mount(TopBar, {
    props: {
      updateState: { phase: "idle" },
      ...props
    },
    global: { plugins: [i18n] }
  });
}
```

Add these tests:

```ts
it("keeps the active count when updater activity is not visible", () => {
  const wrapper = mountTopBar({
    activeCount: 3,
    alwaysOnTop: false,
    theme: "system",
    locale: "en",
    updateState: { phase: "checking" }
  });

  expect(wrapper.text()).toContain("3 active");
  expect(wrapper.find(".top-bar__update").exists()).toBe(false);
});

it.each([
  [
    { phase: "downloading", version: "0.4.0", downloaded: 42, total: 100, percent: 42 },
    "Update 42%",
    true
  ],
  [
    { phase: "downloading", version: "0.4.0", downloaded: 42 },
    "Updating",
    true
  ],
  [{ phase: "ready", version: "0.4.0" }, "Update", false],
  [{ phase: "installing", version: "0.4.0" }, "Updating", true],
  [{ phase: "failed", stage: "download" }, "Update failed", false]
] as const)("renders updater state %o as %s", (updateState, label, disabled) => {
  const wrapper = mountTopBar({
    activeCount: 3,
    alwaysOnTop: false,
    theme: "system",
    locale: "en",
    updateState
  });
  const badge = wrapper.get(".top-bar__update");

  expect(wrapper.find(".top-bar__count").exists()).toBe(false);
  expect(badge.text()).toBe(label);
  expect(badge.attributes("aria-live")).toBe("polite");
  expect(badge.attributes("disabled") !== undefined).toBe(disabled);
});

it("emits activation only from an enabled update badge", async () => {
  const wrapper = mountTopBar({
    activeCount: 1,
    alwaysOnTop: false,
    theme: "system",
    locale: "en",
    updateState: { phase: "ready", version: "0.4.0" }
  });

  const badge = wrapper.get(".top-bar__update");
  expect(badge.attributes("title")).toBe("Install version 0.4.0");
  expect(badge.attributes("aria-label")).toBe("Install version 0.4.0");
  await badge.trigger("click");

  expect(wrapper.emitted("activate-update")).toHaveLength(1);
});
```

- [ ] **Step 2: Add locale and narrow-layout contracts**

In `src/__tests__/i18n.spec.ts`, import `messages` and add a structural locale
contract. It catches a missing or blank locale entry without pinning
human-facing wording:

```ts
it.each(["zh-CN", "fr", "de"] as const)(
  "keeps updater keys complete and non-empty for %s",
  (locale) => {
    const englishKeys = Object.keys(messages.en.updater).sort();
    const localized = messages[locale].updater;

    expect(Object.keys(localized).sort()).toEqual(englishKeys);
    expect(
      Object.values(localized).every(
        (value) => typeof value === "string" && value.trim().length > 0
      )
    ).toBe(true);
  }
);

it("keeps the English updater keys non-empty", () => {
  expect(
    Object.values(messages.en.updater).every(
      (value) => value.trim().length > 0
    )
  ).toBe(true);
});
```

In `src/__tests__/topBarLayout.spec.ts`, replace the old assertion that the
narrow media contains no mark rule with these exact requirements:

```ts
expect(rule(".top-bar .top-bar__update")).toContain("flex: 0 0 auto;");
expect(rule(".top-bar .top-bar__update")).toContain("white-space: nowrap;");
expect(narrowMedia).toContain(".top-bar--updating .top-bar__mark");
expect(narrowMedia).toContain("display: none;");
expect(narrowMedia).toContain(".top-bar--updating { gap: 6px;");
expect(narrowMedia).toContain(
  ".top-bar--updating .top-bar__controls { gap: 3px;"
);
```

This updates the repository's existing CSS contract; it does not substitute
for the real 320-pixel browser measurement in Task 7.

- [ ] **Step 3: Run the component contracts and verify RED**

Run:

```bash
pnpm test -- src/__tests__/TopBar.spec.ts src/__tests__/topBarLayout.spec.ts src/__tests__/i18n.spec.ts
```

Expected: FAIL because TopBar has no updater prop/event/badge and the translation
keys and styles are absent.

- [ ] **Step 4: Add exact updater copy in all four locales**

Add this sibling object to each locale in `src/i18n.ts`:

```ts
// en
updater: {
  downloading: "Update {percent}%",
  downloadingUnknown: "Updating",
  ready: "Update",
  installing: "Updating",
  failed: "Update failed",
  readyTitle: "Install version {version}",
  retryTitle: "Retry update",
  confirmTitle: "Install Codex Pulse update",
  confirmMessage: "Version {version} is ready. Install it and restart Codex Pulse?"
}

// zh-CN
updater: {
  downloading: "更新 {percent}%",
  downloadingUnknown: "更新中",
  ready: "更新",
  installing: "更新中",
  failed: "更新失败",
  readyTitle: "安装版本 {version}",
  retryTitle: "重试更新",
  confirmTitle: "安装 Codex Pulse 更新",
  confirmMessage: "版本 {version} 已准备好。现在安装并重启 Codex Pulse？"
}

// fr
updater: {
  downloading: "MàJ {percent} %",
  downloadingUnknown: "MàJ…",
  ready: "MàJ",
  installing: "MàJ…",
  failed: "Échec MàJ",
  readyTitle: "Installer la version {version}",
  retryTitle: "Réessayer la mise à jour",
  confirmTitle: "Installer la mise à jour de Codex Pulse",
  confirmMessage: "La version {version} est prête. L’installer et redémarrer Codex Pulse ?"
}

// de
updater: {
  downloading: "Update {percent} %",
  downloadingUnknown: "Update läuft",
  ready: "Update",
  installing: "Update läuft",
  failed: "Updatefehler",
  readyTitle: "Version {version} installieren",
  retryTitle: "Update erneut versuchen",
  confirmTitle: "Codex-Pulse-Update installieren",
  confirmMessage: "Version {version} ist bereit. Jetzt installieren und Codex Pulse neu starten?"
}
```

- [ ] **Step 5: Render and activate the badge**

In `TopBar.vue`:

- import `computed`;
- import `UpdaterState` as a type;
- add optional `updateState` with an idle default;
- add `activate-update` to `defineEmits`;
- compute `showUpdate`, `updateLabel`, `updateTitle`, and `updateDisabled`;
- add `top-bar--updating` to the header while visible;
- render the active count only while hidden; and
- render the update button otherwise.

Use these exact script contracts:

```ts
interface TopBarProps {
  activeCount: number;
  alwaysOnTop: boolean;
  theme: ThemeMode;
  locale: LocaleMode;
  updateState?: UpdaterState;
}

const props = withDefaults(defineProps<TopBarProps>(), {
  updateState: () => ({ phase: "idle" as const })
});
const emit = defineEmits<{
  "toggle-pin": [];
  "set-theme": [theme: ThemeMode];
  "set-locale": [locale: LocaleMode];
  "activate-update": [];
}>();

const showUpdate = computed(
  () =>
    props.updateState.phase !== "idle" &&
    props.updateState.phase !== "checking"
);
const updateLabel = computed(() => {
  const state = props.updateState;
  if (state.phase === "downloading") {
    return state.percent === undefined
      ? t("updater.downloadingUnknown")
      : t("updater.downloading", { percent: state.percent });
  }
  if (state.phase === "ready") return t("updater.ready");
  if (state.phase === "installing") return t("updater.installing");
  if (state.phase === "failed") return t("updater.failed");
  return "";
});
const updateTitle = computed(() => {
  const state = props.updateState;
  if (state.phase === "ready") {
    return t("updater.readyTitle", { version: state.version });
  }
  if (state.phase === "failed") return t("updater.retryTitle");
  return updateLabel.value;
});
const updateDisabled = computed(
  () =>
    props.updateState.phase === "downloading" ||
    props.updateState.phase === "installing"
);
```

Use this exact badge template after `top-bar__name`:

```vue
<span v-if="!showUpdate" class="top-bar__count">{{ t("topBar.active", { count: props.activeCount }) }}</span>
<button
  v-else
  class="top-bar__update"
  :class="{ 'top-bar__update--failed': props.updateState.phase === 'failed' }"
  type="button"
  :disabled="updateDisabled"
  :title="updateTitle"
  :aria-label="updateTitle"
  aria-live="polite"
  @click="emit('activate-update')"
>{{ updateLabel }}</button>
```

The computed values must map:

- known download to `updater.downloading`;
- unknown download and install to their indeterminate labels;
- ready title to `updater.readyTitle` with the target version;
- failed title to `updater.retryTitle`; and
- disabled to only downloading/installing.

- [ ] **Step 6: Add compact light, dark, and narrow styles**

Add a `.top-bar__update` rule after the generic TopBar button rule. It must
override the square button dimensions:

```css
.top-bar .top-bar__update { width: auto; min-width: 40px; max-width: 86px; height: 24px; min-height: 24px; flex: 0 0 auto; overflow: hidden; padding: 0 7px; border-radius: 999px; font-size: 10px; font-variant-numeric: tabular-nums; font-weight: 700; line-height: 1; text-overflow: ellipsis; white-space: nowrap; }
.top-bar .top-bar__update:disabled { cursor: default; opacity: 0.78; transform: none; }
.top-bar .top-bar__update--failed { border-color: rgba(193, 49, 64, 0.28); color: #a82132; background: rgba(255, 221, 225, 0.62); }
```

Inside `@media (max-width: 360px)`, add:

```css
.top-bar--updating { gap: 6px; }
.top-bar--updating .top-bar__brand { gap: 5px; font-size: 14px; }
.top-bar--updating .top-bar__mark { display: none; }
.top-bar--updating .top-bar__controls { gap: 3px; }
.top-bar--updating .top-bar__theme-group { padding: 1px; }
.top-bar--updating .top-bar__theme-group button { width: 24px; height: 24px; min-height: 24px; }
.top-bar--updating .top-bar__locale > button, .top-bar--updating .top-bar__pin { width: 28px; }
```

Add a dark failure override:

```css
:root[data-theme="dark"] .top-bar .top-bar__update--failed { border-color: rgba(255, 132, 147, 0.34); color: #ff9eaa; background: rgba(103, 32, 45, 0.5); }
```

- [ ] **Step 7: Run the focused suite and build to verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/TopBar.spec.ts src/__tests__/topBarLayout.spec.ts src/__tests__/i18n.spec.ts
pnpm build
```

Expected: component, layout, locale, and type-check contracts PASS.

- [ ] **Step 8: Commit the TopBar experience**

```bash
git add src/components/TopBar.vue src/i18n.ts src/styles.css src/__tests__/TopBar.spec.ts src/__tests__/topBarLayout.spec.ts src/__tests__/i18n.spec.ts
git commit -m "feat: show automatic update status"
```

---

### Task 4: Wire updater lifecycle and native confirmation into App

**Files:**

- Modify: `src/App.vue`
- Create: `src/__tests__/AppUpdater.spec.ts`

**Interfaces:**

- Consumes: `useUpdater()` and `updater.*` locale keys.
- Produces: updater `start()` only after the initial `pulse.load()` promise settles.
- Produces: updater `stop()` during root unmount.
- Produces: localized `UpdateConfirmationCopy` for ready installs.

- [ ] **Step 1: Write the mounted App integration contract**

Create `src/__tests__/AppUpdater.spec.ts`. Mock only the external Tauri
IPC/network/native-dialog boundary; mount the real `App`, `usePulse`,
`useUpdater`, `TopBar`, and i18n:

```ts
import { flushPromises, mount } from "@vue/test-utils";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi
} from "vitest";
import type { AppSnapshot } from "../types";

const boundary = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  check: vi.fn(),
  confirm: vi.fn(),
  relaunch: vi.fn()
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: boundary.invoke,
  isTauri: () => true
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: boundary.listen
}));
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: boundary.check
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: boundary.confirm
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: boundary.relaunch
}));

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function snapshot(): AppSnapshot {
  return {
    sessions: [],
    weeklyQuota: undefined,
    isLoading: false,
    initialization: { runId: 1, phase: "complete", events: [] },
    monitoring: {
      enabled: true,
      needsRepair: false,
      staleCount: 0
    },
    alwaysOnTop: false,
    launchAtLogin: false,
    locale: "en",
    theme: "system"
  };
}

async function mountApp() {
  const [{ default: App }, { i18n }] = await Promise.all([
    import("../App.vue"),
    import("../i18n")
  ]);
  return mount(App, { global: { plugins: [i18n] } });
}

describe("App automatic updater integration", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    boundary.invoke.mockReset();
    boundary.listen.mockReset().mockResolvedValue(() => undefined);
    boundary.check.mockReset().mockResolvedValue(null);
    boundary.confirm.mockReset().mockResolvedValue(true);
    boundary.relaunch.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.useRealTimers();
  });

  it("waits for the first snapshot, checks in production, and stops on unmount", async () => {
    const firstSnapshot = deferred<AppSnapshot>();
    boundary.invoke.mockImplementation((command: string) => {
      if (command === "get_snapshot") return firstSnapshot.promise;
      return Promise.resolve(undefined);
    });
    vi.stubEnv("PROD", true);
    const wrapper = await mountApp();

    await flushPromises();
    expect(boundary.check).not.toHaveBeenCalled();

    firstSnapshot.resolve(snapshot());
    await flushPromises();
    expect(boundary.check).toHaveBeenCalledTimes(1);

    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(21_600_000);
    expect(boundary.check).toHaveBeenCalledTimes(1);
  });

  it("does not contact the updater outside a production build", async () => {
    boundary.invoke.mockResolvedValue(snapshot());
    vi.stubEnv("PROD", false);
    const wrapper = await mountApp();

    await flushPromises();
    await vi.advanceTimersByTimeAsync(21_600_000);

    expect(boundary.check).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("renders a ready update and sends localized copy to the native dialog", async () => {
    const update = {
      version: "0.4.0",
      download: vi.fn().mockResolvedValue(undefined),
      install: vi.fn().mockResolvedValue(undefined),
      close: vi.fn().mockResolvedValue(undefined)
    };
    boundary.invoke.mockResolvedValue(snapshot());
    boundary.check.mockResolvedValue(update);
    vi.stubEnv("PROD", true);
    const wrapper = await mountApp();

    await vi.waitFor(() =>
      expect(wrapper.get(".top-bar__update").text()).toBe("Update")
    );
    await wrapper.get(".top-bar__update").trigger("click");
    await flushPromises();

    expect(boundary.confirm).toHaveBeenCalledWith(
      "Version 0.4.0 is ready. Install it and restart Codex Pulse?",
      { title: "Install Codex Pulse update", kind: "info" }
    );
    expect(update.install).toHaveBeenCalledWith({
      restartAfterInstall: true
    });
    expect(boundary.relaunch).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
```

- [ ] **Step 2: Run the App test and verify RED**

Run:

```bash
pnpm test -- src/__tests__/AppUpdater.spec.ts
```

Expected: the production timing and ready-badge cases FAIL because App does not
start, stop, render, or activate the real updater. The non-production case may
already pass; the file remains RED overall for the intended missing behavior.

- [ ] **Step 3: Wire the composable without coupling it to Pulse**

In `src/App.vue`:

```ts
import { useUpdater } from "./composables/useUpdater";

const updater = useUpdater();
const updaterState = updater.state;

async function activateUpdate() {
  const version =
    updaterState.value.phase === "ready" ? updaterState.value.version : "";
  await updater.activate({
    title: t("updater.confirmTitle"),
    message: t("updater.confirmMessage", { version })
  });
}
```

Immediately after the existing `await pulse.load();`, call:

```ts
updater.start();
```

In `onUnmounted`, call:

```ts
updater.stop();
```

Pass these additions to `TopBar`:

```vue
:update-state="updaterState"
@activate-update="activateUpdate"
```

Do not put updater state in `AppSnapshot`, do not call `pulse.load()` from
updater callbacks, and do not route updater errors into `pulse.error`.

- [ ] **Step 4: Run App, state-machine, and production-build checks**

Run:

```bash
pnpm test -- src/__tests__/AppUpdater.spec.ts src/__tests__/App.spec.ts src/__tests__/useUpdater.spec.ts src/__tests__/TopBar.spec.ts
pnpm build
```

Expected: all tests PASS, including the existing 60-second session fallback;
the mounted integration proves real lifecycle ordering and dialog copy, and the
production bundle type-checks with updater lifecycle wiring.

- [ ] **Step 5: Commit App integration**

```bash
git add src/App.vue src/__tests__/AppUpdater.spec.ts
git commit -m "feat: connect automatic updates to app lifecycle"
```

---

### Task 5: Sign and validate the Draft release manifest

**Files:**

- Create: `scripts/verify-updater-manifest.mjs`
- Create: `src/__tests__/updaterManifest.spec.ts`
- Modify: `.github/workflows/release.yml`
- Modify: `src/__tests__/githubWorkflows.spec.ts`

**Interfaces:**

- Produces: `node scripts/verify-updater-manifest.mjs <path> <version>`.
- Requires: exact `version`, `darwin-aarch64`, `windows-x86_64`, and non-empty
  string `url`/`signature` values.
- Produces: release build environment with the two approved GitHub Secrets.
- Preserves: tag guard, Draft release, two-target matrix, and serial uploads.

- [ ] **Step 1: Write manifest validator tests**

Create `src/__tests__/updaterManifest.spec.ts`:

```ts
import {
  mkdtempSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";

const directories: string[] = [];

type PlatformName = "darwin-aarch64" | "windows-x86_64";
type ManifestField = "url" | "signature";
interface ManifestFixture {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Record<
    PlatformName,
    { url: string; signature: string }
  >;
}

function runManifest(manifest: unknown, version = "0.4.0") {
  const directory = mkdtempSync(join(tmpdir(), "codex-pulse-updater-"));
  directories.push(directory);
  const path = join(directory, "latest.json");
  writeFileSync(path, JSON.stringify(manifest));
  return spawnSync(
    process.execPath,
    [
      resolve(process.cwd(), "scripts/verify-updater-manifest.mjs"),
      path,
      version
    ],
    { encoding: "utf8" }
  );
}

function validManifest(): ManifestFixture {
  return {
    version: "0.4.0",
    notes: "Synthetic test fixture",
    pub_date: "2026-07-28T00:00:00Z",
    platforms: {
      "darwin-aarch64": {
        url: "https://example.invalid/Codex.Pulse.app.tar.gz",
        signature: "mac-signature"
      },
      "windows-x86_64": {
        url: "https://example.invalid/Codex.Pulse-setup.exe",
        signature: "windows-signature"
      }
    }
  };
}

afterEach(() => {
  for (const directory of directories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

describe("updater manifest validator", () => {
  it("accepts the exact version and both signed platforms", () => {
    const result = runManifest(validManifest());

    expect(result.status).toBe(0);
    expect(result.stdout).toContain(
      "Validated updater manifest 0.4.0 for darwin-aarch64, windows-x86_64"
    );
    expect(result.stderr).toBe("");
  });

  it("rejects a version that does not match the tag", () => {
    const result = runManifest(validManifest(), "0.4.1");

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain(
      "version 0.4.0 does not match tag 0.4.1"
    );
  });

  it.each([
    ["darwin-aarch64", "url"],
    ["darwin-aarch64", "signature"],
    ["windows-x86_64", "url"],
    ["windows-x86_64", "signature"]
  ] satisfies ReadonlyArray<readonly [PlatformName, ManifestField]>)(
    "rejects missing %s %s",
    (platform, field) => {
      const manifest = validManifest();
      manifest.platforms[platform][field] = "";
      const result = runManifest(manifest);

      expect(result.status).not.toBe(0);
      expect(result.stderr).toContain(`${platform}.${field} must be non-empty`);
    }
  );
});
```

- [ ] **Step 2: Extend the workflow contract before changing YAML**

In `src/__tests__/githubWorkflows.spec.ts`:

- allow `WorkflowJob.needs` to be `string | string[]`;
- add optional top-level job `env`;
- update expected release job keys to
  `["guard", "release", "verify_updater_manifest"]`;
- require the Tauri Action environment to include both signing Secrets;
- require `uploadUpdaterJson`, `uploadUpdaterSignatures`, and
  `updaterJsonPreferNsis` to be `true`; and
- assert the verification job needs `release`, checks out without persisted
  credentials, downloads `latest.json` with `gh release download`, and calls
  the validator with `$GITHUB_REF_NAME`.

Use these exact signing expectations:

```ts
expect(build.env).toEqual({
  GITHUB_TOKEN: "${{ secrets.GITHUB_TOKEN }}",
  APPLE_SIGNING_IDENTITY: "${{ matrix.apple-signing-identity }}",
  TAURI_SIGNING_PRIVATE_KEY:
    "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}",
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD:
    "${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}"
});
expect(build.with).toMatchObject({
  releaseDraft: true,
  uploadUpdaterJson: true,
  uploadUpdaterSignatures: true,
  updaterJsonPreferNsis: true
});
expect(release.strategy?.["max-parallel"]).toBe(1);
```

- [ ] **Step 3: Run validator and workflow tests to verify RED**

Run:

```bash
pnpm test -- src/__tests__/updaterManifest.spec.ts src/__tests__/githubWorkflows.spec.ts
```

Expected: FAIL because the validator script is absent and the workflow still
sets `uploadUpdaterJson: false` with no signing environment or validation job.

- [ ] **Step 4: Implement the strict manifest validator**

Create `scripts/verify-updater-manifest.mjs`:

```js
#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const [manifestPath, expectedVersion] = process.argv.slice(2);
const requiredPlatforms = ["darwin-aarch64", "windows-x86_64"];

function fail(message) {
  console.error(`Updater manifest invalid: ${message}`);
  process.exit(1);
}

if (!manifestPath || !expectedVersion) {
  fail("usage: verify-updater-manifest.mjs <path> <version>");
}

let manifest;
try {
  manifest = JSON.parse(await readFile(manifestPath, "utf8"));
} catch (error) {
  fail(`cannot read JSON: ${error instanceof Error ? error.message : String(error)}`);
}

if (
  typeof manifest !== "object" ||
  manifest === null ||
  Array.isArray(manifest)
) {
  fail("root must be an object");
}

if (manifest.version !== expectedVersion) {
  fail(`version ${String(manifest.version)} does not match tag ${expectedVersion}`);
}

if (
  typeof manifest.platforms !== "object" ||
  manifest.platforms === null ||
  Array.isArray(manifest.platforms)
) {
  fail("platforms must be an object");
}

for (const platform of requiredPlatforms) {
  const entry = manifest.platforms[platform];
  if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
    fail(`${platform} must be an object`);
  }
  for (const field of ["url", "signature"]) {
    if (typeof entry[field] !== "string" || entry[field].trim() === "") {
      fail(`${platform}.${field} must be non-empty`);
    }
  }
}

console.log(
  `Validated updater manifest ${expectedVersion} for ${requiredPlatforms.join(", ")}`
);
```

- [ ] **Step 5: Enable signed updater uploads and Draft validation**

In the existing `Build draft release` step:

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  APPLE_SIGNING_IDENTITY: ${{ matrix.apple-signing-identity }}
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
with:
  uploadUpdaterJson: true
  uploadUpdaterSignatures: true
  updaterJsonPreferNsis: true
```

Keep every existing Tauri Action input, including `releaseDraft: true`. Keep
`strategy.max-parallel: 1`.

Add this job after `release`:

```yaml
verify_updater_manifest:
  name: Verify updater manifest
  runs-on: ubuntu-latest
  needs: release
  env:
    GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  steps:
    - name: Check out repository
      uses: actions/checkout@v7
      with:
        persist-credentials: false
    - name: Download updater manifest
      shell: bash
      run: |
        mkdir -p "$RUNNER_TEMP/updater-manifest"
        gh release download "$GITHUB_REF_NAME" \
          --repo "$GITHUB_REPOSITORY" \
          --pattern latest.json \
          --dir "$RUNNER_TEMP/updater-manifest" \
          --clobber
    - name: Validate updater manifest
      run: node scripts/verify-updater-manifest.mjs "$RUNNER_TEMP/updater-manifest/latest.json" "$GITHUB_REF_NAME"
```

- [ ] **Step 6: Run the focused workflow suite and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/updaterManifest.spec.ts src/__tests__/githubWorkflows.spec.ts
git diff --check .github/workflows/release.yml scripts/verify-updater-manifest.mjs
```

Expected: validator fixtures and all pre-existing workflow contracts PASS.

- [ ] **Step 7: Commit the signed Draft pipeline**

```bash
git add .github/workflows/release.yml scripts/verify-updater-manifest.mjs src/__tests__/updaterManifest.spec.ts src/__tests__/githubWorkflows.spec.ts
git commit -m "ci: publish signed updater manifests"
```

---

### Task 6: Configure GitHub Secrets and document updater behavior

**Files:**

- Modify: `README.md`
- Modify: `docs/README.zh-CN.md`
- Modify: `src/__tests__/githubCommunity.spec.ts`
- External modify: GitHub repository Actions Secrets

**Interfaces:**

- Produces: English and Chinese public claims for network timing, automatic
  download, transcript privacy, failure isolation, manual bootstrap, and
  platform signing limits.
- Produces: the two exact GitHub Secret names consumed by Task 5.
- Preserves: the private key and passphrase as write-only external state.

- [ ] **Step 1: Replace only the stale release-status assertion**

In `src/__tests__/githubCommunity.spec.ts`, replace the exact stale Draft
phrases:

```ts
expect(english).toContain("unsigned experimental Draft Release artifact");
expect(chinese).toContain("未签名的实验性草稿发布产物");
```

with these aligned public-status assertions:

```ts
expect(english).toContain("published unsigned experimental installer");
expect(english).not.toContain("unsigned experimental Draft Release artifact");
expect(chinese).toContain("已发布的未签名实验性安装程序");
expect(chinese).not.toContain("未签名的实验性草稿发布产物");
```

Retain the existing unsigned, SmartScreen, Gatekeeper, WSL, and platform
support checks. Do not add exact-phrase tests for the new human-facing privacy
prose; the user explicitly approved that documentation-class test exception.

- [ ] **Step 2: Run the documentation contract and verify RED**

Run:

```bash
pnpm test -- src/__tests__/githubCommunity.spec.ts
```

Expected: FAIL because the READMEs still describe the Windows installer as a
Draft. The privacy prose is reviewed semantically in Steps 3-5.

- [ ] **Step 3: Correct release status and add English disclosure**

In `README.md`:

- change the warning to call Windows a `published unsigned experimental installer`;
- change platform status to published experimental DMG/NSIS for `0.3.2`;
- retain the Apple/Windows trust warnings; and
- add an `## Automatic Updates and Privacy` section after platform support.

That section must state that updater-capable production builds:

1. check GitHub Releases after startup and every six hours;
2. download a signed installer automatically and ask before install/restart;
3. do not send Codex transcripts, prompts, session data, quota data, or project
   paths;
4. leave session monitoring operational if a check fails;
5. require the first updater-capable release to be installed manually; and
6. use an updater signature that does not replace Apple notarization or Windows
   Authenticode.

Also state that ordinary request metadata is handled under GitHub's terms.

- [ ] **Step 4: Mirror the same disclosure in Simplified Chinese**

In `docs/README.zh-CN.md`, make the corresponding release-status changes and
add `## 自动更新与隐私`. Use the tested published-installer status phrase, then
retain the same six semantic claims and GitHub request-metadata qualification.
Translate the meaning rather than mechanically mirroring English word order.

- [ ] **Step 5: Run aligned documentation tests to verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/githubCommunity.spec.ts
```

Expected: PASS with no stale Draft assertion.

Then inspect the English and Chinese diff side by side and check the six claims
from Steps 3-4 one by one. Confirm neither document implies that updater
signatures remove Gatekeeper/SmartScreen requirements, that `0.3.2` already
contains the updater, or that a failed update check affects session monitoring.

- [ ] **Step 6: Write both signing values to GitHub Actions Secrets**

Run without `set -x`:

```bash
set -euo pipefail
UPDATER_KEY_PATH=/Users/loki/.tauri/codex-pulse-updater.key
UPDATER_KEYCHAIN_SERVICE="Codex Pulse Updater Signing"
UPDATER_KEYCHAIN_ACCOUNT=qwertyerge/codex-pulse
test -s "$UPDATER_KEY_PATH"
UPDATER_KEY_PASSWORD="$(security find-generic-password -w -a "$UPDATER_KEYCHAIN_ACCOUNT" -s "$UPDATER_KEYCHAIN_SERVICE")"
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo qwertyerge/codex-pulse < "$UPDATER_KEY_PATH"
printf '%s' "$UPDATER_KEY_PASSWORD" | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo qwertyerge/codex-pulse
unset UPDATER_KEY_PASSWORD
```

Verify names and update timestamps only:

```bash
gh secret list --repo qwertyerge/codex-pulse --json name,updatedAt --jq '.[] | select(.name == "TAURI_SIGNING_PRIVATE_KEY" or .name == "TAURI_SIGNING_PRIVATE_KEY_PASSWORD")'
```

Expected: exactly both approved names are returned. Do not use a command that
prints either value. Do not treat GitHub Secrets as the required offline
backup.

- [ ] **Step 7: Commit the public documentation**

```bash
git add README.md docs/README.zh-CN.md src/__tests__/githubCommunity.spec.ts
git commit -m "docs: explain automatic update behavior"
```

---

### Task 7: Run fresh verification and record the remaining gates

**Files:**

- Create: `docs/superpowers/reports/automatic-updates-acceptance.md`
- Modify only if verification exposes a defect: files from Tasks 1-6

**Interfaces:**

- Produces: fresh command evidence for frontend, Rust, configuration, local
  signed macOS updater artifacts, GitHub Secret names, and git hygiene.
- Produces: an explicit pending list for Windows CI artifacts, real Draft
  manifest, interactive restart, cross-version update, and offline backup.
- Preserves: detached HEAD, version `0.3.2`, no tag/Draft/publication/push.

- [ ] **Step 1: Run the complete local automated verification**

Run fresh, without relying on earlier focused output:

```bash
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: every command exits `0`. Record total frontend and Rust test counts
from these exact runs.

- [ ] **Step 2: Build a signed macOS updater artifact without printing secrets**

Run without `set -x`:

```bash
set -euo pipefail
UPDATER_KEY_PATH=/Users/loki/.tauri/codex-pulse-updater.key
UPDATER_KEYCHAIN_SERVICE="Codex Pulse Updater Signing"
UPDATER_KEYCHAIN_ACCOUNT=qwertyerge/codex-pulse
test -s "$UPDATER_KEY_PATH"
UPDATER_KEY_PASSWORD="$(security find-generic-password -w -a "$UPDATER_KEYCHAIN_ACCOUNT" -s "$UPDATER_KEYCHAIN_SERVICE")"
TAURI_SIGNING_PRIVATE_KEY="$UPDATER_KEY_PATH" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$UPDATER_KEY_PASSWORD" pnpm tauri build --bundles app
unset UPDATER_KEY_PASSWORD
```

Verify one archive and its adjacent signature:

```bash
UPDATER_BUNDLE_DIR=src-tauri/target/release/bundle/macos
test "$(find "$UPDATER_BUNDLE_DIR" -maxdepth 1 -type f -name '*.app.tar.gz' | wc -l | tr -d ' ')" = "1"
test "$(find "$UPDATER_BUNDLE_DIR" -maxdepth 1 -type f -name '*.app.tar.gz.sig' | wc -l | tr -d ' ')" = "1"
find "$UPDATER_BUNDLE_DIR" -maxdepth 1 -type f \( -name '*.app.tar.gz' -o -name '*.app.tar.gz.sig' \) -print
```

Expected: one `.app.tar.gz` and one `.app.tar.gz.sig`. This proves local
artifact generation, not installation, restart, publication, or notarization.

- [ ] **Step 3: Inspect the real 320-pixel TopBar layout**

Start Vite on the loopback interface:

```bash
pnpm dev --host 127.0.0.1
```

Open `http://127.0.0.1:5180` in the in-app browser, set the viewport to
`320x420`, and use the real rendered TopBar and loaded `src/styles.css`.
Because development deliberately disables the updater, inject only the
approved failed-badge DOM state in the browser inspector:

```js
const header = document.querySelector(".top-bar");
const brand = document.querySelector(".top-bar__brand");
const name = document.querySelector(".top-bar__name");
const count = document.querySelector(".top-bar__count");
if (!(header instanceof HTMLElement) ||
    !(brand instanceof HTMLElement) ||
    !(name instanceof HTMLElement)) {
  throw new Error("TopBar is not rendered");
}
count?.remove();
header.classList.add("top-bar--updating");
const badge = document.createElement("button");
badge.className = "top-bar__update top-bar__update--failed";
badge.type = "button";
badge.textContent = "Update failed";
badge.setAttribute("aria-label", "Retry update");
badge.setAttribute("aria-live", "polite");
name.after(badge);
const brandRect = brand.getBoundingClientRect();
const controls = document.querySelector(".top-bar__controls");
const mark = document.querySelector(".top-bar__mark");
if (!(controls instanceof HTMLElement) || !(mark instanceof SVGElement)) {
  throw new Error("TopBar controls or mark are not rendered");
}
const controlsRect = controls.getBoundingClientRect();
({
  viewportWidth: window.innerWidth,
  documentWidth: document.documentElement.scrollWidth,
  brandRight: brandRect.right,
  controlsLeft: controlsRect.left,
  overlap: brandRect.right > controlsRect.left,
  fullName: name.textContent,
  nameClipped: name.scrollWidth > name.clientWidth,
  badgeClipped: badge.scrollWidth > badge.clientWidth,
  markDisplay: getComputedStyle(mark).display
});
```

Expected measured result:

- `viewportWidth` and `documentWidth` are both `320`;
- `overlap`, `nameClipped`, and `badgeClipped` are `false`;
- `fullName` is `Codex Pulse`; and
- `markDisplay` is `none`.

Also inspect the real page once in light and dark mode for a readable badge and
visible focus ring. This is manual browser evidence, separate from the
component and CSS contracts. Stop the Vite process after recording the
measurements.

- [ ] **Step 4: Verify external names and repository hygiene**

Run:

```bash
gh secret list --repo qwertyerge/codex-pulse --json name,updatedAt --jq '.[] | select(.name == "TAURI_SIGNING_PRIVATE_KEY" or .name == "TAURI_SIGNING_PRIVATE_KEY_PASSWORD")'
test "$(stat -f '%Lp' /Users/loki/.tauri/codex-pulse-updater.key)" = "600"
if git ls-files | rg -q 'codex-pulse-updater\.key'; then
  echo "private updater key is tracked" >&2
  exit 1
fi
git diff --check
git status --short --branch
git log --oneline --decorate -10
```

Expected: two Secret names, mode `600`, no tracked key file, no whitespace
errors, detached HEAD, and only the intended automatic-update commits above
`51fcaa6`.

- [ ] **Step 5: Write the acceptance report from observed output**

Create `docs/superpowers/reports/automatic-updates-acceptance.md` with:

- exact detached HEAD and base commit;
- each verification command, exit status, and observed test count;
- exact macOS updater archive and `.sig` paths;
- the 320-pixel browser measurements and light/dark/focus observations;
- the two GitHub Secret names and their observed update timestamps, never
  values;
- confirmation that all three version files remain `0.3.2`;
- confirmation that no private key/passphrase is tracked;
- current evidence status for code, local macOS bundle, workflow contract, and
  Secrets; and
- this explicit pending list:
  - separately verified offline backup of encrypted key and passphrase;
  - first updater-capable version preparation and manual bootstrap install;
  - real Windows updater artifact and `.sig`;
  - real authenticated Draft `latest.json` with both platforms;
  - interactive macOS and Windows install/restart;
  - signed old-version to new-version update rehearsal;
  - publication and post-publication update check.

Do not mark any pending item complete based only on a local command or static
workflow test.

- [ ] **Step 6: Re-run affected checks if the report follows a verification fix**

If any defect was fixed during Steps 1-4, rerun its focused RED/GREEN test,
then rerun all four commands from Step 1 and the signed bundle check from Step
2. The acceptance report must cite only the final fresh run.

- [ ] **Step 7: Commit the verified evidence and inspect final state**

```bash
git add docs/superpowers/reports/automatic-updates-acceptance.md
git commit -m "docs: record automatic update verification"
git status --short --branch
git show --stat --oneline HEAD
```

Expected: detached and clean. Do not push, tag, create a release, or create a
pull request.

---

## First Updater Release Gate

This plan intentionally stops before release preparation. A later, separately
approved release task must not tag the first updater-capable version until it
has:

1. independently verified the encrypted key and passphrase offline backup;
2. selected the next version and updated all three version sources together;
3. run the tag workflow to a Draft;
4. authenticated to the Draft and inspected `latest.json`, both updater
   archives/signatures, DMG, and NSIS;
5. manually installed the bootstrap build on macOS ARM64 and Windows 11 x64;
6. produced a second signed test version and rehearsed old-to-new install and
   restart on both platforms; and
7. obtained explicit approval before publication.

Draft workflow success, publication, updater reachability, native restart, and
user acceptance remain separate gates.
