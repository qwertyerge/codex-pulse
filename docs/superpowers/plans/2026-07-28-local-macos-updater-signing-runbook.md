# Local macOS Updater Signing Runbook Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one maintainer-only macOS command that builds a locally signed updater app and fails unless the source and archived app both pass strict signing and executable-parity checks.

**Architecture:** A Bash runbook owns Keychain lookup, the required default `APPLE_SIGNING_IDENTITY="-"`, the Tauri build, and post-build native verification. A focused Vitest contract locks the script's executable/syntax/security/signing invariants and its two canonical documentation references without requiring CI to possess signing secrets.

**Tech Stack:** Bash 3.2, macOS Keychain, Apple `codesign`, Tauri 2.11, pnpm 10.33.0, Vitest 4, Node.js 24

## Global Constraints

- Work only in `/Users/loki/.codex/worktrees/007c/codex-pulse`, which is an externally managed detached worktree.
- Do not create a branch, push, tag, Draft release, publication, or installer replacement.
- Do not change `.github/workflows/release.yml`; it already supplies `APPLE_SIGNING_IDENTITY`.
- Do not change `README.md`, `docs/README.zh-CN.md`, versions, or the historical acceptance report.
- Never print, persist, or accept the updater password or private-key contents on the command line.
- Default the local Apple signing identity to `-`, while honoring an explicitly supplied `APPLE_SIGNING_IDENTITY`.
- Keep the existing updater key and Keychain defaults overridable only through the approved project-specific environment variables.
- The script may remove only the temporary extraction directory it creates.
- A successful local run remains ad-hoc signed and not notarized; it is not ordinary distribution evidence.

---

## File Map

- Create `scripts/build-local-updater-macos.sh`: the single macOS build-and-verification entry point.
- Create `src/__tests__/localUpdaterBuild.spec.ts`: executable, syntax, security, signing, archive-parity, and documentation contracts.
- Modify `CONTRIBUTING.md`: maintainer-only invocation and evidence boundary.
- Modify `docs/superpowers/plans/2026-07-28-automatic-updates.md`: replace the fragile inline local signing sequence with the canonical script.

### Task 1: Add the failing local runbook contract

**Files:**
- Create: `src/__tests__/localUpdaterBuild.spec.ts`
- Test: `src/__tests__/localUpdaterBuild.spec.ts`

**Interfaces:**
- Consumes: repository root from `process.cwd()`.
- Produces: a contract for `scripts/build-local-updater-macos.sh`, `CONTRIBUTING.md`, and `docs/superpowers/plans/2026-07-28-automatic-updates.md`.

- [ ] **Step 1: Create the complete contract test before the script exists**

Create `src/__tests__/localUpdaterBuild.spec.ts` with:

