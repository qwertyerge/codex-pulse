# Updater Signing Backup Recovery Verification Design

- Date: `2026-07-29`
- Status: approved for implementation planning
- Repository: `qwertyerge/codex-pulse`
- Release gate: first updater-capable `0.4.0` bootstrap

## Context

Codex Pulse already has an updater signing identity configured in
`src-tauri/tauri.conf.json`, GitHub Actions Secrets with the expected signing
secret names, and a local macOS updater build runbook. None of those facts
prove that the independently stored encrypted private key and its separately
stored passphrase can be recovered.

The `0.4.0` readiness report therefore keeps the independent offline signing
backup at `pending-user-evidence`. The exit conditions are:

1. recover the encrypted updater private key from storage independent of the
   developer machine and GitHub Secrets;
2. recover its passphrase from separately protected storage;
3. sign a benign fixture with the restored key; and
4. verify the resulting signature against the public key committed in
   `src-tauri/tauri.conf.json`.

The maintainer confirmed that both recovery inputs are currently available,
without disclosing either input or its location.

## Goal

Add a repeatable, auditable recovery-verification path that lets the
maintainer perform the secret-bearing steps in a local Terminal while the
repository retains only independently reviewable, non-secret evidence.

A successful run must prove that the restored encrypted key and separately
retrieved passphrase can produce a Tauri updater signature that the committed
updater public key accepts.

## Non-Goals

This task does not:

- read or mutate GitHub Secret values;
- read the original developer-machine updater private key;
- use or mutate the existing macOS Keychain item;
- build a complete application or updater bundle;
- prove macOS or Windows installation behavior;
- merge pull request #19;
- create a tag, Draft Release, published Release, or updater manifest;
- install or replace Codex Pulse; or
- prove the later production `0.4.0` to `0.4.1` update lifecycle.

Those remain separate gates.

## Approaches Considered

### Persistent, independently replayable evidence

Add a repository-owned interactive script, a small Rust verification example,
tests, and a public evidence bundle containing the fixture and its signature.

This is the selected approach. It gives future maintainers a repeatable
recovery drill and lets reviewers replay verification without possessing the
private key or passphrase.

### One-off temporary verifier

Build the verifier only in a temporary directory and record hashes plus a
success statement in the readiness report.

This would minimize the branch diff, but a reviewer could not reproduce the
actual cryptographic verification from repository contents and future drills
would have to recreate the tooling.

### Restored-key full updater build

Temporarily place the restored credentials in Keychain and reuse
`scripts/build-local-updater-macos.sh`.

This provides deeper packaging coverage but mutates Keychain, takes
substantially longer, and conflates backup recoverability with full updater
artifact generation. It also violates the selected local silent-interaction
boundary.

## Components

### Interactive recovery entry point

Add:

```text
scripts/verify-updater-signing-backup.sh
```

This is the only secret-bearing entry point. It:

1. requires an interactive TTY;
2. requires the maintainer to attest that the encrypted key came from storage
   independent of the developer machine and GitHub Secrets;
3. requires the maintainer to attest that the passphrase came from separately
   protected storage;
4. silently reads both the restored-key path and passphrase from `/dev/tty`;
5. copies the encrypted key into a permission-restricted temporary directory;
6. creates a random benign challenge fixture;
7. signs the fixture once with the Tauri CLI;
8. invokes the Rust verifier;
9. runs a tampered-fixture negative check; and
10. atomically promotes only public, sanitized evidence after every check
    succeeds.

The script is scoped to the macOS maintainer environment used for this
recovery drill. It does not modify the source backup medium.

### Runtime-equivalent signature verifier

Add:

```text
src-tauri/examples/verify_updater_signature.rs
```

The example accepts explicit paths to:

- `src-tauri/tauri.conf.json`;
- a fixture; and
- the fixture's Tauri `.sig` file.

It extracts `plugins.updater.pubkey` from the configuration, base64-decodes the
configured public-key document, base64-decodes the `.sig` content, parses both
with `minisign-verify`, and verifies the fixture with legacy-signature support
enabled.

