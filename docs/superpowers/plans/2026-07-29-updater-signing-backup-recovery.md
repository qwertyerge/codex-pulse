# Updater Signing Backup Recovery Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add and execute a repeatable local recovery drill that proves the independently restored encrypted updater key and separately recovered passphrase can sign a benign fixture accepted by the public key committed for Codex Pulse `0.4.0`.

**Architecture:** A macOS Bash entry point owns TTY-only attestations, hidden recovery input, a permission-restricted temporary key copy, one Tauri signing call, fail-closed cleanup, and sanitized evidence promotion. A Rust example mirrors `tauri-plugin-updater 2.10.1` signature decoding and verification with the locked `minisign-verify 0.2.5`; public fixture/signature evidence remains replayable without either secret.

**Tech Stack:** Bash 3.2, Tauri CLI 2.11.4, Rust 1.82+, `base64 0.22.1`, `minisign-verify 0.2.5`, Cargo, pnpm, Node.js, Vitest 4, GitHub CLI

## Global Constraints

- Work only in `/Users/loki/.codex/worktrees/007c/codex-pulse` on the existing `codex/release-0.4.0` branch and existing pull request #19.
- Do not create another branch or pull request.
- Do not merge pull request #19.
- Do not create a `0.4.0` tag, Draft Release, published Release, updater manifest, or installation.
- Do not read or mutate GitHub Secret values, the original developer-machine updater key, or the existing updater Keychain item.
- Do not run a complete application or updater bundle build for this recovery gate.
- Never send or record the private key, passphrase, restored-key path, private-key hash, username, hostname, or backup medium identifier.
- Require interactive `/dev/tty` input for the real recovery drill; do not add a production non-interactive secret-input override.
- Never pass the restored-key path or passphrase in process argv. Use `TAURI_SIGNING_PRIVATE_KEY_PATH` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` only in the one signing subprocess environment.
- Disable xtrace before secrets are read; do not provide a real-secret debug mode.
- Use exact development dependencies `base64 = "=0.22.1"` and `minisign-verify = "=0.2.5"`.
- Keep the Rust verifier outside the application command surface as `src-tauri/examples/verify_updater_signature.rs`.
- Promote exactly `fixture.txt`, `fixture.txt.sig`, and `verification.json` only after positive verification and tamper rejection both succeed.
- Decode `.sig` before promotion and require the exact path-free Tauri comment schema plus two base64 signature-body lines.
- Never overwrite existing public recovery evidence.
- Keep raw signer output inside the private temporary directory and delete it on every exit.
- Run frontend and Rust validation serially; do not recreate the previously observed parallel test contention.
- Treat local proof, exact-head CI, CodeRabbit review, PR merge, tag, Draft, installation, publication, and old-to-new updater acceptance as separate gates.

---

## File Map

- Modify `src-tauri/Cargo.toml`: add exact verifier-only development dependencies.
- Modify `src-tauri/Cargo.lock`: record the two direct development dependencies on the local package without changing their already locked versions.
- Create `src-tauri/examples/verify_updater_signature.rs`: stable three-path verifier with safe exit statuses and unit vectors.
- Create `src/__tests__/updaterBackupRecovery.spec.ts`: pseudo-terminal behavior contract for the real Bash entry point with fake external boundaries.
- Create `scripts/verify-updater-signing-backup.sh`: TTY-only recovery drill and evidence producer.
- Create at runtime `docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt`: public randomized challenge.
- Create at runtime `docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt.sig`: public Tauri updater signature.
- Create at runtime `docs/superpowers/reports/0.4.0-updater-backup-recovery/verification.json`: sanitized machine-readable result.
- Create `src/__tests__/updaterBackupRecoveryEvidence.spec.ts`: durable structural and hash validation of public evidence.
- Modify `docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md`: close only the independent offline backup gate.

### Task 1: Add the runtime-equivalent Rust signature verifier

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Create: `src-tauri/examples/verify_updater_signature.rs`
- Test: `src-tauri/examples/verify_updater_signature.rs`

**Interfaces:**
- Consumes: three positional path arguments in this order: Tauri config,
  fixture, signature.
- Produces: exit `0` and `signature_verified=true` for a valid signature; exit `2` for usage, `3` for malformed or missing inputs, and `4` for cryptographic rejection.
- Produces for Task 2: `cargo run --locked --offline --quiet --manifest-path src-tauri/Cargo.toml --example verify_updater_signature -- "$config" "$fixture" "$signature"`.

- [ ] **Step 1: Add exact development dependencies and the failing verifier tests**

Append to `[dev-dependencies]` in `src-tauri/Cargo.toml`:

```toml
base64 = "=0.22.1"
minisign-verify = "=0.2.5"
```

Create `src-tauri/examples/verify_updater_signature.rs` with imports, the public
test vectors, an empty `main`, and tests that reference the not-yet-created
interfaces:

```rust
use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use std::{
    env, fs,
    path::Path,
    process::{ExitCode, Termination},
};

#[derive(Debug, PartialEq, Eq)]
enum VerificationError {
    Input,
    Rejected,
}

