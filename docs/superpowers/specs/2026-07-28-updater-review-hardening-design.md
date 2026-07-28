# Updater Review Hardening Design

Date: 2026-07-28

Status: Design approved; written-spec review pending

Repository: `qwertyerge/codex-pulse`

Pull request: `#17`

## Context

PR #17 introduced signed automatic updates and a maintainer-only local macOS
signing runbook. GitHub Actions is green at commit `02b165a`, after two
follow-up fixes:

- `30ec6ea` prevents macOS/POSIX runbook tests from running on Windows and
  removes a six-hour full-app timer advance from the non-production updater
  integration test.
- `02b165a` keeps updater signing secrets out of pull-request CI while
  preserving signed updater artifacts in the tag release workflow.

CodeRabbit posted eight inline review comments and one review-body filename
nitpick. Current-source verification found four remaining concerns worth
changing:

1. The custom updater candidate contract passes an unsupported
   `restartAfterInstall` option to Tauri 2.10.1 `install()`.
2. `stop()` does not invalidate pending check, download, confirmation, or
   installation continuations.
3. Native dialog, process, and updater plugin registration has no automated IPC
   regression.
4. The historical acceptance report needs a separate follow-up section for the
   reviewed implementation rather than rewriting its original evidence.

The semicolon and test-filename findings do not require changes. The manifest
spec already terminates TypeScript statements with semicolons, and the
lower-camel filename follows the repository's existing non-component spec
naming pattern.

## Goals

- Match the installed `@tauri-apps/plugin-updater@2.10.1` zero-argument
  `install(): Promise<void>` contract.
- Make updater state changes explicitly owned by one start lifecycle and one
  active asynchronous operation.
- Allow a stopped updater instance to start again without stale work affecting
  the new lifecycle.
- Prove that dialog, process, and updater native IPC handlers are registered by
  the production builder path.
- Preserve historical evidence while recording fresh review verification.
- Reply to each inline review thread with current-code evidence.

## Non-Goals

- Cancel Tauri network, dialog, download, or installer operations at the
  operating-system level.
- Add `AbortController` wrappers around APIs that do not accept abort signals.
- Change update cadence, UI copy, release publication policy, signing keys, or
  updater endpoints.
- Inject updater signing secrets into pull-request CI.
- Rename `localUpdaterBuild.spec.ts` or mechanically reformat
  `updaterManifest.spec.ts`.
- Merge PR #17, create a tag, or publish a release.

## Lifecycle Ownership

`useUpdater()` will track two independent ownership values:

- `lifecycleGeneration` identifies one successful `start()` to `stop()` span.
- `operationToken` identifies the current check or activation operation within
  that generation.

Every asynchronous continuation must prove both values still match before it
can mutate state, retain a candidate, install, close the current generation's
candidate, or relaunch.

### Start

`start()` remains idempotent while already started. For an enabled runtime, a
fresh start advances the lifecycle generation, begins an immediate check, and
creates the six-hour interval. A start after stop is a new lifecycle; it must
not wait for or inherit ownership from stale work.

### Stop

`stop()` performs these actions in order:

1. Mark the updater stopped and advance the lifecycle generation.
2. Clear the interval.
3. Release ownership of the current operation token.
4. Return visible updater state to `idle`.
5. Move out and asynchronously close the retained candidate, if any.

Candidate close failures remain private and cannot replace `idle`.

### Stale asynchronous work

- A stale check returning `null` commits nothing.
- A stale check returning a candidate closes that returned candidate directly
  and never assigns it to the current lifecycle.
- Stale download callbacks and completion events commit no progress or `ready`
  state.
- A confirmation that resolves after stop cannot begin installation.
- If installation had already started before stop, the underlying installer may
  finish because Tauri exposes no cancellation API. The stale continuation
  cannot change state, close a candidate owned by a newer lifecycle, or
  relaunch.
- A stale `finally` block can release only its own operation token.