This mirrors the locked `tauri-plugin-updater 2.10.1` implementation:

```text
configured public key -> base64 decode -> PublicKey::decode
release signature     -> base64 decode -> Signature::decode
fixture bytes         -> PublicKey::verify(..., allow_legacy = true)
```

Add exact development dependencies for the versions already present in the
Cargo lockfile:

```toml
base64 = "=0.22.1"
minisign-verify = "=0.2.5"
```

The example is verification tooling only. It is not registered as an
application command and does not change updater runtime behavior.

### Public recovery evidence

After a successful real recovery drill, create:

```text
docs/superpowers/reports/0.4.0-updater-backup-recovery/
  fixture.txt
  fixture.txt.sig
  verification.json
```

`fixture.txt` contains only a schema label, repository/release purpose,
source commit, UTC challenge time, and random nonce. It contains no username,
hostname, path, or storage identifier.

`fixture.txt.sig` is the public Tauri updater signature produced by the
restored private key. Publishing it is equivalent to publishing updater
artifact signatures and does not disclose the private key or passphrase.

`verification.json` uses a stable schema and records only:

- evidence schema version;
- UTC verification time;
- source commit;
- Tauri CLI version;
- updater plugin version;
- `minisign-verify` version;
- SHA-256 of the fixture;
- SHA-256 of the public signature;
- SHA-256 fingerprint of the configured public-key value;
- `encrypted_key_format_valid: true`;
- `independent_key_source_attested: true`;
- `separate_passphrase_source_attested: true`;
- `signature_verified: true`; and
- `tampered_fixture_rejected: true`.

The evidence deliberately excludes:

- private-key or passphrase content;
- a hash or fingerprint of the encrypted private key;
- the restored-key path;
- username or hostname;
- backup medium name, mount point, vendor, serial number, or other identifier;
- Keychain content; and
- GitHub Secret values.

The durable `0.4.0` readiness report links to this evidence and changes the
offline backup gate only after the real drill and independent replay both
succeed.

## Secret Lifecycle

The script disables shell tracing before reading any sensitive input and does
not offer a real-secret debug mode.

The restored-key path and passphrase are read silently from `/dev/tty`. The
path is never passed in command-line arguments. The passphrase is never passed
with `--password`, printed, persisted, or written to shell history.

The restored encrypted key is copied to a `mktemp` directory with directory
mode `0700` and file mode `0600`. The script validates that the copy uses the
encrypted Tauri/rsign secret-key format without printing decoded key material.

The Tauri CLI has no password-from-stdin interface. Therefore, for the single
signing subprocess only, the script supplies:

