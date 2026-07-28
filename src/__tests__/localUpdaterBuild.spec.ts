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