#[cfg(test)]
const TEST_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXkgRTc2MjBGMTg0MkI0RTgxRgpSV1FmNkxSQ0dBOWk1M21sWWVjTzRJelQ1MVRHUHB2V3VjTlNDaDFDQk0wUVRhTG43M1k3R0ZPMwo=";
#[cfg(test)]
const TEST_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIG1pbmlzaWduIHNlY3JldCBrZXkKUldRZjZMUkNHQTlpNTlTTE9GeHo2Tnh2QVNYREplUnR1Wnlrd1FlcGJERUd0ODdpZzFCTnBXYVZXdU5ybTczWWlJaUpicTcxV2krZFA5ZUtMOE9DMzUxdndJYXNTU2JYeHdBPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNTU1Nzc5OTY2CWZpbGU6dGVzdApRdEtNWFd5WWN3ZHBaQWxQRjd0RTJFTkprUmQxdWp2S2psajFtOVJ0SFRCblpQYTVXS1U1dVdSczVHb1A1TS9WcUU4MVFGdU1LSTVrL1NmTlFVYU9BQT09Cg==";

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn outer_encode(document: &str) -> String {
        STANDARD.encode(document.as_bytes())
    }

    #[test]
    fn accepts_a_valid_tauri_encoded_signature() {
        assert_eq!(
            verify_encoded_signature(TEST_PUBLIC_KEY, TEST_SIGNATURE, b"test"),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_modified_fixture() {
        assert_eq!(
            verify_encoded_signature(TEST_PUBLIC_KEY, TEST_SIGNATURE, b"Test"),
            Err(VerificationError::Rejected)
        );
    }

    #[test]
    fn rejects_a_modified_public_key() {
        let public_document = decode_document(TEST_PUBLIC_KEY)
            .expect("public test vector should decode")
            .replace("GFO3", "GFO2");

        assert_eq!(
            verify_encoded_signature(
                &outer_encode(&public_document),
                TEST_SIGNATURE,
                b"test"
            ),
            Err(VerificationError::Rejected)
        );
    }

    #[test]
    fn rejects_a_modified_signature() {
        let signature_document = decode_document(TEST_SIGNATURE)
            .expect("signature test vector should decode")
            .replace("RWQf6LRCGA9i59S", "RWQf6LRCGA9i58S");

        assert_eq!(
            verify_encoded_signature(
                TEST_PUBLIC_KEY,
                &outer_encode(&signature_document),
                b"test"
            ),
            Err(VerificationError::Rejected)
        );
    }

    #[test]
    fn rejects_malformed_outer_base64() {
        assert_eq!(
            verify_encoded_signature("not-base64", TEST_SIGNATURE, b"test"),
            Err(VerificationError::Input)
        );
        assert_eq!(
            verify_encoded_signature(TEST_PUBLIC_KEY, "not-base64", b"test"),
            Err(VerificationError::Input)
        );
    }

    #[test]
    fn rejects_a_config_without_an_updater_public_key() {
        let temporary = tempdir().expect("temporary directory should exist");
        let config = temporary.path().join("tauri.conf.json");
        let fixture = temporary.path().join("fixture.txt");
        let signature = temporary.path().join("fixture.txt.sig");

        fs::write(&config, r#"{"plugins":{"updater":{}}}"#)
            .expect("config should be written");
        fs::write(&fixture, b"test").expect("fixture should be written");
        fs::write(&signature, TEST_SIGNATURE)
            .expect("signature should be written");

        assert_eq!(
            verify_files(&config, &fixture, &signature),
            Err(VerificationError::Input)
        );
    }
}
```

- [ ] **Step 2: Run the example test target and confirm the RED state**

Run:

```bash
cargo test --offline --manifest-path src-tauri/Cargo.toml --example verify_updater_signature
```

Expected: compilation fails with unresolved `decode_document`,
`verify_encoded_signature`, and `verify_files` references. Cargo may update only
the local `codex-pulse` dependency list in `Cargo.lock`; the locked
`base64 0.22.1` and `minisign-verify 0.2.5` package entries must not change.

- [ ] **Step 3: Implement the complete verifier**

Replace the empty `main` and add these functions above the test module:

```rust
fn decode_document(encoded: &str) -> Result<String, VerificationError> {
    let bytes = STANDARD
        .decode(encoded.trim())
        .map_err(|_| VerificationError::Input)?;
    String::from_utf8(bytes).map_err(|_| VerificationError::Input)
}

fn verify_encoded_signature(
    encoded_public_key: &str,
    encoded_signature: &str,
    fixture: &[u8],
) -> Result<(), VerificationError> {
    let public_key_document = decode_document(encoded_public_key)?;
    let signature_document = decode_document(encoded_signature)?;
    let public_key =
        PublicKey::decode(&public_key_document).map_err(|_| VerificationError::Input)?;
    let signature =
        Signature::decode(&signature_document).map_err(|_| VerificationError::Input)?;

    public_key
        .verify(fixture, &signature, true)
        .map_err(|_| VerificationError::Rejected)
}

fn verify_files(
    config_path: &Path,
    fixture_path: &Path,
    signature_path: &Path,
) -> Result<(), VerificationError> {
    let config_bytes = fs::read(config_path).map_err(|_| VerificationError::Input)?;
    let config: Value =
        serde_json::from_slice(&config_bytes).map_err(|_| VerificationError::Input)?;
    let encoded_public_key = config
        .pointer("/plugins/updater/pubkey")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(VerificationError::Input)?;
    let fixture = fs::read(fixture_path).map_err(|_| VerificationError::Input)?;
    let encoded_signature =
        fs::read_to_string(signature_path).map_err(|_| VerificationError::Input)?;

    verify_encoded_signature(encoded_public_key, &encoded_signature, &fixture)
}

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 {
        eprintln!("verification_status=usage_error");
        return ExitCode::from(2);
    }

    match verify_files(
        Path::new(&arguments[0]),
        Path::new(&arguments[1]),
        Path::new(&arguments[2]),
    ) {
        Ok(()) => {
            println!("signature_verified=true");
            ExitCode::SUCCESS
        }
        Err(VerificationError::Input) => {
            eprintln!("verification_status=input_error");
            ExitCode::from(3)
        }
        Err(VerificationError::Rejected) => {
            eprintln!("verification_status=rejected");
            ExitCode::from(4)
        }
    }
}
```

Remove the unused `Termination` import. Keep stderr stable and path-free; do
not print the underlying I/O, decoding, key, or signature error.

- [ ] **Step 4: Run the focused Rust tests and CLI exit-contract checks**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --example verify_updater_signature

set +e
cargo run --locked --offline --quiet --manifest-path src-tauri/Cargo.toml --example verify_updater_signature
usage_status=$?
set -e
test "$usage_status" -eq 2
```

Expected: six example unit tests pass and the missing-argument invocation exits
exactly `2`.

- [ ] **Step 5: Verify the lockfile boundary and commit**

Run:

```bash
git diff -- src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/examples/verify_updater_signature.rs
git diff --check
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/examples/verify_updater_signature.rs
git diff --cached --check
git commit -m "feat: add updater signature evidence verifier"
```

Expected: the root `codex-pulse` lock entry gains direct `base64` and
`minisign-verify` dependencies, their existing locked package versions remain
unchanged, and the commit contains only this verifier slice.

---

### Task 2: Add the TTY-only recovery drill with an isolated behavior contract

**Files:**
- Create: `src/__tests__/updaterBackupRecovery.spec.ts`
- Create: `scripts/verify-updater-signing-backup.sh`
- Test: `src/__tests__/updaterBackupRecovery.spec.ts`

**Interfaces:**
- Consumes: no argv for a real run; two visible `yes` attestations followed by hidden restored-key path and hidden passphrase through `/dev/tty`.
- Consumes: Task 1 verifier exit statuses `0`, `3`, and `4`.
- Produces: exactly three public evidence files on success and no evidence directory on failure.
- Preserves: no production non-interactive secret-input override.

- [ ] **Step 1: Create the failing pseudo-terminal behavior contract**

Create `src/__tests__/updaterBackupRecovery.spec.ts`. Reuse the repository's
existing fake-command style, but drive the real script through a platform
appropriate `script(1)` pseudo-terminal:

```ts
import {
  chmodSync,
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { afterEach, describe, expect, it } from "vitest";

interface TextResult {
  status: number | null;
  stdout: string;
  stderr: string;
}

interface HarnessOptions {
  existingEvidence?: boolean;
  signatureMetadataInvalid?: boolean;
  signerFails?: boolean;
  verifierFails?: boolean;
  tamperedAccepted?: boolean;
}

interface Harness {
  auditDirectory: string;
  evidenceDirectory: string;
  keyCanary: string;
  passwordCanary: string;
  privateTempDirectory: string;
  restoredKeyPath: string;
  run: (responses?: string[]) => Promise<TextResult>;
}

const repositoryRoot = process.cwd();
const sourceScript = resolve(
  repositoryRoot,
  "scripts/verify-updater-signing-backup.sh",
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

function ptyArguments(scriptPath: string) {
  if (process.platform === "darwin") {
    return ["-q", "/dev/null", "/bin/bash", scriptPath];
  }
  return ["-q", "-e", "-c", `/bin/bash ${scriptPath}`, "/dev/null"];
}

function runInPty(
  scriptPath: string,
  cwd: string,
  env: NodeJS.ProcessEnv,
  responses: string[],
): Promise<TextResult> {
  const prompts = [
    "Type yes to attest independent key recovery:",
    "Type yes to attest separate passphrase recovery:",
    "Restored encrypted key path (hidden):",
    "Restored key passphrase (hidden):",
  ];

  return new Promise((resolveResult, reject) => {
    const child = spawn("script", ptyArguments(scriptPath), {
      cwd,
      env,
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    let responseIndex = 0;
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("recovery script pseudo-terminal timed out"));
    }, 15_000);

    const answerPrompts = () => {
      const combined = `${stdout}\n${stderr}`;
      while (
        responseIndex < responses.length &&
        combined.includes(prompts[responseIndex])
      ) {
        child.stdin.write(`${responses[responseIndex]}\n`);
        responseIndex += 1;
      }
    };

    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString("utf8");
      answerPrompts();
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString("utf8");
      answerPrompts();
    });
    child.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.on("close", (status) => {
      clearTimeout(timer);
      resolveResult({ status, stdout, stderr });
    });
  });
}

function createHarness(options: HarnessOptions = {}): Harness {
  const fixtureRoot = mkdtempSync(
    join(tmpdir(), "codex-pulse-backup-recovery-"),
  );
  temporaryFixtures.push(fixtureRoot);

  const fixtureScript = join(
    fixtureRoot,
    "scripts/verify-updater-signing-backup.sh",
  );
  mkdirSync(dirname(fixtureScript), { recursive: true });
  copyFileSync(sourceScript, fixtureScript);
  chmodSync(fixtureScript, 0o755);

  mkdirSync(join(fixtureRoot, "src-tauri"), { recursive: true });
  mkdirSync(join(fixtureRoot, "docs/superpowers/reports"), {
    recursive: true,
  });
  writeFileSync(
    join(fixtureRoot, "src-tauri/tauri.conf.json"),
    JSON.stringify({
      version: "0.4.0",
      plugins: { updater: { pubkey: "cHVibGljLWtleQ==" } },
    }),
  );
  writeFileSync(
    join(fixtureRoot, "src-tauri/Cargo.lock"),
    [
      "[[package]]",
      'name = "tauri-plugin-updater"',
      'version = "2.10.1"',
      "",
      "[[package]]",
      'name = "minisign-verify"',
      'version = "0.2.5"',
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(fixtureRoot, "src-tauri/Cargo.toml"),
    '[package]\nname = "fixture"\nversion = "0.4.0"\n',
  );

  const keyCanary = "private-key-canary";
  const passwordCanary = "passphrase-canary";
  const fakePublicSignature = Buffer.from(
    [
      "untrusted comment: signature from tauri secret key",
      "ZmFrZS1zaWduYXR1cmU=",
      "trusted comment: timestamp:1785332262\tfile:fixture.txt",
      "ZmFrZS1nbG9iYWwtc2lnbmF0dXJl",
      "",
    ].join("\n"),
  ).toString("base64");
  const invalidPublicSignature = Buffer.from(
    [
      "untrusted comment: signature from tauri secret key",
      "ZmFrZS1zaWduYXR1cmU=",
      "trusted comment: timestamp:1785332262\tfile:/private/path/fixture.txt",
      "ZmFrZS1nbG9iYWwtc2lnbmF0dXJl",
      "",
    ].join("\n"),
  ).toString("base64");
  const restoredKeyPath = join(fixtureRoot, "restored.key");
  writeFileSync(
    restoredKeyPath,
    Buffer.from(
      `untrusted comment: rsign encrypted secret key\n${keyCanary}\n`,
    ).toString("base64"),
  );

  const fakeBin = join(fixtureRoot, "fake-bin");
  const auditDirectory = join(fixtureRoot, "audit");
  const privateTempDirectory = join(fixtureRoot, "private-tmp");
  mkdirSync(fakeBin);
  mkdirSync(auditDirectory);
  mkdirSync(privateTempDirectory);

  writeStub(fakeBin, "uname", 'printf "Darwin\\n"');
  writeStub(fakeBin, "uuidgen", 'printf "11111111-2222-4333-8444-555555555555\\n"');
  writeStub(
    fakeBin,
    "cp",
    [
      'test "${1:-}" != "$FAKE_RESTORED_KEY_PATH"',
      '/bin/cp "$@"',
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "git",
    [
      'case "$1" in',
      '  status) exit 0 ;;',
      '  rev-parse) printf "0123456789abcdef0123456789abcdef01234567\\n" ;;',
      "  *) exit 41 ;;",
      "esac",
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "pnpm",
    [
      'if [[ "$*" = "tauri --version" ]]; then',
      '  test -z "${TAURI_SIGNING_PRIVATE_KEY:-}"',
      '  test -z "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}"',
      '  test -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"',
      '  test -z "${TAURI_PRIVATE_KEY:-}"',
      '  test -z "${TAURI_PRIVATE_KEY_PATH:-}"',
      '  test -z "${TAURI_PRIVATE_KEY_PASSWORD:-}"',
      '  test -z "${TAURI_KEY_PASSWORD:-}"',
      '  printf "tauri-cli 2.11.4\\n"',
      "  exit 0",
      "fi",
      'test "$1" = "tauri"',
      'test "$2" = "signer"',
      'test "$3" = "sign"',
      'test "$#" -eq 4',
      'test -z "${TAURI_SIGNING_PRIVATE_KEY:-}"',
      'test -z "${TAURI_PRIVATE_KEY:-}"',
      'test -z "${TAURI_PRIVATE_KEY_PATH:-}"',
      'test -z "${TAURI_PRIVATE_KEY_PASSWORD:-}"',
      'test -z "${TAURI_KEY_PASSWORD:-}"',
      'test "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" = "$FAKE_PASSWORD"',
      'test "$TAURI_SIGNING_PRIVATE_KEY_PATH" != "$FAKE_RESTORED_KEY_PATH"',
      'node -e \'const fs=require("fs"); const mode=fs.statSync(process.argv[1]).mode & 0o777; if (mode !== 0o600) process.exit(42)\' "$TAURI_SIGNING_PRIVATE_KEY_PATH"',
      'printf "%s\\n" "$*" > "$FAKE_AUDIT_DIRECTORY/signer-argv"',
      'printf "raw-path=%s raw-password=%s\\n" "$TAURI_SIGNING_PRIVATE_KEY_PATH" "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"',
      'if [[ "${FAKE_SIGNER_FAILS:-0}" = "1" ]]; then exit 43; fi',
      'if [[ "${FAKE_SIGNATURE_METADATA_INVALID:-0}" = "1" ]]; then',
      '  printf "%s" "$FAKE_INVALID_PUBLIC_SIGNATURE" > "$4.sig"',
      "else",
      '  printf "%s" "$FAKE_PUBLIC_SIGNATURE" > "$4.sig"',
      "fi",
    ].join("\n"),
  );
  writeStub(
    fakeBin,
    "cargo",
    [
      'fixture="${11}"',
      'if grep -q "tampered=true" "$fixture"; then',
      '  if [[ "${FAKE_TAMPERED_ACCEPTED:-0}" = "1" ]]; then',
      '    printf "signature_verified=true\\n"',
      "    exit 0",
      "  fi",
      "  exit 4",
      "fi",
      'if [[ "${FAKE_VERIFIER_FAILS:-0}" = "1" ]]; then exit 3; fi',
      'printf "signature_verified=true\\n"',
    ].join("\n"),
  );

  const evidenceDirectory = join(
    fixtureRoot,
    "docs/superpowers/reports/0.4.0-updater-backup-recovery",
  );
  if (options.existingEvidence) {
    mkdirSync(evidenceDirectory);
    writeFileSync(join(evidenceDirectory, "sentinel"), "preserve\n");
  }

  const environment: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: `${fakeBin}:${process.env.PATH ?? ""}`,
    TMPDIR: `${privateTempDirectory}/`,
    FAKE_AUDIT_DIRECTORY: auditDirectory,
    FAKE_INVALID_PUBLIC_SIGNATURE: invalidPublicSignature,
    FAKE_PASSWORD: passwordCanary,
    FAKE_PUBLIC_SIGNATURE: fakePublicSignature,
    FAKE_SIGNATURE_METADATA_INVALID:
      options.signatureMetadataInvalid ? "1" : "0",
    FAKE_RESTORED_KEY_PATH: restoredKeyPath,
    FAKE_SIGNER_FAILS: options.signerFails ? "1" : "0",
    FAKE_TAMPERED_ACCEPTED: options.tamperedAccepted ? "1" : "0",
    FAKE_VERIFIER_FAILS: options.verifierFails ? "1" : "0",
    TAURI_SIGNING_PRIVATE_KEY: "inherited-key-must-be-removed",
    TAURI_SIGNING_PRIVATE_KEY_PATH: "inherited-path-must-be-replaced",
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD:
      "inherited-password-must-be-replaced",
    TAURI_PRIVATE_KEY: "legacy-key-must-be-removed",
    TAURI_PRIVATE_KEY_PATH: "legacy-path-must-be-removed",
    TAURI_PRIVATE_KEY_PASSWORD:
      "legacy-private-password-must-be-removed",
    TAURI_KEY_PASSWORD: "legacy-password-must-be-removed",
  };
  return {
    auditDirectory,
    evidenceDirectory,
    keyCanary,
    passwordCanary,
    privateTempDirectory,
    restoredKeyPath,
    run: (responses = ["yes", "yes", restoredKeyPath, passwordCanary]) =>
      runInPty(fixtureScript, fixtureRoot, environment, responses),
  };
}

function allEvidenceText(directory: string) {
  if (!existsSync(directory)) return "";
  return readdirSync(directory)
    .map((name) => readFileSync(join(directory, name), "utf8"))
    .join("\n");
}

function expectNoSecrets(result: TextResult, harness: Harness) {
  const observable = [
    result.stdout,
    result.stderr,
    allEvidenceText(harness.evidenceDirectory),
  ].join("\n");
  expect(observable).not.toContain(harness.keyCanary);
  expect(observable).not.toContain(harness.passwordCanary);
  expect(observable).not.toContain(harness.restoredKeyPath);
  expect(observable).not.toContain(
    readFileSync(harness.restoredKeyPath, "utf8").trim(),
  );
}

afterEach(() => {
  for (const fixture of temporaryFixtures.splice(0)) {
    rmSync(fixture, { recursive: true, force: true });
  }
});

const describeOnPosix =
  process.platform === "win32" ? describe.skip : describe;

describeOnPosix("updater signing backup recovery drill", () => {
  it("prints help without requesting a secret", () => {
    expect(existsSync(sourceScript)).toBe(true);
    expect(statSync(sourceScript).mode & constants.S_IXUSR).toBeTruthy();
    const result = spawnSync("/bin/bash", [sourceScript, "--help"], {
      cwd: repositoryRoot,
      env: { PATH: "" },
      encoding: "utf8",
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Verify a restored updater signing backup");
  });

  it("rejects a non-interactive real run", () => {
    const result = spawnSync("/bin/bash", [sourceScript], {
      cwd: repositoryRoot,
      encoding: "utf8",
    });
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("recovery_verification_failed=tty");
  });

  it("promotes only sanitized public evidence after positive and negative verification", async () => {
    const harness = createHarness();
    const result = await harness.run();

    expect(result.status).toBe(0);
    expect(readdirSync(harness.evidenceDirectory).sort()).toEqual([
      "fixture.txt",
      "fixture.txt.sig",
      "verification.json",
    ]);
    expect(JSON.parse(
      readFileSync(
        join(harness.evidenceDirectory, "verification.json"),
        "utf8",
      ),
    )).toMatchObject({
      schema_version: 1,
      source_commit: "0123456789abcdef0123456789abcdef01234567",
      encrypted_key_format_valid: true,
      independent_key_source_attested: true,
      separate_passphrase_source_attested: true,
      signature_verified: true,
      tampered_fixture_rejected: true,
    });
    const signerArgv = readFileSync(
      join(harness.auditDirectory, "signer-argv"),
      "utf8",
    );
    expect(signerArgv).not.toContain(harness.restoredKeyPath);
    expect(signerArgv).not.toContain(harness.passwordCanary);
    expect(signerArgv).not.toContain(
      readFileSync(harness.restoredKeyPath, "utf8").trim(),
    );
    expect(readdirSync(harness.privateTempDirectory)).toEqual([]);
    expectNoSecrets(result, harness);
  });

  it.each<[string, string[]]>([
    ["key-source attestation", ["no"]],
    ["passphrase-source attestation", ["yes", "no"]],
  ])("requires %s", async (_name, responses) => {
    const harness = createHarness();
    const result = await harness.run(responses);
    expect(result.status).not.toBe(0);
    expect(existsSync(harness.evidenceDirectory)).toBe(false);
    expect(readdirSync(harness.privateTempDirectory)).toEqual([]);
    expectNoSecrets(result, harness);
  });

  it.each<[string, HarnessOptions]>([
    ["signing failure", { signerFails: true }],
    ["signature metadata failure", { signatureMetadataInvalid: true }],
    ["verification failure", { verifierFails: true }],
    ["unexpected tamper acceptance", { tamperedAccepted: true }],
  ])("fails closed on %s", async (_name, options) => {
    const harness = createHarness(options);
    const result = await harness.run();
    expect(result.status).not.toBe(0);
    expect(existsSync(harness.evidenceDirectory)).toBe(false);
    expect(readdirSync(harness.privateTempDirectory)).toEqual([]);
    expectNoSecrets(result, harness);
  });

  it("never overwrites existing evidence", async () => {
    const harness = createHarness({ existingEvidence: true });
    const result = await harness.run([]);
    expect(result.status).not.toBe(0);
    expect(
      readFileSync(join(harness.evidenceDirectory, "sentinel"), "utf8"),
    ).toBe("preserve\n");
    expect(readdirSync(harness.privateTempDirectory)).toEqual([]);
  });
});
```

If TypeScript reports a nullable `stdout` type from `spawnSync`, normalize it
with `result.stdout ?? ""` at the assertion. Do not weaken any secret-canary
assertion.

- [ ] **Step 2: Run the shell contract and confirm the RED state**

Run:

```bash
pnpm exec vitest run src/__tests__/updaterBackupRecovery.spec.ts
```

Expected: failure at the source-script existence/executable assertion. No
production file exists yet.

- [ ] **Step 3: Implement the complete recovery script**

Create `scripts/verify-updater-signing-backup.sh`:

```bash
#!/usr/bin/env bash

set +x
set -euo pipefail

usage() {
  printf '%s\n' \
    "Verify a restored updater signing backup." \
    "" \
    "Run interactively from a clean Codex Pulse repository." \
    "The restored key path and passphrase are read silently from /dev/tty." \
    "No secret, path, private-key hash, or storage identifier is recorded."
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi
if [[ "$#" -ne 0 ]]; then
  usage >&2
  exit 2
fi

unset TAURI_SIGNING_PRIVATE_KEY
unset TAURI_SIGNING_PRIVATE_KEY_PATH
unset TAURI_SIGNING_PRIVATE_KEY_PASSWORD
unset TAURI_PRIVATE_KEY
unset TAURI_PRIVATE_KEY_PATH
unset TAURI_PRIVATE_KEY_PASSWORD
unset TAURI_KEY_PASSWORD

fail() {
  printf 'recovery_verification_failed=%s\n' "$1" >&2
  exit 1
}

for required_command in \
  awk \
  cargo \
  chmod \
  cp \
  date \
  dirname \
  git \
  grep \
  mktemp \
  mv \
  node \
  pnpm \
  rm \
  shasum \
  tr \
  uname \
  uuidgen
do
  command -v "$required_command" >/dev/null 2>&1 ||
    fail "missing-command"
done

[[ "$(uname -s)" == "Darwin" ]] || fail "platform"
[[ -t 0 && -t 1 && -r /dev/tty && -w /dev/tty ]] || fail "tty"

script_directory="$(
  cd "$(dirname "${BASH_SOURCE[0]}")"
  pwd -P
)"
repository_root="$(
  cd "$script_directory/.."
  pwd -P
)"
cd "$repository_root"

evidence_relative="docs/superpowers/reports/0.4.0-updater-backup-recovery"
evidence_directory="$repository_root/$evidence_relative"
[[ ! -e "$evidence_directory" ]] || fail "evidence-exists"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
  fail "dirty-worktree"

node -e '
  const config = require(process.argv[1]);
  if (config.version !== "0.4.0") process.exit(1);
  const key = config.plugins?.updater?.pubkey;
  if (typeof key !== "string" || key.length === 0) process.exit(1);
' "$repository_root/src-tauri/tauri.conf.json" ||
  fail "configuration"

prompt_attestation() {
  local prompt="$1"
  local answer=""
  printf '%s ' "$prompt" > /dev/tty
  IFS= read -r answer < /dev/tty
  [[ "$answer" == "yes" ]]
}

prompt_attestation \
  "Type yes to attest independent key recovery:" ||
  fail "key-source-attestation"
prompt_attestation \
  "Type yes to attest separate passphrase recovery:" ||
  fail "passphrase-source-attestation"

restored_key_path=""
restored_key_password=""
restored_key_encoded=""
extra_key_line=""
private_directory=""
public_staging=""
temporary_root="${TMPDIR:-/tmp}"
temporary_root="${temporary_root%/}"

cleanup() {
  unset restored_key_path
  unset restored_key_password
  unset restored_key_encoded
  unset extra_key_line
  unset TAURI_SIGNING_PRIVATE_KEY
  unset TAURI_SIGNING_PRIVATE_KEY_PATH
  unset TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  unset TAURI_PRIVATE_KEY
  unset TAURI_PRIVATE_KEY_PATH
  unset TAURI_PRIVATE_KEY_PASSWORD
  unset TAURI_KEY_PASSWORD

  if [[ -n "$private_directory" && -d "$private_directory" ]]; then
    case "$private_directory" in
      "$temporary_root"/codex-pulse-backup-recovery.*)
        rm -rf -- "$private_directory"
        ;;
      *)
        printf 'recovery_cleanup_refused=private-directory\n' >&2
        ;;
    esac
  fi

  if [[ -n "$public_staging" && -d "$public_staging" ]]; then
    case "$public_staging" in
      "$repository_root"/docs/superpowers/reports/.0.4.0-updater-backup-recovery.*)
        rm -rf -- "$public_staging"
        ;;
      *)
        printf 'recovery_cleanup_refused=public-staging\n' >&2
        ;;
    esac
  fi
}
trap cleanup EXIT

read_hidden() {
  local prompt="$1"
  local destination="$2"
  printf '%s ' "$prompt" > /dev/tty
  IFS= read -r -s "$destination" < /dev/tty
  printf '\n' > /dev/tty
}

read_hidden "Restored encrypted key path (hidden):" restored_key_path
case "$restored_key_path" in
  /*) ;;
  *) fail "key-path" ;;
esac
[[ -f "$restored_key_path" && ! -L "$restored_key_path" ]] ||
  fail "key-file"
[[ -r "$restored_key_path" ]] || fail "key-file"

read_hidden "Restored key passphrase (hidden):" restored_key_password
[[ -n "$restored_key_password" ]] || fail "passphrase"

private_directory="$(
  mktemp -d "$temporary_root/codex-pulse-backup-recovery.XXXXXX"
)"
chmod 700 "$private_directory"
private_key_copy="$private_directory/restored-updater.key"
if ! { exec 3<"$restored_key_path"; } 2>/dev/null; then
  fail "key-file"
fi
IFS= read -r restored_key_encoded <&3 ||
  [[ -n "$restored_key_encoded" ]] ||
  fail "key-file"
if IFS= read -r extra_key_line <&3 || [[ -n "$extra_key_line" ]]; then
  fail "encrypted-key-format"
fi
exec 3<&-
printf '%s' "$restored_key_encoded" > "$private_key_copy"
chmod 600 "$private_key_copy"
unset restored_key_path
unset restored_key_encoded
unset extra_key_line

key_header="$(
  node -e '
    const fs = require("fs");
    const encoded = fs.readFileSync(process.argv[1], "utf8").trim();
    const decoded = Buffer.from(encoded, "base64").toString("utf8");
    process.stdout.write(decoded.split(/\r?\n/, 1)[0] ?? "");
  ' "$private_key_copy" 2>/dev/null
)" || fail "encrypted-key-format"
[[ "$key_header" == "untrusted comment: rsign encrypted secret key" ]] ||
  fail "encrypted-key-format"
unset key_header

source_commit="$(git rev-parse HEAD)"
challenge_time="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
challenge_nonce="$(
  uuidgen |
    tr '[:upper:]' '[:lower:]'
)"
fixture="$private_directory/fixture.txt"
printf '%s\n' \
  "schema=codex-pulse-updater-backup-recovery-v1" \
  "repository=qwertyerge/codex-pulse" \
  "release=0.4.0" \
  "source_commit=$source_commit" \
  "verified_at_utc=$challenge_time" \
  "nonce=$challenge_nonce" > "$fixture"

signer_log="$private_directory/signer.log"
sign_succeeded="false"
if (
  unset TAURI_SIGNING_PRIVATE_KEY
  unset TAURI_PRIVATE_KEY
  unset TAURI_PRIVATE_KEY_PATH
  unset TAURI_PRIVATE_KEY_PASSWORD
  unset TAURI_KEY_PASSWORD
  export TAURI_SIGNING_PRIVATE_KEY_PATH="$private_key_copy"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$restored_key_password"
  pnpm tauri signer sign "$fixture"
) >"$signer_log" 2>&1
then
  sign_succeeded="true"
fi
unset restored_key_password
[[ "$sign_succeeded" == "true" ]] || fail "signing"

signature="$fixture.sig"
[[ -s "$signature" ]] || fail "signature-output"
node -e '
  const fs = require("fs");
  const raw = fs.readFileSync(process.argv[1], "utf8");
  if (raw !== raw.trim()) process.exit(1);
  const encoded = raw;
  if (Buffer.from(encoded, "base64").toString("base64") !== encoded) {
    process.exit(1);
  }
  const decoded = Buffer.from(encoded, "base64")
    .toString("utf8")
    .replace(/\r\n/g, "\n");
  const lines = decoded.endsWith("\n")
    ? decoded.slice(0, -1).split("\n")
    : decoded.split("\n");
  const body = /^[A-Za-z0-9+/=]+$/;
  if (
    lines.length !== 4 ||
    lines[0] !== "untrusted comment: signature from tauri secret key" ||
    !body.test(lines[1]) ||
    !/^trusted comment: timestamp:\d+\tfile:fixture\.txt$/.test(lines[2]) ||
    !body.test(lines[3])
  ) {
    process.exit(1);
  }
' "$signature" || fail "signature-metadata"
config="$repository_root/src-tauri/tauri.conf.json"
positive_log="$private_directory/positive-verification.log"
if ! cargo run \
  --locked \
  --offline \
  --quiet \
  --manifest-path "$repository_root/src-tauri/Cargo.toml" \
  --example verify_updater_signature \
  -- "$config" "$fixture" "$signature" \
  >"$positive_log" 2>&1
then
  fail "signature-verification"
fi
grep -Fx "signature_verified=true" "$positive_log" >/dev/null ||
  fail "signature-verification"

tampered_fixture="$private_directory/tampered-fixture.txt"
cp "$fixture" "$tampered_fixture"
printf '%s\n' "tampered=true" >> "$tampered_fixture"
negative_log="$private_directory/negative-verification.log"
set +e
cargo run \
  --locked \
  --offline \
  --quiet \
  --manifest-path "$repository_root/src-tauri/Cargo.toml" \
  --example verify_updater_signature \
  -- "$config" "$tampered_fixture" "$signature" \
  >"$negative_log" 2>&1
negative_status=$?
set -e
[[ "$negative_status" -eq 4 ]] || fail "tamper-rejection"

public_staging="$(
  mktemp -d \
    "$repository_root/docs/superpowers/reports/.0.4.0-updater-backup-recovery.XXXXXX"
)"
cp "$fixture" "$public_staging/fixture.txt"
cp "$signature" "$public_staging/fixture.txt.sig"
chmod 644 "$public_staging/fixture.txt" "$public_staging/fixture.txt.sig"

public_key_value="$private_directory/public-key-value.txt"
node -e '
  const config = require(process.argv[1]);
  process.stdout.write(config.plugins.updater.pubkey);
' "$config" > "$public_key_value"

sha256_file() {
  shasum -a 256 "$1" |
    awk '{print $1}'
}

locked_version() {
  node -e '
    const fs = require("fs");
    const name = process.argv[1];
    const lock = fs.readFileSync(process.argv[2], "utf8");
    const block = lock
      .split("[[package]]")
      .find((candidate) =>
        new RegExp(`^name = "${name}"$`, "m").test(candidate)
      );
    const version = block?.match(/^version = "([^"]+)"$/m)?.[1];
    if (!version) process.exit(1);
    process.stdout.write(version);
  ' "$1" "$repository_root/src-tauri/Cargo.lock"
}

tauri_cli_version="$(
  pnpm tauri --version |
    awk 'NF { line = $0 } END { print line }'
)"
updater_plugin_version="$(locked_version "tauri-plugin-updater")"
minisign_verify_version="$(locked_version "minisign-verify")"
fixture_sha256="$(sha256_file "$public_staging/fixture.txt")"
signature_sha256="$(sha256_file "$public_staging/fixture.txt.sig")"
public_key_config_sha256="$(sha256_file "$public_key_value")"
verified_at_utc="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
verification_output="$public_staging/verification.json"

EVIDENCE_OUTPUT="$verification_output" \
EVIDENCE_VERIFIED_AT="$verified_at_utc" \
EVIDENCE_SOURCE_COMMIT="$source_commit" \
EVIDENCE_TAURI_CLI_VERSION="$tauri_cli_version" \
EVIDENCE_UPDATER_PLUGIN_VERSION="$updater_plugin_version" \
EVIDENCE_MINISIGN_VERIFY_VERSION="$minisign_verify_version" \
EVIDENCE_FIXTURE_SHA256="$fixture_sha256" \
EVIDENCE_SIGNATURE_SHA256="$signature_sha256" \
EVIDENCE_PUBLIC_KEY_CONFIG_SHA256="$public_key_config_sha256" \
node -e '
  const fs = require("fs");
  const env = process.env;
  const evidence = {
    schema_version: 1,
    verified_at_utc: env.EVIDENCE_VERIFIED_AT,
    source_commit: env.EVIDENCE_SOURCE_COMMIT,
    tauri_cli_version: env.EVIDENCE_TAURI_CLI_VERSION,
    updater_plugin_version: env.EVIDENCE_UPDATER_PLUGIN_VERSION,
    minisign_verify_version: env.EVIDENCE_MINISIGN_VERIFY_VERSION,
    fixture_sha256: env.EVIDENCE_FIXTURE_SHA256,
    signature_sha256: env.EVIDENCE_SIGNATURE_SHA256,
    public_key_config_sha256:
      env.EVIDENCE_PUBLIC_KEY_CONFIG_SHA256,
    encrypted_key_format_valid: true,
    independent_key_source_attested: true,
    separate_passphrase_source_attested: true,
    signature_verified: true,
    tampered_fixture_rejected: true,
  };
  fs.writeFileSync(
    env.EVIDENCE_OUTPUT,
    `${JSON.stringify(evidence, null, 2)}\n`,
    { mode: 0o644 },
  );
'

mv "$public_staging" "$evidence_directory"
public_staging=""
printf 'evidence=%s\n' "$evidence_relative"
printf 'signature_verified=true\n'
printf 'tampered_fixture_rejected=true\n'
```

Run `chmod 755 scripts/verify-updater-signing-backup.sh`.

The original key transfer intentionally uses Bash file-descriptor, `read`, and
`printf` builtins. Do not replace it with `cp`, `cat`, `node`, or another child
process: doing so would expose the restored backup path or encrypted key
content through argv or a pipe. The shell-memory exposure is already disclosed
in the approved design and the variables are unset immediately after the
private copy is created. The no-newline `printf '%s'` form is required: an
empirical recovery drill confirmed that Tauri rejects the same canonical
encrypted key bytes when the private copy gains a trailing newline.

The two `rm -rf` calls are allowed only for directories created by this
invocation and guarded by exact prefixes. Do not broaden either pattern.

- [ ] **Step 4: Run the focused shell contract and fix only contract defects**

Run:

```bash
/bin/bash -n scripts/verify-updater-signing-backup.sh
pnpm exec vitest run src/__tests__/updaterBackupRecovery.spec.ts
```

Expected: Bash syntax succeeds and all recovery-drill tests pass on POSIX. If
BSD and util-linux `script(1)` output differs only by carriage returns, normalize
`\r` in the test harness; do not weaken the TTY requirement or secret-canary
checks.

- [ ] **Step 5: Commit the recovery entry point**

Run:

```bash
git diff --check
git add scripts/verify-updater-signing-backup.sh src/__tests__/updaterBackupRecovery.spec.ts
git diff --cached --check
git commit -m "feat: add updater signing backup recovery drill"
```

Expected: one independently testable script-and-contract commit. Do not create
the real public evidence directory in this task.

---

### Task 3: Establish the immutable, non-secret verifier checkpoint

**Files:**
- Verify: `src-tauri/examples/verify_updater_signature.rs`
- Verify: `scripts/verify-updater-signing-backup.sh`
- Verify: `src/__tests__/updaterBackupRecovery.spec.ts`

**Interfaces:**
- Consumes: committed Tasks 1 and 2.
- Produces: one clean immutable source commit for the real recovery fixture's `source_commit`.
- Preserves: no remote push and no real recovery input yet.

- [ ] **Step 1: Run all non-secret local validation serially**

Run, one command at a time:

```bash
pnpm exec vitest run src/__tests__/updaterBackupRecovery.spec.ts
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --example verify_updater_signature
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: focused tests, full frontend tests, production frontend build, Rust
formatting, full Rust tests, and diff check all pass. Run frontend and Rust
suites serially.

- [ ] **Step 2: Audit the secret and scope boundary**

Run:

```bash
rg -n \
  "(TAURI_SIGNING_PRIVATE_KEY|TAURI_PRIVATE_KEY|restored_key|passphrase|/Users/|/Volumes/)" \
  scripts/verify-updater-signing-backup.sh \
  src/__tests__/updaterBackupRecovery.spec.ts \
  src-tauri/examples/verify_updater_signature.rs
git log --oneline --decorate -5
git status --short --branch
```

Expected:

- matches are variable names, explicit exclusions, test canaries, and approved
  environment-variable plumbing only;
- no literal secret, restored backup path, username, hostname, or medium
  identifier appears;
- implementation commits are present; and
- the worktree is clean.

- [ ] **Step 3: Capture the immutable verifier source commit**

Run:

```bash
verifier_source_commit="$(git rev-parse HEAD)"
test "$(git status --porcelain --untracked-files=all)" = ""
printf '%s\n' "$verifier_source_commit"
```

Expected: one 40-character commit ID. Do not amend or rebase this commit before
the recovery drill.

- [ ] **Step 4: Ask the maintainer to execute the secret-bearing step**

Use AskHuman `ask`, attach
`scripts/verify-updater-signing-backup.sh`, state the exact
`verifier_source_commit`, and ask the maintainer to run only:

```bash
scripts/verify-updater-signing-backup.sh
```

in their own Terminal from this worktree.

The AskHuman message must explicitly say:

- do not paste the private key, passphrase, restored-key path, raw signer log,
  or medium information into AskHuman;
- enter the restored-key path and passphrase only at the script's hidden TTY
  prompts;
- answer after the script reports either the stable success fields or one
  stable named failure field such as `recovery_verification_failed=signing`;
  and
- do not attach the encrypted private key because encrypted key material is
  still sensitive.

Do not invoke the real script through an agent tool and do not request secret
input yourself.

- [ ] **Step 5: Branch on the maintainer-observed result**

If the maintainer reports a stable failure stage:

- verify that no
  `docs/superpowers/reports/0.4.0-updater-backup-recovery` directory exists;
- do not ask for raw logs;
- use `superpowers:systematic-debugging` on the named non-secret stage;
- require confirmation before changing the approved design or secret channel;
  and
- keep the readiness gate `pending-user-evidence`.

If the maintainer reports:

```text
signature_verified=true
tampered_fixture_rejected=true
```

continue to Task 4.

---

### Task 4: Independently replay and commit the public recovery evidence

**Files:**
- Create at runtime: `docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt`
- Create at runtime: `docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt.sig`
- Create at runtime: `docs/superpowers/reports/0.4.0-updater-backup-recovery/verification.json`
- Create: `src/__tests__/updaterBackupRecoveryEvidence.spec.ts`
- Modify: `docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md`

**Interfaces:**
- Consumes: public evidence generated from the immutable Task 3 commit.
- Produces: independently replayable cryptographic evidence and a truthful `verified` backup gate.
- Preserves: no private input or storage identifier enters Git.

- [ ] **Step 1: Inspect the exact public file boundary before reading content**

Run:

```bash
evidence_directory="docs/superpowers/reports/0.4.0-updater-backup-recovery"
test -d "$evidence_directory"
find "$evidence_directory" -maxdepth 1 -type f -print | LC_ALL=C sort
git status --short
```

Expected: exactly the three approved untracked files and no other worktree
change.

- [ ] **Step 2: Add the durable evidence contract**

Create `src/__tests__/updaterBackupRecoveryEvidence.spec.ts`:

```ts
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

interface RecoveryEvidence {
  schema_version: number;
  verified_at_utc: string;
  source_commit: string;
  tauri_cli_version: string;
  updater_plugin_version: string;
  minisign_verify_version: string;
  fixture_sha256: string;
  signature_sha256: string;
  public_key_config_sha256: string;
  encrypted_key_format_valid: boolean;
  independent_key_source_attested: boolean;
  separate_passphrase_source_attested: boolean;
  signature_verified: boolean;
  tampered_fixture_rejected: boolean;
}

const repositoryRoot = process.cwd();
const evidenceDirectory = resolve(
  repositoryRoot,
  "docs/superpowers/reports/0.4.0-updater-backup-recovery",
);

function read(name: string) {
  return readFileSync(join(evidenceDirectory, name));
}

function sha256(contents: Buffer | string) {
  return createHash("sha256").update(contents).digest("hex");
}

describe("updater signing backup recovery evidence", () => {
  it("contains only the approved schema and public facts", () => {
    const fixture = read("fixture.txt").toString("utf8");
    const signature = read("fixture.txt.sig").toString("utf8");
    const evidence = JSON.parse(
      read("verification.json").toString("utf8"),
    ) as RecoveryEvidence;

    expect(Object.keys(evidence).sort()).toEqual([
      "encrypted_key_format_valid",
      "fixture_sha256",
      "independent_key_source_attested",
      "minisign_verify_version",
      "public_key_config_sha256",
      "schema_version",
      "separate_passphrase_source_attested",
      "signature_sha256",
      "signature_verified",
      "source_commit",
      "tampered_fixture_rejected",
      "tauri_cli_version",
      "updater_plugin_version",
      "verified_at_utc",
    ]);
    expect(evidence).toMatchObject({
      schema_version: 1,
      tauri_cli_version: "tauri-cli 2.11.4",
      updater_plugin_version: "2.10.1",
      minisign_verify_version: "0.2.5",
      encrypted_key_format_valid: true,
      independent_key_source_attested: true,
      separate_passphrase_source_attested: true,
      signature_verified: true,
      tampered_fixture_rejected: true,
    });
    expect(evidence.verified_at_utc).toMatch(
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/,
    );
    expect(evidence.source_commit).toMatch(/^[0-9a-f]{40}$/);
    expect(fixture).toMatch(
      /^schema=codex-pulse-updater-backup-recovery-v1\nrepository=qwertyerge\/codex-pulse\nrelease=0\.4\.0\nsource_commit=[0-9a-f]{40}\nverified_at_utc=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\nnonce=[0-9a-f-]{36}\n$/,
    );
    expect(fixture).toContain(`source_commit=${evidence.source_commit}\n`);
    expect(signature).toMatch(/^[A-Za-z0-9+/=]+$/);
    expect(
      Buffer.from(signature.trim(), "base64").toString("base64"),
    ).toBe(signature.trim());
    const decodedSignature = Buffer.from(
      signature.trim(),
      "base64",
    ).toString("utf8").replace(/\r\n/g, "\n");
    const signatureLines = decodedSignature.endsWith("\n")
      ? decodedSignature.slice(0, -1).split("\n")
      : decodedSignature.split("\n");
    expect(signatureLines).toHaveLength(4);
    expect(signatureLines[0]).toBe(
      "untrusted comment: signature from tauri secret key",
    );
    expect(signatureLines[1]).toMatch(/^[A-Za-z0-9+/=]+$/);
    expect(signatureLines[2]).toMatch(
      /^trusted comment: timestamp:\d+\tfile:fixture\.txt$/,
    );
    expect(signatureLines[3]).toMatch(/^[A-Za-z0-9+/=]+$/);
    expect(sha256(read("fixture.txt"))).toBe(evidence.fixture_sha256);
    expect(sha256(read("fixture.txt.sig"))).toBe(
      evidence.signature_sha256,
    );

    const config = JSON.parse(
      readFileSync(
        resolve(repositoryRoot, "src-tauri/tauri.conf.json"),
        "utf8",
      ),
    ) as { plugins: { updater: { pubkey: string } } };
    expect(sha256(config.plugins.updater.pubkey)).toBe(
      evidence.public_key_config_sha256,
    );

    const publicText = [
      fixture,
      signature,
      decodedSignature,
      JSON.stringify(evidence),
    ].join("\n");
    expect(publicText).not.toMatch(
      /\/Users\/|\/Volumes\/|TAURI_SIGNING_PRIVATE_KEY|TAURI_PRIVATE_KEY|restored-updater\.key/,
    );
  });
});
```

The durable Vitest contract intentionally does not require a historical Git
object: CI uses the default shallow `actions/checkout` depth. Task 4 Step 4
performs the stronger source-commit identity check locally without changing CI
checkout scope.

- [ ] **Step 3: Run the structural/hash contract**

Run:

```bash
pnpm exec vitest run src/__tests__/updaterBackupRecoveryEvidence.spec.ts
```

Expected: one evidence test passes. Any schema, hash, source-commit, path, or
version mismatch leaves the gate unverified.

- [ ] **Step 4: Replay positive verification and exact tamper rejection**

Run:

```bash
evidence_directory="docs/superpowers/reports/0.4.0-updater-backup-recovery"
evidence_source_commit="$(
  node -p \
    'require("./docs/superpowers/reports/0.4.0-updater-backup-recovery/verification.json").source_commit'
)"
git diff --exit-code \
  "$evidence_source_commit" \
  -- \
  scripts/verify-updater-signing-backup.sh \
  src-tauri/Cargo.toml \
  src-tauri/Cargo.lock \
  src-tauri/examples/verify_updater_signature.rs
