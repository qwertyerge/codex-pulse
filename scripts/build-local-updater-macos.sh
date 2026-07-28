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
