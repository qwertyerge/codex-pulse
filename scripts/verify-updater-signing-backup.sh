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