cargo run \
  --locked \
  --offline \
  --quiet \
  --manifest-path src-tauri/Cargo.toml \
  --example verify_updater_signature \
  -- \
  src-tauri/tauri.conf.json \
  "$evidence_directory/fixture.txt" \
  "$evidence_directory/fixture.txt.sig"

tampered_fixture="$(mktemp /tmp/codex-pulse-evidence-tamper.XXXXXX)"
cleanup_tampered_fixture() {
  case "$tampered_fixture" in
    /tmp/codex-pulse-evidence-tamper.*) unlink "$tampered_fixture" ;;
    *) return 1 ;;
  esac
}
trap cleanup_tampered_fixture EXIT
cp "$evidence_directory/fixture.txt" "$tampered_fixture"
printf '%s\n' "tampered=true" >> "$tampered_fixture"
set +e
cargo run \
  --locked \
  --offline \
  --quiet \
  --manifest-path src-tauri/Cargo.toml \
  --example verify_updater_signature \
  -- \
  src-tauri/tauri.conf.json \
  "$tampered_fixture" \
  "$evidence_directory/fixture.txt.sig"
tampered_status=$?
set -e
test "$tampered_status" -eq 4
```

Expected: the script, verifier, manifest, and lockfile are byte-identical to
the evidence source commit; the public fixture exits `0` with
`signature_verified=true`; and the tampered copy exits exactly `4`.

- [ ] **Step 5: Update only the offline backup gate**

In the gate ledger of
`docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md`, replace the
offline backup row with:

```markdown
| Independent offline signing backup | `verified` | The maintainer recovered the encrypted key and separately protected passphrase through the approved TTY-only drill. The public fixture, Tauri signature, sanitized result, and independently replayable hashes are recorded in [`0.4.0-updater-backup-recovery/`](./0.4.0-updater-backup-recovery/). |
```

Replace the version-PR exact-head row with:

```markdown
| Version PR exact-SHA CI and CodeRabbit check | `not-started` | The prior `2aabcf6dfecde3a9f092b0ff61dbfb8558d8f323` head passed its exact checks. The recovery verifier and evidence introduce a new head that requires fresh exact-SHA CI and CodeRabbit evidence before this gate can be reverified. |
```

Replace the `## Offline Backup Gate` body with:

```markdown
## Offline Backup Gate

Status: `verified`.

The maintainer affirmatively attested that the encrypted updater private key
was recovered from storage independent of the developer machine and GitHub
Secrets, and that its passphrase was recovered from separately protected
storage. Neither storage location nor secret material was disclosed.

The approved TTY-only drill:

1. validated the restored file as an encrypted Tauri/rsign secret key;
2. signed a new randomized benign fixture with Tauri CLI;
3. verified the public signature against the updater public key committed in
   `src-tauri/tauri.conf.json`; and
4. proved that the same signature rejects a modified fixture.

The exact source commit, UTC time, tool versions, public-key fingerprint,
fixture hash, signature hash, and boolean results are recorded in
[`0.4.0-updater-backup-recovery/verification.json`](./0.4.0-updater-backup-recovery/verification.json).
The public fixture and signature allow independent replay without access to
the restored private key or passphrase.

No private key, passphrase, private-key hash, restored-key path, username,
hostname, Keychain content, GitHub Secret value, or backup medium identifier
is recorded.
```

Immediately after that body, add:

```markdown
## Offline Backup Recovery Verification

The recovery evidence was checked independently from repository contents:

| Verification | Observed result |
| --- | --- |
| Focused recovery Vitest | `2` files and `11` tests passed. |
| Rust signature verifier | `6` example tests passed. |
| Public fixture replay | Exited `0` with `signature_verified=true`. |
| Modified fixture replay | Exited exactly `4` and was rejected. |
| Full frontend suite | `27` files and `147` tests passed serially. |
| Frontend production build | `vue-tsc --noEmit` and Vite production build passed. |
| Rust formatting and full suite | `cargo fmt --check` and the complete Cargo test target set passed serially. |
| Evidence hashes and schema | Fixture, signature, configured-public-key fingerprint, exact fields, and decoded path-free signature comments matched. |
| Diff check | `git diff --check` passed. |

These results prove backup recoverability and public signature replay. They do
not prove a complete updater bundle, platform installation, publication, or a
production old-to-new update.
```