```ts
import {
  constants,
  existsSync,
  readFileSync,
  statSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const repositoryRoot = process.cwd();
const scriptPath = resolve(
  repositoryRoot,
  "scripts/build-local-updater-macos.sh",
);
const contributingPath = resolve(repositoryRoot, "CONTRIBUTING.md");
const automaticUpdatePlanPath = resolve(
  repositoryRoot,
  "docs/superpowers/plans/2026-07-28-automatic-updates.md",
);

function read(path: string) {
  return readFileSync(path, "utf8");
}

describe("local macOS updater signing runbook", () => {
  it("is executable, valid Bash, and exposes side-effect-free help", () => {
    expect(existsSync(scriptPath)).toBe(true);
    expect(statSync(scriptPath).mode & constants.S_IXUSR).toBeTruthy();

    const syntax = spawnSync("/bin/bash", ["-n", scriptPath], {
      cwd: repositoryRoot,
      encoding: "utf8",
    });
    expect(syntax.status).toBe(0);
    expect(syntax.stderr).toBe("");

    const help = spawnSync("/bin/bash", [scriptPath, "--help"], {
      cwd: repositoryRoot,
      encoding: "utf8",
      env: {
        PATH: "",
      },
    });
    expect(help.status).toBe(0);
    expect(help.stderr).toBe("");
    expect(help.stdout).toContain(
      "Build and verify the local macOS updater bundle",
    );
    expect(help.stdout).toContain("CODEX_PULSE_UPDATER_KEY_PATH");
    expect(help.stdout).toContain(
      "CODEX_PULSE_UPDATER_KEYCHAIN_SERVICE",
    );
    expect(help.stdout).toContain(
      "CODEX_PULSE_UPDATER_KEYCHAIN_ACCOUNT",
    );
    expect(help.stdout).toContain("APPLE_SIGNING_IDENTITY");
  });

  it("locks secret handling and the pre-archive signing identity", () => {
    const script = read(scriptPath);

    expect(script).toContain("set -euo pipefail");
    expect(script).not.toMatch(/set\s+-[^\\n]*x/);
    expect(script).toContain(
      'apple_signing_identity="${APPLE_SIGNING_IDENTITY:--}"',
    );
    expect(script).toContain(
      "security find-generic-password -w",
    );
    expect(script).toContain(
      'TAURI_SIGNING_PRIVATE_KEY="$updater_key_path"',
    );
    expect(script).toContain(
      'TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$updater_key_password"',
    );
    expect(script).toContain('trap cleanup EXIT');
    expect(script).toContain('unset updater_key_password');
    expect(script).not.toContain("TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$1");
  });

  it("fails closed on source and archived app verification", () => {
    const script = read(scriptPath);

    expect(
      script.match(/codesign --verify --deep --strict/g),
    ).toHaveLength(2);
    expect(script).not.toContain("codesign --force");
    expect(script).toContain(
      'expected_bundle_identifier="com.codexpulse.desktop"',
    );
    expect(script).toContain('expected_architecture="arm64"');
    expect(script).toContain('if [[ "$key_mode" != "600" ]]');
    expect(script).toContain('require("./package.json").version');
    expect(script).toContain(
      'require("./src-tauri/tauri.conf.json").version',
    );
    expect(script).toContain(
      '"$repository_root/src-tauri/Cargo.toml"',
    );
    expect(script).toContain("Print :CFBundleShortVersionString");
    expect(script).toContain(
      'archive_file="$bundle_directory/Codex Pulse.app.tar.gz"',
    );
    expect(script).toContain('test -s "$archive_file"');
    expect(script).toContain('test -s "$signature_file"');
    expect(script).toContain("mktemp -d");
    expect(script).toContain('tar -xzf "$archive_file"');
    expect(script).toContain('test "$source_hash" = "$archive_hash"');
    expect(script).toContain('rm -rf -- "$audit_directory"');
  });

  it("keeps both maintainer documents on the canonical entry point", () => {
    const contributing = read(contributingPath);
    const automaticUpdatePlan = read(automaticUpdatePlanPath);
    const command = "scripts/build-local-updater-macos.sh";

    expect(contributing).toContain(command);
    expect(contributing).toContain("ad-hoc");
    expect(contributing).toContain("notarized");
    expect(automaticUpdatePlan).toContain(command);
    expect(automaticUpdatePlan).toContain(
      "codesign --verify --deep --strict",
    );
    expect(automaticUpdatePlan).not.toContain(
      'TAURI_SIGNING_PRIVATE_KEY="$UPDATER_KEY_PATH" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$UPDATER_KEY_PASSWORD" pnpm tauri build --bundles app',
    );
  });
});
```

- [ ] **Step 2: Run the focused contract to prove RED**

Run:

```bash
pnpm test -- src/__tests__/localUpdaterBuild.spec.ts
```

Expected: FAIL because
`scripts/build-local-updater-macos.sh` does not exist and neither maintainer
document references it. Preserve the exact failure output in the task report.

### Task 2: Implement the hardened macOS build-and-verification script

**Files:**
- Create: `scripts/build-local-updater-macos.sh`
- Test: `src/__tests__/localUpdaterBuild.spec.ts`

**Interfaces:**
- Consumes:
  - `CODEX_PULSE_UPDATER_KEY_PATH`, defaulting to
    `$HOME/.tauri/codex-pulse-updater.key`;
  - `CODEX_PULSE_UPDATER_KEYCHAIN_SERVICE`, defaulting to
    `Codex Pulse Updater Signing`;
  - `CODEX_PULSE_UPDATER_KEYCHAIN_ACCOUNT`, defaulting to
    `qwertyerge/codex-pulse`; and
  - `APPLE_SIGNING_IDENTITY`, defaulting to `-`.
