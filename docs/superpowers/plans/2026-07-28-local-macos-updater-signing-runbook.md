# Local macOS Updater Signing Runbook Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one maintainer-only macOS command that builds a locally signed updater app and fails unless the source and archived app both pass strict signing and executable-parity checks.

**Architecture:** A Bash runbook owns Keychain lookup, the required default `APPLE_SIGNING_IDENTITY="-"`, the Tauri build, and post-build native verification. A focused Vitest contract copies and executes the real runbook in a temporary repository while replacing only external platform/build boundaries; it asserts exit status, non-secret output, artifact parity, and cleanup rather than source text or mock calls.

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
- Test human documentation through review, not brittle source-text assertions.

---

## File Map

- Create `scripts/build-local-updater-macos.sh`: the single macOS build-and-verification entry point.
- Create `src/__tests__/localUpdaterBuild.spec.ts`: isolated behavior contracts for success and every fail-closed boundary.
- Modify `CONTRIBUTING.md`: maintainer-only invocation and evidence boundary.
- Modify `docs/superpowers/plans/2026-07-28-automatic-updates.md`: replace the fragile inline local signing sequence with the canonical script.

### Task 1: Add the failing isolated behavior contract

**Files:**
- Create: `src/__tests__/localUpdaterBuild.spec.ts`
- Test: `src/__tests__/localUpdaterBuild.spec.ts`

**Interfaces:**
- Consumes: the production script at `scripts/build-local-updater-macos.sh`.
- Produces: temporary repository fixtures whose fake Keychain, Tauri build,
  codesign, plist, file, and BSD-stat commands provide controlled external
  outcomes to the real script.

- [ ] **Step 1: Create the complete behavioral contract before the script exists**

Create `src/__tests__/localUpdaterBuild.spec.ts`:

```ts
import {
  chmodSync,
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";

interface TextResult {
  status: number | null;
  stdout: string;
  stderr: string;
}

interface HarnessOptions {
  appleIdentity?: string;
  archiveMismatch?: boolean;
  codesignFailureCall?: number;
  emptySignature?: boolean;
  keyMode?: string;
}

interface Harness {
  auditRoot: string;
  keyCanary: string;
  passwordCanary: string;
  run: () => TextResult;
}

const repositoryRoot = process.cwd();
const sourceScriptPath = resolve(
  repositoryRoot,
  "scripts/build-local-updater-macos.sh",
);
const temporaryFixtures: string[] = [];

function writeExecutable(path: string, contents: string) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents.endsWith("\n") ? contents : `${contents}\n`);
  chmodSync(path, 0o755);
}

function writeStub(directory: string, name: string, body: string) {
  writeExecutable(
    join(directory, name),
    `#!/usr/bin/env bash\nset -euo pipefail\n${body}`,
  );
}