Near the end of the report, replace:

```text
Offline backup remains `pending-user-evidence`.
```

with:

```text
Offline backup is `verified`.
```

Keep version-PR merge, tag, Draft, artifact inspection, installation,
publication, `0.4.1`, and cross-version update as `not-started`.
Keep the version-PR exact-head gate `not-started` until Task 5 records fresh
recovery-head CI and review evidence.

- [ ] **Step 6: Re-run focused evidence checks and commit**

Run:

```bash
pnpm exec vitest run \
  src/__tests__/updaterBackupRecovery.spec.ts \
  src/__tests__/updaterBackupRecoveryEvidence.spec.ts
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --example verify_updater_signature
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
git diff --check
git add \
  docs/superpowers/reports/0.4.0-updater-backup-recovery \
  docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md \
  src/__tests__/updaterBackupRecoveryEvidence.spec.ts
git diff --cached --check
git diff --cached --name-status
git commit -m "docs: record updater backup recovery evidence"
```

Expected: the focused Vitest command passes `2` files and `11` tests, the full
frontend suite passes `27` files and `147` tests, the Rust example passes `6`
tests, every remaining command succeeds serially, and the commit contains
exactly the three public evidence files, the evidence contract, and the
truthful gate/report update.

---

### Task 5: Run final verification and update the existing PR