- Produces:
  - `src-tauri/target/release/bundle/macos/Codex Pulse.app`;
  - `src-tauri/target/release/bundle/macos/Codex Pulse.app.tar.gz`;
  - `src-tauri/target/release/bundle/macos/Codex Pulse.app.tar.gz.sig`; and
  - non-secret paths, sizes, versions, architecture, and SHA-256 evidence.

- [ ] **Step 1: Create the script with side-effect-free help and fail-closed prerequisites**

Create `scripts/build-local-updater-macos.sh`:

```bash
#!/usr/bin/env bash

set -euo pipefail

usage() {
  printf '%s\n' \
    "Build and verify the local macOS updater bundle." \
    "" \
    "Environment overrides:" \
    "  CODEX_PULSE_UPDATER_KEY_PATH" \
    "  CODEX_PULSE_UPDATER_KEYCHAIN_SERVICE" \
    "  CODEX_PULSE_UPDATER_KEYCHAIN_ACCOUNT" \
    "  APPLE_SIGNING_IDENTITY (defaults to -)"
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$#" -ne 0 ]]; then
  usage >&2
  exit 2
fi

if [[ "$(/usr/bin/uname -s)" != "Darwin" ]]; then
  printf 'This runbook supports macOS only.\n' >&2
  exit 1
fi

script_directory="$(
  cd "$(/usr/bin/dirname "${BASH_SOURCE[0]}")"
  pwd -P
)"
repository_root="$(
  cd "$script_directory/.."
  pwd -P
)"

updater_key_path="${CODEX_PULSE_UPDATER_KEY_PATH:-$HOME/.tauri/codex-pulse-updater.key}"
updater_keychain_service="${CODEX_PULSE_UPDATER_KEYCHAIN_SERVICE:-Codex Pulse Updater Signing}"
updater_keychain_account="${CODEX_PULSE_UPDATER_KEYCHAIN_ACCOUNT:-qwertyerge/codex-pulse}"
apple_signing_identity="${APPLE_SIGNING_IDENTITY:--}"
updater_key_password=""
audit_directory=""

cleanup() {
  unset updater_key_password
  if [[ -n "$audit_directory" && -d "$audit_directory" ]]; then
    case "$audit_directory" in
      "${TMPDIR:-/tmp}"/codex-pulse-updater-audit.*)
        /bin/rm -rf -- "$audit_directory"
        ;;
      *)
        printf 'Refusing to remove unexpected audit directory: %s\n' \
          "$audit_directory" >&2
        ;;
    esac
  fi
}
trap cleanup EXIT

for required_command in \
  /bin/rm \
  /usr/bin/awk \
  /usr/bin/codesign \
  /usr/bin/dirname \
  /usr/bin/file \
  /usr/bin/head \
  /usr/bin/mktemp \
  /usr/bin/sed \
  /usr/bin/security \
  /usr/bin/shasum \
  /usr/bin/stat \
  /usr/bin/tar \
  /usr/bin/uname \
  /usr/libexec/PlistBuddy \
  node \
  pnpm
do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    printf 'Required command is unavailable: %s\n' \
      "$required_command" >&2
    exit 1
  fi
done

test -s "$updater_key_path"
key_mode="$(/usr/bin/stat -f '%Lp' "$updater_key_path")"
if [[ "$key_mode" != "600" ]]; then
  printf 'Updater key mode must be 600, observed %s.\n' \
    "$key_mode" >&2
  exit 1
fi

updater_key_password="$(
  /usr/bin/security find-generic-password \
    -w \
    -a "$updater_keychain_account" \
    -s "$updater_keychain_service"
)"
test -n "$updater_key_password"
```

- [ ] **Step 2: Add the build with the signing identity present before packaging**

Append:

```bash
(
  cd "$repository_root"
  APPLE_SIGNING_IDENTITY="$apple_signing_identity" \
    TAURI_SIGNING_PRIVATE_KEY="$updater_key_path" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$updater_key_password" \
    pnpm tauri build --bundles app
)
unset updater_key_password
```

The identity and updater credentials must be on the same `pnpm tauri build`
invocation. Do not add a post-archive re-signing step.

- [ ] **Step 3: Add exact version, identifier, architecture, archive, signature, and parity gates**

