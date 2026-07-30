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
  delayedReadSetup?: boolean;
  exitSuccessfullyDuringHiddenRead?: boolean;
  existingEvidence?: boolean;
  signalDuringHiddenRead?: "HUP" | "INT" | "TERM";
  signatureMetadataInvalid?: boolean;
  signerFails?: boolean;
  verifierFails?: boolean;
  tamperedAccepted?: boolean;
  ttyRestoreFails?: boolean;
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
  return ["-q", "-e", "-c", `/bin/bash ${scriptPath}`, "/dev/null"];
}

const darwinExpectProgram = String.raw`
set timeout 15
log_user 1
set script_path $env(CODEX_PTY_SCRIPT_PATH)
set response_count $env(CODEX_PTY_RESPONSE_COUNT)
set responses [list]
for {set index 0} {$index < $response_count} {incr index} {
  set response_key "CODEX_PTY_RESPONSE_$index"
  lappend responses [set env($response_key)]
  unset env($response_key)
}
unset env(CODEX_PTY_SCRIPT_PATH)
unset env(CODEX_PTY_RESPONSE_COUNT)
set prompts [list \
  {Type yes to attest independent key recovery:} \
  {Type yes to attest separate passphrase recovery:} \
  {Restored encrypted key path (hidden):} \
  {Restored key passphrase (hidden):} \
]
spawn -noecho /usr/bin/script -q /dev/null /bin/bash $script_path
set saw_eof 0
for {set index 0} {$index < [llength $responses]} {incr index} {
  set prompt [lindex $prompts $index]
  set reached_prompt 0
  expect {
    -exact $prompt { set reached_prompt 1 }
    eof { set saw_eof 1 }
    timeout { exit 124 }
  }
  if {!$reached_prompt} {
    break
  }
  send -- "[lindex $responses $index]\r"
}
if {!$saw_eof} {
  expect {
    eof {}
    timeout { exit 124 }
  }
}
set result [wait]
exit [lindex $result 3]
`;

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
    const darwinEnvironment = {
      ...env,
      CODEX_PTY_SCRIPT_PATH: scriptPath,
      CODEX_PTY_RESPONSE_COUNT: String(responses.length),
      ...Object.fromEntries(
        responses.map((response, index) => [
          `CODEX_PTY_RESPONSE_${index}`,
          response,
        ]),
      ),
    };
    const child = spawn(
      process.platform === "darwin" ? "/usr/bin/expect" : "script",
      process.platform === "darwin"
        ? ["-c", darwinExpectProgram]
        : ptyArguments(scriptPath),
      {
        cwd,
        env: process.platform === "darwin" ? darwinEnvironment : env,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );
    let stdout = "";
    let stderr = "";
    let responseIndex = 0;
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("recovery script pseudo-terminal timed out"));
    }, 15_000);

    const answerPrompts = () => {
      if (process.platform === "darwin") return;
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

  const ptyDriver = join(fixtureRoot, "run-recovery-in-pty.sh");
  writeExecutable(
    ptyDriver,
    [
      "#!/usr/bin/env bash",
      "set +e",
      'driver_directory="$(',
      '  cd "$(dirname "${BASH_SOURCE[0]}")"',
      "  pwd -P",
      ')"',
      'child_environment_contains_response="false"',
      "for environment_name in $(compgen -e); do",
      '  environment_value="${!environment_name}"',
      '  case "$environment_value" in',
      "    *passphrase-canary*|*/restored.key)",
      '      child_environment_contains_response="true"',
      "      ;;",
      "  esac",
      "done",
      "printf 'child_environment_contains_response=%s\\n' \\",
      '  "$child_environment_contains_response"',
      'original_tty_state="$(stty -g < /dev/tty)"',
      '/bin/bash "$driver_directory/scripts/verify-updater-signing-backup.sh"',
      'recovery_status="$?"',
      'restored_tty_state="$(stty -g < /dev/tty)"',
      'if [[ "$restored_tty_state" == "$original_tty_state" ]]; then',
      "  printf 'tty_state_restored=true\\n'",
      "else",
      "  printf 'tty_state_restored=false\\n'",
      "fi",
      'exit "$recovery_status"',
    ].join("\n"),
  );

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

  if (options.ttyRestoreFails) {
    writeStub(
      fakeBin,
      "stty",
      [
        'if [[ "${FAKE_STTY_RESTORE_FAILS:-0}" = "1" && "$#" -eq 1 && "$1" != "-g" && "$1" != "-echo" ]]; then',
        '  printf "tty_restore_failure_injected=true\\n" >&2',
        "  exit 1",
        "fi",
        'exec /bin/stty "$@"',
      ].join("\n"),
    );
  }

  writeStub(fakeBin, "uname", 'printf "Darwin\\n"');
  writeStub(
    fakeBin,
    "uuidgen",
    'printf "11111111-2222-4333-8444-555555555555\\n"',
  );
  writeStub(
    fakeBin,
    "cp",
    [
      'case "${1:-}" in',
      "  */restored.key) exit 45 ;;",
      "esac",
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
      `test "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" = ${JSON.stringify(passwordCanary)}`,
      'case "$TAURI_SIGNING_PRIVATE_KEY_PATH" in',
      "  */restored.key) exit 46 ;;",
      "esac",
      'node -e \'const fs=require("fs"); const mode=fs.statSync(process.argv[1]).mode & 0o777; if (mode !== 0o600) process.exit(42)\' "$TAURI_SIGNING_PRIVATE_KEY_PATH"',
      'node -e \'const fs=require("fs"); const bytes=fs.readFileSync(process.argv[1]); if (bytes.at(-1) === 10 || bytes.at(-1) === 13) process.exit(44)\' "$TAURI_SIGNING_PRIVATE_KEY_PATH"',
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

  const bashEnvironment = join(fixtureRoot, "delay-read-setup.sh");
  const usesReadWrapper = Boolean(
    options.delayedReadSetup ||
      options.exitSuccessfullyDuringHiddenRead ||
      options.signalDuringHiddenRead,
  );
  if (usesReadWrapper) {
    writeFileSync(
      bashEnvironment,
      [
        "read() {",
        '  local prompt=""',
        "  local saw_prompt=0",
        "  local saw_silent=0",
        "  local -a read_arguments=()",
        '  while [[ "$#" -gt 0 ]]; do',
        '    case "$1" in',
        "      -p)",
        '        prompt="$2"',
        "        saw_prompt=1",
        "        shift 2",
        "        ;;",
        "      -s)",
        "        saw_silent=1",
        '        read_arguments[${#read_arguments[@]}]="$1"',
        "        shift",
        "        ;;",
        "      *)",
        '        read_arguments[${#read_arguments[@]}]="$1"',
        "        shift",
        "        ;;",
        "    esac",
        "  done",
        '  if [[ "$saw_prompt" -eq 1 ]]; then',
        '    printf "%s" "$prompt" > /dev/tty',
        "  fi",
        '  if [[ "$saw_silent" -eq 1 && -n "${FAKE_READ_SIGNAL:-}" ]]; then',
        '    printf "signal_injected=true\\n"',
        "    sleep 0.2",
        '    kill "-$FAKE_READ_SIGNAL" "$$"',
        "  fi",
        '  if [[ "$saw_silent" -eq 1 && "${FAKE_READ_EXIT_SUCCESS:-0}" = "1" ]]; then',
        '    printf "hidden_read_success_exit_injected=true\\n"',
        "    exit 0",
        "  fi",
        "  sleep 0.2",
        '  builtin read "${read_arguments[@]}"',
        "}",
        "export -f read",
        "",
      ].join("\n"),
    );
  }

  const environment: NodeJS.ProcessEnv = {
    ...process.env,
    ...(usesReadWrapper ? { BASH_ENV: bashEnvironment } : {}),
    PATH: `${fakeBin}:${process.env.PATH ?? ""}`,
    TMPDIR: `${privateTempDirectory}/`,
    FAKE_AUDIT_DIRECTORY: auditDirectory,
    FAKE_INVALID_PUBLIC_SIGNATURE: invalidPublicSignature,
    FAKE_PUBLIC_SIGNATURE: fakePublicSignature,
    FAKE_READ_EXIT_SUCCESS:
      options.exitSuccessfullyDuringHiddenRead ? "1" : "0",
    FAKE_READ_SIGNAL: options.signalDuringHiddenRead ?? "",
    FAKE_SIGNATURE_METADATA_INVALID:
      options.signatureMetadataInvalid ? "1" : "0",
    FAKE_SIGNER_FAILS: options.signerFails ? "1" : "0",
    FAKE_STTY_RESTORE_FAILS: options.ttyRestoreFails ? "1" : "0",
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
      runInPty(ptyDriver, fixtureRoot, environment, responses),
  };
}

function allEvidenceText(directory: string) {
  if (!existsSync(directory)) return "";
  return readdirSync(directory)
    .map((name) => readFileSync(join(directory, name), "utf8"))
    .join("\n");
}

function expectNoSecrets(
  result: TextResult,
  harness: Harness,
  ttyRestored = true,
) {
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
  expect(`${result.stdout}\n${result.stderr}`).toContain(
    `tty_state_restored=${String(ttyRestored)}`,
  );
  expect(`${result.stdout}\n${result.stderr}`).not.toContain(
    `tty_state_restored=${String(!ttyRestored)}`,
  );
  expect(`${result.stdout}\n${result.stderr}`).toContain(
    "child_environment_contains_response=false",
  );
  expect(`${result.stdout}\n${result.stderr}`).not.toContain(
    "child_environment_contains_response=true",
  );
}

afterEach(() => {
  for (const fixture of temporaryFixtures.splice(0)) {
    rmSync(fixture, { recursive: true, force: true });
  }
});

const describeOnPosix =
  process.platform === "win32" ? describe.skip : describe;

describeOnPosix("updater signing backup recovery drill", { timeout: 15_000 }, () => {
  it("prints help without requesting a secret", () => {
    expect(existsSync(sourceScript)).toBe(true);
    expect(statSync(sourceScript).mode & constants.S_IXUSR).toBeTruthy();
    const result = spawnSync("/bin/bash", [sourceScript, "--help"], {
      cwd: repositoryRoot,
      env: { PATH: "" },
      encoding: "utf8",
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain(
      "Verify a restored updater signing backup",
    );
  });

  it("rejects a non-interactive real run", () => {
    const fixtureRoot = mkdtempSync(
      join(tmpdir(), "codex-pulse-backup-noninteractive-"),
    );
    temporaryFixtures.push(fixtureRoot);
    const fakeBin = join(fixtureRoot, "fake-bin");
    for (const command of [
      "awk",
      "cargo",
      "chmod",
      "cp",
      "date",
      "dirname",
      "git",
      "grep",
      "mktemp",
      "mv",
      "node",
      "pnpm",
      "rm",
      "shasum",
      "tr",
      "uname",
      "uuidgen",
    ]) {
      writeExecutable(
        join(fakeBin, command),
        command === "uname"
          ? '#!/bin/bash\nprintf "Darwin\\n"'
          : "#!/bin/bash\nexit 99",
      );
    }
    const result = spawnSync("/bin/bash", [sourceScript], {
      cwd: repositoryRoot,
      env: { PATH: `${fakeBin}:/bin:/usr/bin` },
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
    expect(
      JSON.parse(
        readFileSync(
          join(harness.evidenceDirectory, "verification.json"),
          "utf8",
        ),
      ),
    ).toMatchObject({
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

  it("keeps hidden input silent across Apple Bash's prompt-to-noecho delay", async () => {
    const harness = createHarness({ delayedReadSetup: true });
    const result = await harness.run();

    expect(result.status).toBe(0);
    expectNoSecrets(result, harness);
  });

  it.each([
    ["HUP", 129],
    ["INT", 130],
    ["TERM", 143],
  ] as const)(
    "restores the terminal after %s during hidden input",
    async (signal, expectedStatus) => {
      const harness = createHarness({
        delayedReadSetup: true,
        signalDuringHiddenRead: signal,
      });
      const result = await harness.run();

      expect(result.status).toBe(expectedStatus);
      expect(`${result.stdout}\n${result.stderr}`).toContain(
        "signal_injected=true",
      );
      expect(existsSync(harness.evidenceDirectory)).toBe(false);
      expect(readdirSync(harness.privateTempDirectory)).toEqual([]);
      expectNoSecrets(result, harness);
    },
  );

  it("fails closed when EXIT cleanup cannot restore the terminal", async () => {
    const harness = createHarness({
      delayedReadSetup: true,
      exitSuccessfullyDuringHiddenRead: true,
      ttyRestoreFails: true,
    });
    const result = await harness.run();
    const output = `${result.stdout}\n${result.stderr}`;

    expect(result.status).toBe(1);
    expect(output).toContain("hidden_read_success_exit_injected=true");
    expect(output).toContain("tty_restore_failure_injected=true");
    expect(output).toContain("recovery_cleanup_failed=tty");
    expect(output).not.toContain("signature_verified=true");
    expect(existsSync(harness.evidenceDirectory)).toBe(false);
    expect(readdirSync(harness.privateTempDirectory)).toEqual([]);
    expectNoSecrets(result, harness, false);
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
