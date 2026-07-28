# Automatic Updates Design

## Context

Codex Pulse `0.3.2` is published for macOS ARM64 and Windows x64, but its
release contains only a DMG and an NSIS installer. The application does not
register Tauri's updater, dialog, or process plugins; its Tauri configuration
does not create updater artifacts or declare an updater endpoint; and the
release workflow explicitly disables `latest.json` generation.

Tauri updater signatures are mandatory. A published installer cannot be
retrofitted with the public key, plugin registration, and application behavior
needed to trust and install a future update. The first updater-capable Codex
Pulse release must therefore still be installed manually. Automatic updates
begin with the release after that bootstrap installation.

## Goal

Add an enabled-by-default automatic update path for the supported release
targets:

- macOS on Apple Silicon;
- Windows 11 x64 using the existing per-user NSIS installer; and
- GitHub Releases as the static update service.

In a production Tauri build, the application checks once after its initial
session load and every six hours thereafter. When a newer release is found, it
downloads and verifies the signed artifact automatically. A compact TopBar
badge then asks the user only for permission to install and restart.

The implementation must preserve the existing GitHub Draft release gate,
session-monitoring behavior, four supported locales, 320-pixel minimum window
width, and detached-worktree boundary.

## Non-Goals

- Retrofitting updater support into the already published `0.3.2` installers.
- Publishing a release, preparing a new version, creating a tag or Draft, or
  changing the current `0.3.2` version during this task.
- Pushing the detached worktree, creating a pull request, or merging.
- A settings toggle, manual "check for updates" screen, release notes screen,
  skip-version action, update channels, staged rollout, or downgrade support.
- Windows ARM64, Intel macOS, Linux, MSI, Microsoft Store, or Mac App Store
  updates.
- Background downloads after the application exits or persistence of a
  partially downloaded artifact.
- Replacing Apple Developer ID/notarization or Windows Authenticode signing.
  Tauri updater signing authenticates update artifacts but does not suppress
  operating-system trust warnings.
- Claiming a real Windows update, a real Draft manifest, or an old-to-new
  end-to-end update before the first updater release exists.

## Architecture

The update lifecycle belongs to a new frontend composable,
`src/composables/useUpdater.ts`. It is independent of `usePulse` and
`AppSnapshot`: update-server availability must not change session-monitoring
health, and update failures must not enter `pulse.error`.

The composable owns the Tauri `Update` resource and exposes a small
discriminated state plus lifecycle actions to `App.vue` and `TopBar.vue`.
Tauri's Rust side only registers the official updater, dialog, and process
plugins. There is no application-owned Rust update service, manifest parser,
download client, or signature verifier.

### State Model

The state machine is:

```text
idle -> checking -> downloading -> ready -> installing
           |              |          |           |
           +--------------+----------+-----------+-> failed
```

The states carry only presentation-safe data:

- `idle`: no check or update activity is visible;
- `checking`: an update check is in flight;
- `downloading`: the target version, downloaded byte count, and optional total
  byte count;
- `ready`: the target version and an internally retained, verified `Update`;
- `installing`: the target version while installation is being handed off;
  and
- `failed`: a retryable failure with an internal stage for diagnostics, but no
  low-level error exposed in the TopBar.

A successful check with no newer version returns to `idle`. Every failure
closes any stale update resource and enters `failed`. Clicking the failure
badge starts a fresh check and download. A successful retry replaces the
failure state; the six-hour timer also remains an automatic retry path.

### Startup, Scheduling, and Concurrency

`App.vue` starts the updater after the first `pulse.load()` settles. Starting
is idempotent and performs an immediate check followed by one check every six
hours. The updater stops and clears its timer when the root component unmounts.

Only one check, download, confirmation, or install transition may be active.
Timer ticks are skipped while another transition is in flight and while the
state is `downloading`, `ready`, or `installing`. A ready update remains stable
until the user accepts it, the application exits, or an installation attempt
fails.

Development, browser-only, and test runs do not contact GitHub. The automatic
lifecycle is enabled only in a production Tauri runtime. Unit tests explicitly
exercise the state machine through mocked Tauri boundaries.