```text
TAURI_SIGNING_PRIVATE_KEY_PATH
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

It explicitly removes conflicting signing-key environment variables before
launching the signer and unsets the secret-bearing variables immediately
afterward.

This design minimizes exposure but does not claim memory zeroization. During
the signing call, the shell and Tauri CLI necessarily hold the secret path or
passphrase in process memory or environment. A compromised process with
sufficient access under the same user account is outside this drill's threat
model.

Raw signer stdout and stderr stay in the permission-restricted temporary
directory and are never promoted to evidence. Failure output identifies only
the stable failed stage, because upstream error messages may include the
restored-key path.

An exit trap unsets secret-bearing shell variables and deletes the exact
temporary directory created by this run.

## Preconditions and Fail-Closed Behavior

The script fails before requesting secrets unless:

- it runs from the repository root on macOS;
- `/dev/tty` is available;
- the worktree is clean;
- the expected `0.4.0` configuration and public key are present;
- required local tools are available;
- the public evidence destination does not already exist; and
- the two source-separation attestations are accepted.

After secrets are requested, any of these conditions fails closed:

- restored key is absent, unreadable, not a regular file, or a symlink;
- encrypted-key format validation fails;
- signing fails;
- expected `.sig` output is absent;
- verification against the committed public key fails;
- the tampered fixture is incorrectly accepted;
- an evidence hash cannot be computed; or
- atomic evidence promotion fails.

Failure returns non-zero, removes transient data, writes no successful public
evidence, and leaves the readiness gate unchanged.

The script never overwrites an existing evidence directory. Re-running a
completed drill requires an explicit, separately reviewed evidence-retention
decision.

## Test Strategy

### Shell contract and orchestration

Add a focused Vitest suite that executes the script with fake tools and a
pseudo-terminal boundary where required. It covers:

- non-interactive invocation rejection;
- both source-separation attestations are mandatory;
- key path and passphrase reach the fake signer only through environment
  variables, never argv;
- conflicting inherited signing variables are removed;
- restored key is copied to a private temporary location;
- raw signer output is not copied into evidence;
- secret canaries do not appear in stdout, stderr, fixture, signature,
  verification JSON, or repository paths;
- signing failure, verification failure, and unexpected negative-check
  success produce no evidence;
- an existing evidence directory is never overwritten; and
- success promotes exactly the three approved public files.

### Rust verifier

Rust tests use safe public test vectors to cover:

- valid Tauri-encoded signature accepted;
- malformed base64 rejected;
- wrong key ID rejected;
- modified fixture rejected;
- modified signature rejected; and
- missing updater public-key configuration rejected.

### Real recovery evidence replay

After the maintainer completes the interactive drill:

1. run the Rust example directly against the public fixture, signature, and
   committed configuration;
2. mutate a temporary fixture copy and prove verification fails;
3. validate every declared SHA-256 value;
4. scan evidence for prohibited user-specific and secret-bearing fields; and
5. confirm the readiness report references the exact evidence source commit.

### Full repository verification

Run serially:

```text
pnpm exec vitest run <focused recovery specs>
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

The full application build and local updater bundle build are intentionally
outside this gate.

## Delivery Sequence

1. Implement the verifier and tests with test-driven development.
2. Run the non-secret focused and full verification suites.
3. Commit the verifier implementation locally so the drill is bound to an
   immutable source commit.
4. Ask the maintainer to run the script from that exact commit in their local
   Terminal and enter recovery material only there.
5. Independently replay and negatively test the generated public evidence.
6. Update the readiness report from `pending-user-evidence` to `verified` only
   if all approved exit conditions are met.
7. Commit the public evidence and readiness update.
8. Push the existing `codex/release-0.4.0` branch; do not create another pull
   request.
9. Update pull request #19's evidence summary and wait for exact-head
   Frontend, Rust, Rust (Windows), and CodeRabbit conclusions.

Any new actionable review finding must be resolved and reverified. Any new
whole-head review limitation must be reported as a fresh limitation; the
maintainer's prior acceptance for an older head does not automatically apply.

Pull request #19 remains Ready, open, and unmerged. Merge, tag, Draft,
installation, publication, and secret mutation require separate approval.

## Acceptance Criteria

The recovery gate is `verified` only when:

- the maintainer affirmatively attests to independent restored-key and
  separately protected passphrase sources;
- the restored key is confirmed to be an encrypted Tauri updater key;
- Tauri CLI signs a new benign challenge fixture;
- the locked runtime-equivalent verifier accepts that signature against the
  exact public key committed in `src-tauri/tauri.conf.json`;
- the same verifier rejects a modified fixture;
- public evidence contains no prohibited secret or location data;
- an independent replay from repository contents succeeds;
- focused and full local verification pass;
- the evidence and readiness update are committed and pushed to pull request
  #19; and
- exact-head CI and review conclusions are recorded without conflating them
  with later release gates.

## References

- `docs/superpowers/specs/2026-07-28-automatic-updates-design.md`
- `docs/superpowers/specs/2026-07-28-0.4.0-updater-bootstrap-release-design.md`
- `docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md`
- `scripts/build-local-updater-macos.sh`
- [Tauri updater signing documentation](https://v2.tauri.app/plugin/updater/)
- `tauri-plugin-updater 2.10.1` locked source,
  `src/updater.rs::verify_signature`
- `minisign-verify 0.2.5` locked source