Append:

```bash
bundle_directory="$repository_root/src-tauri/target/release/bundle/macos"
source_app="$bundle_directory/Codex Pulse.app"
archive_file="$bundle_directory/Codex Pulse.app.tar.gz"
signature_file="$archive_file.sig"
source_executable="$source_app/Contents/MacOS/CodexPulse"
expected_bundle_identifier="com.codexpulse.desktop"
expected_architecture="arm64"

test -d "$source_app"
test -x "$source_executable"
test -s "$archive_file"
test -s "$signature_file"

/usr/bin/codesign --verify --deep --strict --verbose=4 "$source_app"

bundle_identifier="$(
  /usr/libexec/PlistBuddy \
    -c 'Print :CFBundleIdentifier' \
    "$source_app/Contents/Info.plist"
)"
test "$bundle_identifier" = "$expected_bundle_identifier"

package_version="$(
  cd "$repository_root"
  node -p 'require("./package.json").version'
)"
tauri_version="$(
  cd "$repository_root"
  node -p 'require("./src-tauri/tauri.conf.json").version'
)"
cargo_version="$(
  /usr/bin/sed -n \
    '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' \
    "$repository_root/src-tauri/Cargo.toml" |
    /usr/bin/head -n 1
)"
bundle_version="$(
  /usr/libexec/PlistBuddy \
    -c 'Print :CFBundleShortVersionString' \
    "$source_app/Contents/Info.plist"
)"
test "$package_version" = "$tauri_version"
test "$package_version" = "$cargo_version"
test "$package_version" = "$bundle_version"

file_description="$(/usr/bin/file "$source_executable")"
case "$file_description" in
  *"Mach-O 64-bit executable $expected_architecture"*)
    ;;
  *)
    printf 'Unexpected app architecture: %s\n' \
      "$file_description" >&2
    exit 1
    ;;
esac

audit_directory="$(
  /usr/bin/mktemp \
    -d \
    "${TMPDIR:-/tmp}/codex-pulse-updater-audit.XXXXXX"
)"
/usr/bin/tar -xzf "$archive_file" -C "$audit_directory"
archive_app="$audit_directory/Codex Pulse.app"
archive_executable="$archive_app/Contents/MacOS/CodexPulse"

test -d "$archive_app"
test -x "$archive_executable"
/usr/bin/codesign --verify --deep --strict --verbose=4 "$archive_app"

source_hash="$(
  /usr/bin/shasum -a 256 "$source_executable" |
    /usr/bin/awk '{print $1}'
)"
archive_hash="$(
  /usr/bin/shasum -a 256 "$archive_executable" |
    /usr/bin/awk '{print $1}'
)"
test "$source_hash" = "$archive_hash"

printf 'bundle=%s\n' "$source_app"
printf 'version=%s\n' "$bundle_version"
printf 'architecture=%s\n' "$expected_architecture"
printf 'archive=%s size=%s\n' \
  "$archive_file" \
  "$(/usr/bin/stat -f '%z' "$archive_file")"
printf 'signature=%s size=%s\n' \
  "$signature_file" \
  "$(/usr/bin/stat -f '%z' "$signature_file")"
printf 'bundle_and_archive_executable_sha256=%s\n' \
  "$source_hash"
```

Use `/usr/bin/awk` in the actual script and include it in the prerequisite
list. Use `/usr/bin/mktemp` in the prerequisite list as well.

- [ ] **Step 4: Make the script executable and run the focused contract**

Run:

```bash
chmod 755 scripts/build-local-updater-macos.sh
pnpm test -- src/__tests__/localUpdaterBuild.spec.ts
```

Expected: the script contracts pass; only the documentation-reference contract
remains RED until Task 3.

### Task 3: Point maintainer documentation at the canonical runbook

**Files:**
- Modify: `CONTRIBUTING.md`
- Modify: `docs/superpowers/plans/2026-07-28-automatic-updates.md`
- Test: `src/__tests__/localUpdaterBuild.spec.ts`

**Interfaces:**
- Consumes: executable `scripts/build-local-updater-macos.sh`.
- Produces: one canonical maintainer invocation with no duplicated Keychain or
  signing environment sequence.

- [ ] **Step 1: Add the maintainer-only section to `CONTRIBUTING.md`**