### Download and Verification

After `check()` returns a newer version, the composable calls
`update.download()` rather than `downloadAndInstall()`. Download events update
the accumulated byte count. When a positive content length is available, the
badge displays a clamped integer percentage; otherwise it displays an
indeterminate downloading label.

Tauri's updater verifies the mandatory artifact signature before the download
promise resolves. The state changes to `ready` only after that promise
successfully resolves. The downloaded bytes remain in the updater resource in
the running process. No application-owned file cache is added; exiting before
installation causes the next launch to check and download again.

### Install and Restart

Clicking a ready badge opens a native OK/Cancel confirmation dialog containing
the target version. Cancel leaves the state and verified resource in `ready`
so the badge can be used again.

Accepting changes the state to `installing` and calls Tauri's install API.
Windows hands the update to NSIS and exits/restarts through the updater flow.
On macOS, the install call returns after replacement, so the application calls
the process plugin's relaunch API. A thrown confirmation, installation, or
relaunch error becomes `failed`; retry starts a fresh check and download.

## TopBar Experience

`TopBar.vue` adds a compact update button immediately after the `Codex Pulse`
brand text.

| State | Badge | Interaction |
| --- | --- | --- |
| `idle`, `checking`, current | Hidden | None |
| `downloading` with total | `更新 42%` or locale equivalent | Disabled |
| `downloading` without total | `更新中` or locale equivalent | Disabled |
| `ready` | `更新` or locale equivalent | Opens native confirmation |
| `installing` | `更新中` or locale equivalent | Disabled |
| `failed` | `更新失败` or locale equivalent | Starts an immediate retry |

When the update badge is visible, it replaces the existing active-session
count so the controls do not overflow at the 320-pixel minimum width. At the
narrowest layout, the waveform mark may be hidden, but the full `Codex Pulse`
name and update badge remain. When the badge disappears, the active count
returns.

The button uses the existing TopBar focus treatment. Its label, `title`, and
`aria-label` are localized in Simplified Chinese, English, French, and German.
Changing progress is announced with `aria-live="polite"`. Disabled downloading
and installing states cannot be activated by mouse or keyboard. No raw network,
filesystem, installer, or signature error is rendered.

## Tauri Configuration and Permissions

The application adds the official updater, dialog, and process plugins to both
Rust and JavaScript dependencies and registers them in `app.rs`.

`tauri.conf.json` declares:

- `bundle.createUpdaterArtifacts: true`;
- the updater public key;
- the static endpoint
  `https://github.com/qwertyerge/codex-pulse/releases/latest/download/latest.json`;
  and
- Windows updater `installMode: "passive"`.

The default capability grants only the actions used by this design:

- `updater:allow-check`;
- `updater:allow-download`;
- `updater:allow-install`;
- `dialog:allow-message`; and
- `process:allow-restart`.

The dialog permission intentionally uses `dialog:allow-message`; Tauri's native
`confirm` API is covered by that permission, while the older
`dialog:allow-confirm` identifier is deprecated.

## Signing-Key Security

Generate one signing-key pair dedicated to the Codex Pulse updater.

- The encrypted private key is stored outside the repository at
  `/Users/loki/.tauri/codex-pulse-updater.key` with mode `0600`.
- A randomly generated high-entropy passphrase is stored in macOS Keychain
  under service `Codex Pulse Updater Signing` and account
  `qwertyerge/codex-pulse`.