## Tauri Install Contract

`UpdateCandidate.install` will take no arguments. Production code will call
`await update.install()` and then, only while lifecycle ownership is still
current, explicitly close the candidate and call the process plugin's
`relaunch()`.

Windows installer presentation remains controlled by
`plugins.updater.windows.installMode` in `tauri.conf.json`; it is not a
JavaScript install option.

Tests and historical plan snippets that currently mention
`restartAfterInstall` will be updated to the zero-argument contract.

## Native Plugin Registration Boundary

The three updater-related native plugins will be registered through one generic
builder helper:

- `tauri_plugin_dialog::init()`
- `tauri_plugin_process::init()`
- `tauri_plugin_updater::Builder::new().build()`

Production `run()` and the owning-module Rust test will call the same helper.
Unrelated existing plugins and setup behavior remain in `run()`.

The Rust test will build a Tauri `MockRuntime` application, create a mock
webview, and send deliberately incomplete IPC requests to one command from each
plugin. A registered command must be routed and reject the malformed arguments.
An unregistered command returns command-not-found and fails the test. The
requests must fail before opening a dialog, restarting the process, or
contacting an updater endpoint.

## Test Strategy

All implementation changes follow red-green-refactor.

### Frontend updater contracts

The focused suite will prove:

1. `install()` is invoked with zero arguments before explicit relaunch.
2. Stop while check is pending leaves state idle and closes a late candidate.
3. Stop while download is pending suppresses progress and ready commits.
4. Stop while confirmation is pending prevents install and relaunch.
5. Stop after installation starts returns state to idle and prevents relaunch
   when installation later resolves.
6. Stop followed by start permits a fresh check while a late result and finally
   from the old generation cannot affect the new generation.

Existing retry, overlap, progress, failure-stage, and candidate-close tests
remain green.

### Native IPC contract

The owning `app.rs` test will initially fail because the shared registration
helper does not exist. After extraction, the MockRuntime IPC probes must
distinguish registered malformed commands from missing commands for dialog,
process, and updater.

### Full verification

Before each implementation commit:

- `pnpm test`
- `pnpm build`
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `git diff --check`

After push, the latest GitHub Actions run must complete successfully for
Frontend, Rust, and Rust (Windows), including NSIS build and package
verification. CodeRabbit status is reported separately from GitHub Actions.

## Evidence and Review Replies

The acceptance report's `cfc0e9d` row with 122 tests is immutable historical
evidence. After the runtime/test commit is verified, a separate follow-up
section will record:

- the exact implementation commit;
- fresh frontend file/test counts;
- fresh Rust test counts;
- build and formatting results;
- the distinction between local verification, GitHub Actions, and any remaining
  manual/native verification boundary.

The follow-up section will be committed separately so it can accurately refer
to the implementation commit without claiming that a commit contains its own
hash.

Each of the eight inline CodeRabbit comments will receive a reply in its
original thread:

- resolved findings cite the implementing commit and focused evidence;
- already-resolved Windows findings cite `30ec6ea` and the successful Windows
  job;
- the semicolon finding receives a source-based explanation for no change;
- the historical evidence reply explains why the original row was preserved
  and points to the follow-up section.

The filename nitpick exists only in the review body, not an inline thread. No
top-level review-summary comment will be added solely to answer it.

## Acceptance Criteria

- No production call or test expects `restartAfterInstall`.
- Pending work from a stopped generation cannot publish state, begin a not-yet
  started installation, close a newer candidate, or relaunch.
- Stop during an already-started installation returns the visible state to idle
  and suppresses relaunch.
- Stop followed by start creates a clean lifecycle immediately.
- The native IPC regression fails if any of the three updater-related plugin
  registrations is removed.
- Historical and follow-up evidence remain temporally accurate.
- All local verification and the latest PR checks pass.
- No signing secret, local transcript, or user-specific path is introduced.
