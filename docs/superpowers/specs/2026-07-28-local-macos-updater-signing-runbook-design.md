# Local macOS Updater Signing Runbook Design

## Context

Codex Pulse's GitHub release workflow already sets
`APPLE_SIGNING_IDENTITY` to `-` for the macOS matrix job. The local updater
acceptance command does not. A fresh native smoke test proved that omitting the
pseudo-identity produces an ad-hoc app bundle that fails
`codesign --verify --deep --strict` with an incomplete resource seal. Repeating
the same build with `APPLE_SIGNING_IDENTITY="-"` produced a strictly valid
bundle before Tauri created the updater archive and signature.

The existing local acceptance instructions also count the `.app.tar.gz` and
`.sig` files without validating the app embedded in the archive. That can
report success even when the local app bundle is not a valid macOS bundle.

## Goal

Provide one maintainer-only macOS command that:

- obtains the updater signing password without printing it;
- builds the updater app with a complete local ad-hoc signature by default;
- verifies both the source app and the app embedded in the updater archive;
- proves that the source and archived executables are identical; and
- fails closed when any signing, artifact, or parity check fails.

## Non-Goals

- Changing the GitHub Actions release workflow, which already supplies the
  macOS signing identity correctly.
- Developer ID signing, Apple notarization, stapling, Gatekeeper acceptance, or
  ordinary third-party distribution.
- Installing or replacing `/Applications/Codex Pulse.app`.
- Creating a tag, Draft release, manifest, or public release.
- Verifying Windows artifacts, native restart, or an old-to-new update.
- Publishing project signing material or adding maintainer Keychain details to
  the public build-from-source instructions.

## Canonical Entry Point

Add the executable script:

```text
scripts/build-local-updater-macos.sh
```

The script supports macOS only and uses `set -euo pipefail`. It never enables
shell tracing. With no arguments, it:

1. resolves the encrypted updater key path;
2. reads the updater password from macOS Keychain;
3. defaults `APPLE_SIGNING_IDENTITY` to `-`;
4. invokes `pnpm tauri build --bundles app`; and
5. runs all local updater artifact acceptance checks.

`--help` prints usage without reading Keychain or starting a build.

### Configuration

The default values are:

| Setting | Default |
| --- | --- |
| Encrypted key | `$HOME/.tauri/codex-pulse-updater.key` |
| Keychain service | `Codex Pulse Updater Signing` |
| Keychain account | `qwertyerge/codex-pulse` |
| Apple signing identity | `-` |

`CODEX_PULSE_UPDATER_KEY_PATH`,
`CODEX_PULSE_UPDATER_KEYCHAIN_SERVICE`, and
`CODEX_PULSE_UPDATER_KEYCHAIN_ACCOUNT` may override the corresponding
defaults. An explicitly supplied
`APPLE_SIGNING_IDENTITY` may replace the ad-hoc default for a maintainer with a
real signing identity. The script must not accept the password as a
command-line argument, where it would be exposed through shell history or
process inspection.

The password exists only in the script process and the Tauri build child
environment. An `EXIT` trap unsets its password variable and removes any
script-owned temporary directory. The script never prints the password or
private-key contents.

## Build and Verification Flow

The script validates prerequisites before starting a build:

- the host is macOS;
- the encrypted key exists and is non-empty;
- the key is not group- or world-readable;
- `security`, `codesign`, `pnpm`, `tar`, and the required checksum and plist
  tools are available; and
- the Keychain query returns a non-empty password.

The build receives:

```text
APPLE_SIGNING_IDENTITY
TAURI_SIGNING_PRIVATE_KEY
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

After Tauri succeeds, the script validates the exact macOS app and updater
artifact paths under `src-tauri/target/release/bundle/macos`:

1. the source `Codex Pulse.app` passes
   `codesign --verify --deep --strict`;
2. its bundle identifier is `com.codexpulse.desktop`;
3. its version matches the equal versions in `package.json`,
   `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`;
4. its executable is an ARM64 Mach-O;
5. the exact `Codex Pulse.app.tar.gz` and adjacent
   `Codex Pulse.app.tar.gz.sig` exist and are non-empty;
6. the archive is extracted into a script-owned temporary directory;
7. the archived app passes the same strict codesign check; and
8. the source and archived `CodexPulse` executable SHA-256 values are equal.

The script removes only its temporary extraction directory. It retains all
build outputs for inspection. Successful output is limited to non-secret
metadata: artifact paths, sizes, version, architecture, and the shared
executable hash.

## Failure Behavior

Every missing prerequisite, Keychain lookup failure, build failure, signature
failure, missing or empty artifact, malformed archive, identifier/version/
architecture mismatch, or executable hash mismatch terminates the script with
a non-zero exit code.

The script does not repair or re-sign a failed bundle after Tauri packaging.
The identity must be present during the Tauri build so that the updater
archive contains the same already-valid app that was locally inspected.

## Documentation Boundary

`CONTRIBUTING.md` gains a maintainer-only local updater verification section
that invokes the script and explains its evidence boundary. The existing
automatic-update implementation plan replaces its incorrect inline Keychain
and build commands with the canonical script.

The public English and Chinese build-from-source sections remain unchanged.
They describe ordinary local builds for contributors who do not have the
project updater key. The existing acceptance report remains a historical
record of its original evidence timestamp rather than being rewritten.

## Test Strategy

Implementation follows test-driven development.

A focused Vitest contract first fails because the script does not exist. It
copies the real script into a temporary repository fixture and executes it
against controlled Keychain, Tauri-build, codesign, plist, architecture, and
BSD-stat command boundaries. The contract asserts observable behavior rather
than grepping the script source:

- `--help` succeeds without a toolchain, Keychain lookup, or build;
- the default Apple identity is `-`, and an explicit identity is honored;
- the build receives the key path and a canary Keychain password without
  exposing either secret value in output;
- failures from either source or archived-app strict codesign check propagate
  as non-zero exits;
- an insecure key mode, empty updater signature, or source/archive executable
  mismatch fails closed;
- a valid fixture produces the expected non-secret evidence; and
- script-owned extraction directories are removed after success and failure.

The external command doubles provide controlled inputs and failures; tests
assert the real runbook's exit status, output, artifacts, and cleanup rather
than asserting calls on the doubles. Human documentation is reviewed directly
and is not protected by brittle source-text assertions.

Final verification includes:

```text
pnpm test -- src/__tests__/localUpdaterBuild.spec.ts
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
scripts/build-local-updater-macos.sh
git diff --check
```

The real script execution must again prove strict validity of both the source
and archived app. It does not close notarization, installation, tray reopen,
Windows, Draft-manifest, or cross-version-update gates.

## Acceptance Criteria

- Maintainers have one executable macOS updater build-and-verify entry point.
- The local default always supplies the ad-hoc pseudo-identity before Tauri
  creates the updater archive.
- Secret values are never printed, persisted, or accepted on the command
  line.
- A successful script run proves strict signing and executable parity for the
  source and archived app.
- Focused and complete automated checks pass.
- The worktree remains detached and no branch, push, tag, Draft, publication,
  or installation occurs.
