# Automatic Updates Acceptance Evidence

Date: 2026-07-28
Repository: `qwertyerge/codex-pulse`

## Scope and Git State

- Base commit: `51fcaa65834e6200fb115207e58a5e61500edcf1`
  (`0.3.2`, `origin/main` at verification time).
- Verified implementation HEAD:
  `cfc0e9d5cbd9544f4f378cc3d0574034eba92b0b`.
- Checkout state: detached HEAD (`## HEAD (no branch)`).
- `package.json`, `src-tauri/tauri.conf.json`, and
  `src-tauri/Cargo.toml` all remain at version `0.3.2`.
- No branch, push, tag, Draft release, publication, or pull request was
  created by this work.

## Fresh Automated Verification

| Command | Exit | Observed result |
| --- | ---: | --- |
| `pnpm test` | 0 | 24 test files passed; 122 tests passed; 0 failed |
| `pnpm build` | 0 | `vue-tsc --noEmit` and Vite production build passed; 1,829 modules transformed |
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | 0 | No formatting differences |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 0 | 81 Rust unit tests passed; 0 failed; auxiliary and doc-test targets also passed |

The final `git diff --check` also exited `0`.

## Signed Tauri Boundary

- The generated public key bytes exactly match
  `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.
- The encrypted private key exists at
  `$HOME/.tauri/codex-pulse-updater.key` with mode `600`.
- The public key exists at
  `$HOME/.tauri/codex-pulse-updater.key.pub` with mode `644`.
- `git ls-files` contains no `codex-pulse-updater.key` path.
- The desktop key-generation script wrote the passphrase to the login
  Keychain and compared a read-back value before reporting success.
- A separate desktop signed-build script subsequently read that Keychain item
  and successfully generated the updater archive and signature. The sandboxed
  command process could not independently query the desktop Keychain item
  (exit `44`), so Keychain evidence is explicitly limited to those two
  successful desktop-script runs.

Observed local updater artifacts:

- `src-tauri/target/release/bundle/macos/Codex Pulse.app.tar.gz`
  — 6,525,185 bytes.
- `src-tauri/target/release/bundle/macos/Codex Pulse.app.tar.gz.sig`
  — 412 bytes.

An independent filesystem check found exactly one `.app.tar.gz` and one
`.app.tar.gz.sig`. This proves local Tauri updater artifact generation only;
it does not prove installation, restart, Apple notarization, publication, or
cross-version updating.

## 320-Pixel TopBar Browser Check

The Vite page was rendered with the real `src/styles.css` at a `320x420`
viewport. Development mode deliberately disables updater network access, so
the approved failed-badge DOM fixture was injected only into the temporary
local browser tab. The injection was removed when the tab was closed.

Measured values:

```text
viewportWidth: 320
viewportHeight: 420
documentWidth: 320
brandRight: 158
controlsLeft: 164
overlap: false
fullName: Codex Pulse
nameClipped: false
badgeClipped: false
markDisplay: none
```

Visual and computed-style observations:

- Light theme: failed badge remained readable with foreground
  `rgb(168, 33, 50)` over `rgba(255, 221, 225, 0.62)`.
- Dark theme after its 160 ms background transition: failed badge remained
  readable with foreground `rgb(255, 158, 170)` over
  `rgba(103, 32, 45, 0.5)`.
- In both themes, the focused badge exposed a visible `3px solid` focus
  outline.
- The browser run was a layout/style check only. Plain Vite does not provide
  the native Tauri IPC bridge, so its expected IPC errors are not native app
  runtime evidence.

The temporary viewport override was reset, the browser tab was finalized, and
the Vite process was stopped after the measurement.

## GitHub Actions Secrets

Read-only `gh secret list` verification returned exactly:

| Secret | Updated at |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | `2026-07-28T03:27:13Z` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | `2026-07-28T03:27:14Z` |

No secret value was printed or recorded in this report. GitHub Secrets are
write-only build inputs and are not the required offline backup.

## Evidence Status

| Area | Current evidence |
| --- | --- |
| Frontend and Rust implementation | Fresh local suites and production build passed |
| Updater configuration | Parsed configuration contract passed; native plugins compiled |
| Local macOS updater bundle | One archive and adjacent signature generated and independently counted |
| Release workflow | Parsed YAML contract and manifest-validator fixtures passed |
| GitHub Secrets | Both required names and update timestamps observed through the GitHub API |
| Native install/restart and release delivery | Not yet verified |

## Pending Release Gates

The following remain pending and must not be inferred from the evidence above:

- Separately verified offline backup of the encrypted key and its passphrase.
- First updater-capable version preparation and manual bootstrap installation.
- A real Windows updater artifact and adjacent signature.
- A real authenticated Draft `latest.json` containing both supported
  platforms.
- Interactive macOS and Windows installation and restart.
- A signed old-version-to-new-version update rehearsal on both platforms.
- Publication and a post-publication update check.

The first updater-capable release must remain a separately approved task. A
Draft workflow success, publication, updater reachability, native restart, and
user acceptance are distinct gates.