**Files:**
- Verify: all Task 1-4 files
- External: existing pull request #19 only

**Interfaces:**
- Consumes: committed verifier, real public evidence, and readiness update.
- Produces: a normally pushed exact head with independently observed CI and review conclusions.
- Preserves: Ready/open/unmerged PR and all later release gates.

- [ ] **Step 1: Run final local verification serially**

Run, one command at a time:

```bash
pnpm exec vitest run \
  src/__tests__/updaterBackupRecovery.spec.ts \
  src/__tests__/updaterBackupRecoveryEvidence.spec.ts
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --example verify_updater_signature
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
git diff --check
git status --short --branch
```

Expected: every command passes and the worktree is clean. Record actual
frontend and Rust test counts without carrying forward older counts.

- [ ] **Step 2: Verify exact commit scope and non-goals**

Run:

```bash
git log --oneline --decorate origin/codex/release-0.4.0..HEAD
git diff --stat origin/codex/release-0.4.0...HEAD
git diff --name-status origin/codex/release-0.4.0...HEAD
git ls-remote --tags origin refs/tags/0.4.0
set +e
gh release view 0.4.0 --repo qwertyerge/codex-pulse
release_status=$?
set -e
test "$release_status" -ne 0
```

Expected: only approved design/plan/verifier/test/public-evidence/readiness
changes are present, no `0.4.0` tag exists, and no `0.4.0` Release exists.