- The private-key contents and passphrase are written to GitHub Actions Secrets
  named `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- The public key is committed in `tauri.conf.json`.
- Commands and reports must never print the private key or passphrase.
  Verification uses secret names and timestamps only.

GitHub Actions Secrets cannot be read back and are not a backup. Before the
first updater-capable release is tagged, the encrypted private key and its
passphrase must have a separately verified offline backup. This task does not
choose or write to an offline backup medium; the future release checklist must
block publication until that evidence exists. Losing the signing key would
prevent installed clients from accepting later updates.

## Release Workflow

The tag-triggered release workflow keeps its current Draft and two-platform
matrix. The Tauri build receives the two signing Secrets and explicitly
enables:

- `uploadUpdaterJson: true`;
- `uploadUpdaterSignatures: true`; and
- `updaterJsonPreferNsis: true`.

The matrix retains `max-parallel: 1`. Tauri Action updates the one static
`latest.json` as platform jobs finish, so serial uploads avoid two jobs racing
to replace that shared asset.

After both platform jobs succeed, a manifest verification job authenticates to
the Draft Release, downloads `latest.json`, and fails unless:

- its version equals the workflow tag without the leading `v`;
- `platforms.darwin-aarch64` exists with a non-empty URL and signature; and
- `platforms.windows-x86_64` exists with a non-empty URL and signature.

The release remains a Draft after the workflow succeeds. Publication is still
a separate human action after artifact, manifest, installer, and upgrade
acceptance.

## Privacy and Documentation

The English and Simplified Chinese READMEs disclose that production builds:

- contact GitHub Releases after startup and every six hours;
- download a signed installer automatically when a newer version exists;
- do not send Codex transcript, prompt, session, quota, or project-path content
  through the updater; and
- keep session monitoring operational when update checks fail.

The documentation also states the manual bootstrap requirement and corrects
the stale claim that the already published `0.3.2` release is still a Draft.
Normal GitHub request metadata remains subject to GitHub's service and privacy
terms.

## Test Strategy

Implementation follows test-driven development: each behavior starts with a
focused failing test, receives the smallest implementation that passes, and is
then refactored while green.

Frontend tests cover:

- immediate and six-hour checks, idempotent start, timer cleanup, and
  single-flight behavior;
- no-update results and every state transition;
- known and unknown download sizes;
- the rule that only a resolved, signature-verified download becomes `ready`;
- confirmation accept/cancel, install/relaunch, and failures at each stage;
- immediate and scheduled retries;
- TopBar labels, replacement of the active count, disabled/click behavior,
  events, and accessibility attributes;
- App startup and teardown wiring; and
- complete, non-empty updater translations in all four locales.

Repository contract tests cover:

- plugin dependencies and Rust registration;
- precise capability permissions;
- updater public key, endpoint, artifact, and passive-NSIS configuration;
- signing environment variables and Tauri Action inputs;
- continued Draft behavior and serial matrix uploads; and
- the two-platform manifest verification gate.

Fresh final verification includes:

```text
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

A local macOS app bundle is also built with the signing values read without
printing them. Acceptance requires the updater archive and adjacent `.sig`
artifact to exist. `gh secret list` confirms only the two expected secret
names. The final git diff and status are inspected for key material,
passphrases, transcripts, and unrelated changes.

## Acceptance and Rollout Boundary

This task is complete when:

- the approved state machine and TopBar behavior are implemented;
- updater signing and the Draft release workflow are configured;
- both GitHub Secrets exist;
- English and Chinese documentation is current;
- all local frontend, build, Rust, formatting, configuration, and signed
  macOS-artifact checks pass; and
- the worktree remains detached, with no push, pull request, version change,
  tag, Draft, or publication.

The following evidence cannot exist within the approved boundary and must
remain explicitly pending:

- a real Windows updater artifact and `.sig` produced by GitHub Actions;
- a real Draft `latest.json` containing both platforms;
- interactive Windows installation and restart;
- interactive macOS replacement and restart; and
- an actual update from one signed published version to a later signed
  published version.

Before the first updater-capable release, its release checklist must require
the offline signing-key backup, both-platform Draft manifest validation,
manual installation of the bootstrap release, and a signed old-to-new update
rehearsal. Automated checks, Draft creation, publication, and user acceptance
remain distinct gates.

## References

- [Tauri updater guide](https://v2.tauri.app/plugin/updater/)
- [Tauri updater JavaScript API](https://v2.tauri.app/reference/javascript/updater/)
- [Tauri process plugin](https://v2.tauri.app/plugin/process/)
- [Tauri dialog plugin](https://v2.tauri.app/plugin/dialog/)
- [Tauri Action](https://github.com/tauri-apps/tauri-action)