function runText(
  command: string,
  args: string[],
  options: {
    cwd: string;
    env: NodeJS.ProcessEnv;
  },
): TextResult {
  const result = spawnSync(command, args, {
    ...options,
    encoding: "utf8",
  });
  return {
    status: result.status,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function requireSourceScript() {
  expect(
    existsSync(sourceScriptPath),
    "the local updater runbook should exist",
  ).toBe(true);
  return sourceScriptPath;
}

function createHarness(options: HarnessOptions = {}): Harness {
  const fixtureRoot = mkdtempSync(
    join(tmpdir(), "codex-pulse-updater-runbook-"),
  );
  temporaryFixtures.push(fixtureRoot);

  const fixtureScript = join(
    fixtureRoot,
    "scripts/build-local-updater-macos.sh",
  );
  mkdirSync(dirname(fixtureScript), { recursive: true });
  copyFileSync(requireSourceScript(), fixtureScript);
  chmodSync(fixtureScript, 0o755);

  mkdirSync(join(fixtureRoot, "src-tauri"), { recursive: true });
  writeFileSync(
    join(fixtureRoot, "package.json"),
    '{"version":"0.3.2"}\n',
  );
  writeFileSync(
    join(fixtureRoot, "src-tauri/tauri.conf.json"),
    '{"version":"0.3.2"}\n',
  );
  writeFileSync(
    join(fixtureRoot, "src-tauri/Cargo.toml"),
    '[package]\nname = "fixture"\nversion = "0.3.2"\n',
  );

  const keyCanary = "encrypted-private-key-canary";
  const passwordCanary = "keychain-password-canary";
  const updaterKeyPath = join(fixtureRoot, "updater.key");
  writeFileSync(updaterKeyPath, keyCanary);
  chmodSync(updaterKeyPath, 0o600);

  const fakeBin = join(fixtureRoot, "fake-bin");
  const fakeState = join(fixtureRoot, "fake-state");
  const auditRoot = join(fixtureRoot, "audit-root");
  mkdirSync(fakeBin);
  mkdirSync(fakeState);
  mkdirSync(auditRoot);

  writeStub(fakeBin, "uname", 'printf "Darwin\\n"');
  writeStub(
    fakeBin,
    "security",
    [
      'test "$*" = "find-generic-password -w -a qwertyerge/codex-pulse -s Codex Pulse Updater Signing"',
      'printf "%s\\n" "$FAKE_PASSWORD"',
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "codesign",
    [
      'count_file="$FAKE_STATE_DIRECTORY/codesign-count"',
      "count=0",
      'if [[ -f "$count_file" ]]; then count="$(<"$count_file")"; fi',
      "count=$((count + 1))",
      'printf "%s\\n" "$count" > "$count_file"',
      'if [[ "${FAKE_CODESIGN_FAIL_CALL:-0}" = "$count" ]]; then exit 41; fi',
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "file",
    'printf "%s: Mach-O 64-bit executable arm64\\n" "$1"',
  );
  writeStub(
    fakeBin,
    "plutil",
    [
      'case "$2" in',
      '  CFBundleIdentifier) printf "com.codexpulse.desktop\\n" ;;',
      '  CFBundleShortVersionString) printf "0.3.2\\n" ;;',
      "  *) exit 42 ;;",
      "esac",
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "stat",
    [
      'case "$2" in',
      '  %Lp) printf "%s\\n" "${FAKE_KEY_MODE:-600}" ;;',
      '  %z) wc -c < "$3" | tr -d " " ;;',
      "  *) exit 43 ;;",
      "esac",
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "shasum",
    [
      'test "$1" = "-a"',
      'test "$2" = "256"',
      'cksum "$3" | awk -v file="$3" \'{printf "%s-%s  %s\\n", $1, $2, file}\'',
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "pnpm",
    [
      'test "$*" = "tauri build --bundles app"',
      'test "${APPLE_SIGNING_IDENTITY:-}" = "$FAKE_EXPECTED_APPLE_IDENTITY"',
      'test "$TAURI_SIGNING_PRIVATE_KEY" = "$CODEX_PULSE_UPDATER_KEY_PATH"',
      'test "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" = "$FAKE_PASSWORD"',
      'bundle_directory="$PWD/src-tauri/target/release/bundle/macos"',
      'source_app="$bundle_directory/Codex Pulse.app"',
      'source_executable="$source_app/Contents/MacOS/CodexPulse"',
      'mkdir -p "$(dirname "$source_executable")"',
      'printf "fixture-binary\\n" > "$source_executable"',
      'chmod 755 "$source_executable"',
      'printf "<plist/>\\n" > "$source_app/Contents/Info.plist"',
      '(cd "$bundle_directory" && tar -czf "Codex Pulse.app.tar.gz" "Codex Pulse.app")',
      'if [[ "${FAKE_ARCHIVE_MISMATCH:-0}" = "1" ]]; then printf "changed\\n" >> "$source_executable"; fi',
      'if [[ "${FAKE_EMPTY_SIGNATURE:-0}" = "1" ]]; then',
      '  : > "$bundle_directory/Codex Pulse.app.tar.gz.sig"',
      "else",
      '  printf "fixture-signature\\n" > "$bundle_directory/Codex Pulse.app.tar.gz.sig"',
      "fi",
    ].join("\n"),
  );

  const expectedIdentity = options.appleIdentity ?? "-";
  const environment: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: `${fakeBin}:${process.env.PATH ?? ""}`,
    HOME: fixtureRoot,
    TMPDIR: `${auditRoot}/`,
    APPLE_SIGNING_IDENTITY: options.appleIdentity,
    CODEX_PULSE_UPDATER_KEY_PATH: updaterKeyPath,
    FAKE_ARCHIVE_MISMATCH: options.archiveMismatch ? "1" : "0",
    FAKE_CODESIGN_FAIL_CALL: String(
      options.codesignFailureCall ?? 0,
    ),
    FAKE_EMPTY_SIGNATURE: options.emptySignature ? "1" : "0",
    FAKE_EXPECTED_APPLE_IDENTITY: expectedIdentity,
    FAKE_KEY_MODE: options.keyMode ?? "600",
    FAKE_PASSWORD: passwordCanary,
    FAKE_STATE_DIRECTORY: fakeState,
  };
  if (options.appleIdentity === undefined) {
    delete environment.APPLE_SIGNING_IDENTITY;
  }

  return {
    auditRoot,
    keyCanary,
    passwordCanary,
    run: () =>
      runText("/bin/bash", [fixtureScript], {
        cwd: fixtureRoot,
        env: environment,
      }),
  };
}

function expectNoSecret(result: TextResult, harness: Harness) {
  const output = `${result.stdout}\n${result.stderr}`;
  expect(output).not.toContain(harness.passwordCanary);
  expect(output).not.toContain(harness.keyCanary);
}

afterEach(() => {
  for (const fixture of temporaryFixtures.splice(0)) {
    rmSync(fixture, { recursive: true, force: true });
  }
});

describe("local macOS updater signing runbook", () => {
  it("prints help without requiring a toolchain", () => {
    const script = requireSourceScript();
    const result = runText("/bin/bash", [script, "--help"], {
      cwd: repositoryRoot,
      env: { PATH: "" },
    });

    expect(result.status).toBe(0);
    expect(result.stderr).toBe("");
    expect(result.stdout).toContain(
      "Build and verify the local macOS updater bundle",
    );
    expect(result.stdout).toContain("CODEX_PULSE_UPDATER_KEY_PATH");
    expect(result.stdout).toContain("APPLE_SIGNING_IDENTITY");
  });

  it("builds and verifies a bundle with the default identity", () => {
    const script = requireSourceScript();
    expect(statSync(script).mode & constants.S_IXUSR).toBeTruthy();

    const syntax = runText("/bin/bash", ["-n", script], {
      cwd: repositoryRoot,
      env: process.env,
    });
    expect(syntax.status).toBe(0);

    const harness = createHarness();
    const result = harness.run();

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("version=0.3.2");
    expect(result.stdout).toContain("architecture=arm64");
    expect(result.stdout).toContain(
      "bundle_and_archive_executable_sha256=",
    );
    expect(readdirSync(harness.auditRoot)).toEqual([]);
    expectNoSecret(result, harness);
  });

  it("honors an explicitly supplied Apple signing identity", () => {
    const harness = createHarness({
      appleIdentity: "Developer ID Application: Example",
    });
    const result = harness.run();

    expect(result.status).toBe(0);
    expectNoSecret(result, harness);
  });

  it.each([1, 2])(
    "fails when codesign verification call %i fails",
    (codesignFailureCall) => {
      const harness = createHarness({ codesignFailureCall });
      const result = harness.run();

      expect(result.status).not.toBe(0);
      expect(readdirSync(harness.auditRoot)).toEqual([]);
      expectNoSecret(result, harness);
    },
  );

  it("rejects an empty updater signature", () => {
    const harness = createHarness({ emptySignature: true });
    const result = harness.run();

    expect(result.status).not.toBe(0);
    expectNoSecret(result, harness);
  });

  it("rejects source and archive executable divergence", () => {
    const harness = createHarness({ archiveMismatch: true });
    const result = harness.run();

    expect(result.status).not.toBe(0);
    expect(readdirSync(harness.auditRoot)).toEqual([]);
    expectNoSecret(result, harness);
  });

  it("rejects an updater key that is not mode 600", () => {
    const harness = createHarness({ keyMode: "644" });
    const result = harness.run();

    expect(result.status).not.toBe(0);
    expectNoSecret(result, harness);
  });
});
```

The fake commands supply controlled external outcomes. Do not assert their
calls. Each test asserts the real runbook's exit status, output, artifact
parity, or cleanup.

- [ ] **Step 2: Run the focused contract to prove RED**

Run:

```bash
pnpm test -- src/__tests__/localUpdaterBuild.spec.ts
```

Expected: FAIL at `the local updater runbook should exist`. This is the
intended feature-missing failure, not a TypeScript or fixture error.

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
- Produces the source `.app`, updater `.app.tar.gz`, adjacent `.sig`, and
  non-secret version/architecture/hash evidence.

- [ ] **Step 1: Create the complete minimal script required by the RED contract**

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

for required_command in \
  awk \
  codesign \
  dirname \
  file \
  head \
  mktemp \
  node \
  plutil \
  pnpm \
  rm \
  security \
  sed \
  shasum \
  stat \
  tar \
  uname
do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    printf 'Required command is unavailable: %s\n' \
      "$required_command" >&2
    exit 1
  fi
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'This runbook supports macOS only.\n' >&2
  exit 1
fi

script_directory="$(
  cd "$(dirname "${BASH_SOURCE[0]}")"
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
temporary_root="${TMPDIR:-/tmp}"
temporary_root="${temporary_root%/}"
audit_directory=""

cleanup() {
  unset updater_key_password
  if [[ -n "$audit_directory" && -d "$audit_directory" ]]; then
    case "$audit_directory" in
      "$temporary_root"/codex-pulse-updater-audit.*)
        rm -rf -- "$audit_directory"
        ;;
      *)
        printf 'Refusing to remove unexpected audit directory: %s\n' \
          "$audit_directory" >&2
        ;;
    esac
  fi
}
trap cleanup EXIT

test -s "$updater_key_path"
key_mode="$(stat -f '%Lp' "$updater_key_path")"
if [[ "$key_mode" != "600" ]]; then
  printf 'Updater key mode must be 600, observed %s.\n' \
    "$key_mode" >&2
  exit 1
fi

updater_key_password="$(
  security find-generic-password \
    -w \
    -a "$updater_keychain_account" \
    -s "$updater_keychain_service"
)"
test -n "$updater_key_password"

(
  cd "$repository_root"
  APPLE_SIGNING_IDENTITY="$apple_signing_identity" \
    TAURI_SIGNING_PRIVATE_KEY="$updater_key_path" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$updater_key_password" \
    pnpm tauri build --bundles app
)
unset updater_key_password

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

codesign --verify --deep --strict --verbose=4 "$source_app"

bundle_identifier="$(
  plutil \
    -extract CFBundleIdentifier \
    raw \
    -o - \
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
  sed -n \
    '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' \
    "$repository_root/src-tauri/Cargo.toml" |
    head -n 1
)"
bundle_version="$(
  plutil \
    -extract CFBundleShortVersionString \
    raw \
    -o - \
    "$source_app/Contents/Info.plist"
)"
test "$package_version" = "$tauri_version"
test "$package_version" = "$cargo_version"
test "$package_version" = "$bundle_version"

file_description="$(file "$source_executable")"
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
  mktemp \
    -d \
    "$temporary_root/codex-pulse-updater-audit.XXXXXX"
)"
tar -xzf "$archive_file" -C "$audit_directory"
archive_app="$audit_directory/Codex Pulse.app"
archive_executable="$archive_app/Contents/MacOS/CodexPulse"

test -d "$archive_app"
test -x "$archive_executable"
codesign --verify --deep --strict --verbose=4 "$archive_app"

source_hash="$(
  shasum -a 256 "$source_executable" |
    awk '{print $1}'
)"
archive_hash="$(
  shasum -a 256 "$archive_executable" |
    awk '{print $1}'
)"
test "$source_hash" = "$archive_hash"

printf 'bundle=%s\n' "$source_app"
printf 'version=%s\n' "$bundle_version"
printf 'architecture=%s\n' "$expected_architecture"
printf 'archive=%s size=%s\n' \
  "$archive_file" \
  "$(stat -f '%z' "$archive_file")"
printf 'signature=%s size=%s\n' \
  "$signature_file" \
  "$(stat -f '%z' "$signature_file")"
printf 'bundle_and_archive_executable_sha256=%s\n' \
  "$source_hash"
```

Do not add `codesign --force` or a post-archive repair path. The identity must
be present when Tauri packages the updater archive.

- [ ] **Step 2: Make the script executable and prove GREEN**

Run:

```bash
chmod 755 scripts/build-local-updater-macos.sh
pnpm test -- src/__tests__/localUpdaterBuild.spec.ts
```

Expected: all eight behavior cases pass. The success cases prove the default
and explicit identities, correct secret injection without disclosure, and
temporary cleanup. The failure cases prove both codesign boundaries, non-empty
signature, executable parity, and key mode fail closed.

### Task 3: Point maintainer documentation at the canonical runbook

**Files:**
- Modify: `CONTRIBUTING.md`
- Modify: `docs/superpowers/plans/2026-07-28-automatic-updates.md`
- Verify: human review plus `src/__tests__/localUpdaterBuild.spec.ts`

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

- [ ] **Step 3: Review documentation scope and rerun the behavior contract**

Confirm through `git diff` that:

- neither public README changed;
- the historical acceptance report did not change;
- `CONTRIBUTING.md` contains only the maintainer entry point and evidence
  boundary; and
- the old inline secret/build sequence is absent from the automatic-update
  plan.

Run:

```bash
pnpm test -- src/__tests__/localUpdaterBuild.spec.ts
```

Expected: all eight behavior cases remain green.

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

- [ ] **Step 1: Confirm the installed app remains the only running instance**

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
- the implementation commit is ahead of the design and plan commits;
- no branch, push, tag, Draft, publication, or installation occurred; and
- the implementation commit contains exactly the script, focused behavior
  contract, `CONTRIBUTING.md`, and the automatic-update plan.