After the Windows NSIS verification paragraph, add:

````markdown
### Maintainer-only macOS updater build

Maintainers with the encrypted updater key and matching macOS Keychain item
can build and verify the local updater bundle with:

```bash
scripts/build-local-updater-macos.sh
```

The script supplies the local ad-hoc signing identity before Tauri creates the
updater archive, then strictly verifies both the source app and the app inside
the archive. It also requires their executable hashes to match. This is local
updater-integrity evidence only: the app remains ad-hoc signed and is not
Developer ID signed or notarized.
````

- [ ] **Step 2: Replace the fragile inline command in the automatic-update plan**

In Step 2 of
`docs/superpowers/plans/2026-07-28-automatic-updates.md`, replace the Keychain
variables, password command substitution, `pnpm tauri build`, archive-count
commands, and their old expectation with:

````markdown
Run the canonical maintainer-only build and verification entry point without
shell tracing:

```bash
scripts/build-local-updater-macos.sh
```

Expected: the command exits `0` only after the source app and the app extracted
from `Codex Pulse.app.tar.gz` both pass
`codesign --verify --deep --strict`, the adjacent `.sig` is non-empty, all
repository and bundle versions match, the executable is ARM64, and the source
and archived executable SHA-256 values are identical.

This proves local updater artifact generation, strict ad-hoc bundle validity,
and archive parity. It does not prove Developer ID signing, notarization,
installation, restart, publication, or cross-version updating.
````

- [ ] **Step 3: Run the focused contract to prove GREEN**

Run:

```bash
pnpm test -- src/__tests__/localUpdaterBuild.spec.ts
```

Expected: all four local runbook contract tests pass.

- [ ] **Step 4: Commit the contract, script, and documentation together**

Run:

```bash
git add \
  scripts/build-local-updater-macos.sh \
  src/__tests__/localUpdaterBuild.spec.ts \
  CONTRIBUTING.md \
  docs/superpowers/plans/2026-07-28-automatic-updates.md
git diff --cached --check
git commit -m "fix: harden local updater signing verification"
```

Expected: the commit contains exactly the four listed files.

### Task 4: Execute the real signed runbook and complete verification

**Files:**
- Verify: `scripts/build-local-updater-macos.sh`
- Verify: repository and generated build outputs

**Interfaces:**
- Consumes: the existing encrypted key at
  `/Users/loki/.tauri/codex-pulse-updater.key` and Keychain service/account
  defaults.
- Produces: fresh non-secret signing, artifact, parity, automated-test, and Git
  evidence.

- [ ] **Step 1: Confirm the user-installed app remains the only running instance**

Run a read-only process query and confirm that no executable from the worktree
bundle is running. Do not stop `/Applications/Codex Pulse.app`.

Expected: only `/Applications/Codex Pulse.app/Contents/MacOS/CodexPulse` may be
present.

- [ ] **Step 2: Run the real script without printing secrets**

Run:

```bash
scripts/build-local-updater-macos.sh
```

Expected:

- the Tauri build reports signing with identity `-`;
- both strict codesign checks succeed;
- version is `0.3.2`;
- architecture is `arm64`;
- archive and signature sizes are positive; and
- the printed source/archive executable SHA-256 is one shared value.

Do not run with `set -x`, do not print the Keychain value, and do not copy the
password into a command argument.

- [ ] **Step 3: Run the complete automated verification**

Run:

```bash
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected:

- all frontend test files and tests pass;
- TypeScript and Vite production build pass;
- Rust formatting has no differences;
- all Rust tests pass; and
- Git whitespace validation passes.

- [ ] **Step 4: Verify final Git provenance and exact commit contents**

Run:

```bash
git status --short --branch
git rev-parse --short=12 HEAD
git rev-parse --short=12 origin/main
git rev-list --left-right --count origin/main...HEAD
git show --stat --oneline --no-renames HEAD
git show --format= --name-only HEAD
```

Expected:

- detached HEAD and clean worktree;
- the implementation commit is ahead of the design and original automatic
  updater commits;
- no branch, push, tag, Draft, publication, or installation occurred; and
- the implementation commit contains exactly the script, focused contract,
  `CONTRIBUTING.md`, and the automatic-update plan.