- [ ] **Step 3: Fast-forward push the existing branch**

Run:

```bash
git fetch origin main codex/release-0.4.0
git merge-base --is-ancestor origin/codex/release-0.4.0 HEAD
git push origin HEAD:codex/release-0.4.0
```

Expected: a normal non-force fast-forward push. Do not merge or create another
PR.

- [ ] **Step 4: Update only the human-owned PR body sections**

Preserve the CodeRabbit-managed block. Transform the current human-owned body
through exact guarded replacements:

```bash
gh pr view 19 \
  --repo qwertyerge/codex-pulse \
  --json body \
  --jq .body |
node -e '
  let body = "";
  process.stdin.on("data", (chunk) => { body += chunk; });
  process.stdin.on("end", () => {
    const replacements = [
      [
        "- Add a bootstrap readiness ledger that keeps backup, Draft, installation, publication, and the later `0.4.0` to `0.4.1` production update as separate gates.",
        "- Add a bootstrap readiness ledger that keeps backup, Draft, installation, publication, and the later `0.4.0` to `0.4.1` production update as separate gates.\n- Add a TTY-only backup recovery drill plus public fixture/signature evidence that can be independently replayed.",
      ],
      [
        "- [x] `cargo test --manifest-path src-tauri/Cargo.toml`",
        "- [x] `cargo test --manifest-path src-tauri/Cargo.toml`\n- [x] Updater backup recovery verifier, tamper rejection, and public evidence replay",
      ],
      [
        "No visual behavior changed; this PR changes release versions, a configuration contract, and release-readiness documentation only.",
        "No visual behavior changed; this PR changes release versions, recovery verification tooling, cryptographic evidence, contracts, and release-readiness documentation only.",
      ],
      [
        "The maintainer reports an offline signing backup exists, but independent restore/sign/verify evidence is still pending. Keep this PR open and unmerged. This PR does not create a tag, Draft Release, publication, installation, secret mutation, or old-to-new update claim.",
        "The independent offline signing backup gate is verified by a TTY-only restore/sign/verify drill and replayable public evidence. Keep this PR open and unmerged. This PR does not create a tag, Draft Release, publication, installation, secret mutation, or old-to-new update claim.",
      ],
    ];
    for (const [before, after] of replacements) {
      const first = body.indexOf(before);
      const last = body.lastIndexOf(before);
      if (first === -1 || first !== last) process.exit(1);
      body = body.replace(before, after);
    }
    process.stdout.write(JSON.stringify({ body }));
  });
' |
gh api \
  --method PATCH \
  repos/qwertyerge/codex-pulse/pulls/19 \
  --input -
```

Then verify:

```bash
gh pr view 19 \
  --repo qwertyerge/codex-pulse \
  --json number,url,state,isDraft,headRefName,baseRefName,headRefOid,body
```

Expected: PR #19 remains Ready/open, its human-owned summary and release
boundary reflect verified backup evidence, and the CodeRabbit block remains
present and unchanged.

- [ ] **Step 5: Select and watch only the recovery-evidence head CI run**

Run:

```bash
evidence_head_sha="$(git rev-parse HEAD)"
test "$(
  gh pr view 19 \
    --repo qwertyerge/codex-pulse \
    --json headRefOid \
    --jq .headRefOid
)" = "$evidence_head_sha"

gh run list \
  --repo qwertyerge/codex-pulse \
  --branch codex/release-0.4.0 \
  --workflow CI \
  --limit 10 \
  --json databaseId,headSha,status,conclusion,url

evidence_ci_run_id=""
for attempt in {1..12}; do
  evidence_ci_run_id="$(
    gh run list \
      --repo qwertyerge/codex-pulse \
      --branch codex/release-0.4.0 \
      --workflow CI \
      --limit 10 \
      --json databaseId,headSha,status,conclusion \
      --jq ".[] | select(.headSha == \"$evidence_head_sha\" and (.status != \"completed\" or .conclusion == \"success\")) | .databaseId" |
      head -n 1
  )"
  [[ -n "$evidence_ci_run_id" ]] && break
  sleep 5
done
test -n "$evidence_ci_run_id"
gh run watch \
  "$evidence_ci_run_id" \
  --repo qwertyerge/codex-pulse \
  --exit-status \
  --interval 10
gh pr checks 19 \
  --repo qwertyerge/codex-pulse \
  --watch \
  --interval 10
```

Expected: Frontend, Rust, Rust (Windows), including NSIS/package verification,
and CodeRabbit reach terminal success for the exact recovery-evidence head. A
completed unsuccessful run is never selected.

- [ ] **Step 6: Inspect fresh review evidence**

Run:

```bash
gh pr view 19 \
  --repo qwertyerge/codex-pulse \
  --json headRefOid,statusCheckRollup,reviews,comments
gh api \
  repos/qwertyerge/codex-pulse/pulls/19/comments \
  --paginate
```

If CodeRabbit reports an actionable finding, use
`superpowers:receiving-code-review`, verify the finding against source, and
resolve it before claiming success. A finding that changes
`scripts/verify-updater-signing-backup.sh`,
`src-tauri/examples/verify_updater_signature.rs`, either exact verifier
dependency, or evidence semantics invalidates the immutable drill binding.
Stop through AskHuman before editing, obtain an explicit evidence-retention
decision, and return to Task 3 plus a new real recovery drill. Never silently
overwrite or reuse the old public evidence. A documentation-only correction
that does not affect those boundaries may proceed with fresh local and remote
verification.

If the new head is rate-limited or lacks a fresh substantive review, report
that as a new limitation through AskHuman and stop for a decision. Do not reuse
the maintainer's acceptance of the older head's limitation.

- [ ] **Step 7: Record the exact recovery-evidence head in readiness**

Re-select only a completed successful run for the unchanged evidence head:

```bash
evidence_head_sha="$(git rev-parse HEAD)"
evidence_ci_run_id="$(
  gh run list \
    --repo qwertyerge/codex-pulse \
    --branch codex/release-0.4.0 \
    --workflow CI \
    --limit 10 \
    --json databaseId,headSha,status,conclusion \
    --jq ".[] | select(.headSha == \"$evidence_head_sha\" and .status == \"completed\" and .conclusion == \"success\") | .databaseId" |
    head -n 1
)"
test -n "$evidence_ci_run_id"
gh run view \
  "$evidence_ci_run_id" \
  --repo qwertyerge/codex-pulse \
  --json headSha,status,conclusion,url,jobs
gh pr view 19 \
  --repo qwertyerge/codex-pulse \
  --json headRefOid,state,isDraft,statusCheckRollup,reviews,comments
```

Use `apply_patch` to change only the version-PR exact-head row and the
corresponding `## Version Pull Request Gate` evidence in
`docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md`:

- set the gate back to `verified`;
- record the exact `evidence_head_sha`, `evidence_ci_run_id`, run URL, and
  successful Frontend, Rust, and Rust (Windows) job conclusions;
- record the fresh CodeRabbit conclusion, including an explicitly accepted
  limitation if Task 5 Step 6 required that decision;
- state that this is the recovery code/evidence head; and
- state that the following report-only evidence commit must pass exact-head CI
  again and will be handed off externally to avoid an infinite evidence loop.

Keep PR merge, tag, Draft, artifact inspection, installation, publication,
`0.4.1`, and cross-version update `not-started`.

Run:

```bash
git diff --check
git add docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md
git diff --cached --check
git diff --cached --name-status
git commit -m "docs: record backup recovery PR verification"
```

Expected: exactly one report-only commit whose recorded head and run were
observed before the commit.

- [ ] **Step 8: Push and verify the final report-only head**

Run:

```bash
git fetch origin codex/release-0.4.0
git merge-base --is-ancestor origin/codex/release-0.4.0 HEAD
git push origin HEAD:codex/release-0.4.0

final_report_head_sha="$(git rev-parse HEAD)"
test "$(
  gh pr view 19 \
    --repo qwertyerge/codex-pulse \
    --json headRefOid \
    --jq .headRefOid
)" = "$final_report_head_sha"

final_report_ci_run_id=""
for attempt in {1..12}; do
  final_report_ci_run_id="$(
    gh run list \
      --repo qwertyerge/codex-pulse \
      --branch codex/release-0.4.0 \
      --workflow CI \
      --limit 10 \
      --json databaseId,headSha,status,conclusion \
      --jq ".[] | select(.headSha == \"$final_report_head_sha\" and (.status != \"completed\" or .conclusion == \"success\")) | .databaseId" |
      head -n 1
  )"
  [[ -n "$final_report_ci_run_id" ]] && break
  sleep 5
done
test -n "$final_report_ci_run_id"
gh run watch \
  "$final_report_ci_run_id" \
  --repo qwertyerge/codex-pulse \
  --exit-status \
  --interval 10
gh pr checks 19 \
  --repo qwertyerge/codex-pulse \
  --watch \
  --interval 10
gh pr view 19 \
  --repo qwertyerge/codex-pulse \
  --json headRefOid,statusCheckRollup,reviews,comments
```

Expected: every required check succeeds for the exact report-only head. If
this new head has an actionable finding, apply the same evidence-invalidation
rule from Step 6 before changing verifier or evidence semantics. If it has a
new review limitation, report it through AskHuman and require a fresh
decision. Do not amend the readiness report solely to record this final run.

- [ ] **Step 9: Perform final Git, PR, tag, and Release verification**

Run:

```bash
git fetch origin main codex/release-0.4.0 --prune
git status --short --branch
git rev-parse HEAD origin/codex/release-0.4.0 origin/main
git rev-list --left-right --count origin/codex/release-0.4.0...HEAD
git merge-base --is-ancestor origin/main HEAD
gh pr view 19 \
  --repo qwertyerge/codex-pulse \
  --json number,url,state,isDraft,baseRefName,headRefName,headRefOid,mergeStateStatus,statusCheckRollup
git ls-remote --tags origin refs/tags/0.4.0
set +e
gh release view 0.4.0 --repo qwertyerge/codex-pulse
release_status=$?
set -e
test "$release_status" -ne 0
```

Expected:

- clean worktree;
- local HEAD equals the remote feature branch;
- the branch remains based on `origin/main`;
- PR #19 is Ready, `OPEN`, unmerged, and points to the exact final verified
  head;
- no `0.4.0` tag exists; and
- no `0.4.0` Release exists.

If `origin/main` advanced so it is no longer an ancestor, report the exact
divergence through AskHuman. Do not rebase, force-push, merge, or rewrite the
already open PR branch without fresh approval.

- [ ] **Step 10: Handoff without broadening the release task**

Use AskHuman `whats_next` only after every approved step is complete. Attach
`docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md` and report:

- the immutable verifier source commit;
- the public evidence commit;
- positive signature replay and exact tamper-rejection results;
- focused and full local verification counts;
- exact final PR head and CI run;
- fresh CodeRabbit conclusion or explicitly accepted limitation;
- PR #19 remains Ready/open/unmerged; and
- no secret value/path, tag, Draft, Release, installation, publication, or
  GitHub Secret mutation occurred.
