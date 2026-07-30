# Updater Recovery TTY Race Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close Apple Bash 3.2's prompt-before-noecho race in the updater
backup-recovery drill, restore the exact terminal state on every supported exit
path, fail closed if that restoration cannot complete, correct the readiness
record, and merge PR #19 only after fresh exact-head verification.

**Architecture:** Keep the existing `/bin/bash` and `/dev/tty` boundary.
`read_hidden` explicitly saves the terminal state and disables echo before the
Bash builtin can emit its prompt; normal flow restores immediately, while
`EXIT` cleanup and explicit catchable-signal traps cover premature termination.
The exit handler preserves existing failures and overrides an otherwise
successful exit when cleanup fails. The PTY harness reproduces Apple's exact
prompt/noecho ordering with canaries, identifies hidden reads by their `-s`
contract, and verifies the terminal after the child script exits.

**Tech Stack:** Apple Bash 3.2, POSIX TTY utilities, Vue/Vitest TypeScript test
harness, GitHub CLI, GitHub Actions.

## Global Constraints

- Keep `docs/superpowers/reports/0.4.0-updater-backup-recovery/{fixture.txt,fixture.txt.sig,verification.json}` byte-for-byte unchanged.
- Do not request, print, persist, hash, or disclose a real key, passphrase, recovery path, signer log, or backup-medium detail.
- Do not change signing identity, public key, key parsing/copy bytes, signer invocation/environment, Rust verifier, verifier dependencies, fixture semantics, or evidence schema.
- Support the host `/bin/bash` Apple `bash-142` / Bash `3.2.57`.
- Preserve the existing interactive `/dev/tty`-only production interface.
- Do not create a tag or Release, install an application, publish artifacts, mutate GitHub Secrets, push `main`, or remove the host-managed worktree.
- Merge only PR #19, by squash, and only when its remote head equals the reviewed commit and every required exact-head gate is successful.

---

## File Map

- Modify `src/__tests__/updaterBackupRecovery.spec.ts`: deterministic Apple
  Bash race injection, semantic hidden-read matching, PTY-state driver, signal
  and restoration-failure cases, and canary assertions.
- Modify `scripts/verify-updater-signing-backup.sh`: explicit TTY state
  lifecycle, fail-closed cleanup, and `HUP`/`INT`/`TERM` routing through cleanup.
- Modify
  `docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md`: correct the
  version-only scope paragraph and the incomplete prompt-race timeline.
- Preserve without edits
  `src-tauri/examples/verify_updater_signature.rs` and the three public
  recovery-evidence files.

### Task 1: Make the Apple Bash race and exit cleanup observable

**Files:**
- Modify: `src/__tests__/updaterBackupRecovery.spec.ts:19-185`
- Modify: `src/__tests__/updaterBackupRecovery.spec.ts:187-425`
- Test: `src/__tests__/updaterBackupRecovery.spec.ts:531-537`

**Interfaces:**
- Consumes: the real copied recovery script and a real pseudo-terminal.
- Produces: `signalDuringHiddenRead?: "HUP" | "INT" | "TERM"` in
  `HarnessOptions`; `signal_injected=true` proves semantic injection into
  `read -s`; each ordinary harness result contains `tty_state_restored=true`;
  `delayedReadSetup` reproduces Apple's ordering; the recovery describe block
  uses a 15-second Vitest timeout consistent with its existing PTY watchdog and
  the repository's other external-process suite.

- [ ] **Step 1: Add a PTY driver that compares terminal state around the script**

In `createHarness`, write an executable `run-recovery-in-pty.sh` next to the
fixture:

```bash
#!/usr/bin/env bash
set +e
driver_directory="$(
  cd "$(dirname "${BASH_SOURCE[0]}")"
  pwd -P
)"
original_tty_state="$(stty -g < /dev/tty)"
/bin/bash "$driver_directory/scripts/verify-updater-signing-backup.sh"
recovery_status="$?"
restored_tty_state="$(stty -g < /dev/tty)"
if [[ "$restored_tty_state" == "$original_tty_state" ]]; then
  printf 'tty_state_restored=true\n'
else
  printf 'tty_state_restored=false\n'
fi
exit "$recovery_status"
```

Pass this driver, rather than the recovery script directly, to `runInPty`.
Extend `expectNoSecrets` with:

```ts
expect(`${result.stdout}\n${result.stderr}`).toContain(
  "tty_state_restored=true",
);
expect(`${result.stdout}\n${result.stderr}`).not.toContain(
  "tty_state_restored=false",
);
```

- [ ] **Step 2: Replace the pre-builtin delay with Apple's real ordering**

Extend `HarnessOptions`:

```ts
exitSuccessfullyDuringHiddenRead?: boolean;
signalDuringHiddenRead?: "HUP" | "INT" | "TERM";
ttyRestoreFails?: boolean;
```

Generate `BASH_ENV` whenever `delayedReadSetup`,
`exitSuccessfullyDuringHiddenRead`, or `signalDuringHiddenRead` is set. The
exported wrapper must remove `-p`, emit that prompt itself, identify the hidden
read from its `-s` option, delay while echo is still in its current state, and
only then enter the builtin:

```bash
read() {
  local prompt=""
  local saw_prompt=0
  local saw_silent=0
  local -a read_arguments=()
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      -p)
        prompt="$2"
        saw_prompt=1
        shift 2
        ;;
      -s)
        saw_silent=1
        read_arguments[${#read_arguments[@]}]="$1"
        shift
        ;;
      *)
        read_arguments[${#read_arguments[@]}]="$1"
        shift
        ;;
    esac
  done
  if [[ "$saw_prompt" -eq 1 ]]; then
    printf '%s' "$prompt" > /dev/tty
  fi
  if [[ "$saw_silent" -eq 1 && -n "${FAKE_READ_SIGNAL:-}" ]]; then
    printf 'signal_injected=true\n'
    sleep 0.2
    kill "-$FAKE_READ_SIGNAL" "$$"
  fi
  if [[ "$saw_silent" -eq 1 && "${FAKE_READ_EXIT_SUCCESS:-0}" = "1" ]]; then
    printf 'hidden_read_success_exit_injected=true\n'
    exit 0
  fi
  sleep 0.2
  builtin read "${read_arguments[@]}"
}
export -f read
```

Set `FAKE_READ_SIGNAL` to the selected signal or an empty string, and use
`FAKE_READ_EXIT_SUCCESS=1` only for the cleanup-failure regression. Keep the
four response values only in the parent Expect environment, which already
unsets each one before spawning the recovery child.

- [ ] **Step 3: Specify delayed-input and signal behavior**

Change the suite declaration to:

```ts
describeOnPosix(
  "updater signing backup recovery drill",
  { timeout: 15_000 },
  () => {
```

Replace the suite's final `});` with `  },` followed by `);`.

Rename the existing delayed test to:

```ts
it("keeps hidden input silent across Apple Bash's prompt-to-noecho delay", async () => {
  const harness = createHarness({ delayedReadSetup: true });
  const result = await harness.run();

  expect(result.status).toBe(0);
  expectNoSecrets(result, harness);
});
```

Add the catchable-signal contract:

```ts
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
```

Add a fail-closed regression that makes the semantic hidden-read wrapper exit
successfully while a fake `stty` rejects restoration. It must expect status
`1`, `recovery_cleanup_failed=tty`, no `signature_verified=true`, no evidence,
and `tty_state_restored=false`. Before the production change, this test must
fail with `expected 1, received 0`.

- [ ] **Step 4: Run the focused race test and verify RED**

Run:

```bash
pnpm exec vitest run src/__tests__/updaterBackupRecovery.spec.ts \
  -t "keeps hidden input silent across Apple Bash's prompt-to-noecho delay"
```

Expected: FAIL because either `restoredKeyPath` or `passwordCanary` appears
in the pseudo-terminal output after the wrapper emits the prompt and before
the current builtin disables echo. The failure must not be a timeout, syntax
error, or missing fixture.

- [ ] **Step 5: Run one signal case and verify RED**

Run:

```bash
pnpm exec vitest run src/__tests__/updaterBackupRecovery.spec.ts \
  -t "restores the terminal after TERM during hidden input"
```

Expected: FAIL on the no-secret assertion against the same prompt/noecho
window. Record that the PTY driver itself still reports the observed terminal
state; do not weaken the canary assertion.

### Task 2: Guard echo before any hidden prompt

**Files:**
- Modify: `scripts/verify-updater-signing-backup.sh:37-58`
- Modify: `scripts/verify-updater-signing-backup.sh:102-165`
- Test: `src/__tests__/updaterBackupRecovery.spec.ts`

**Interfaces:**
- Consumes: `/dev/tty` and `stty`.
- Produces: `saved_tty_state`, `restore_tty`, fail-closed `on_exit`, silent
  hidden reads, and signal exits `129`, `130`, and `143`.

- [ ] **Step 1: Require `stty` and define the restorable state**

Add `stty` to the sorted required-command list. Define
`saved_tty_state=""` before `cleanup`, followed by:

```bash
restore_tty() {
  [[ -n "$saved_tty_state" ]] || return 0
  stty "$saved_tty_state" < /dev/tty || return 1
  saved_tty_state=""
}
```

The state string is never printed or promoted.

- [ ] **Step 2: Make cleanup fail closed and route signals to EXIT**

Track TTY restoration failure without skipping the remaining cleanup:

```bash
cleanup() {
  local cleanup_status=0

  if ! restore_tty; then
    printf 'recovery_cleanup_failed=tty\n' >&2
    cleanup_status=1
  fi
  unset saved_tty_state

  # Preserve the existing secret unsets and bounded directory removal.

  return "$cleanup_status"
}
```

Replace the direct EXIT trap with a handler that preserves any existing
non-zero status but overrides an otherwise successful exit when cleanup fails:

```bash
on_exit() {
  local original_status=$?
  local cleanup_status=0

  trap - EXIT
  cleanup || cleanup_status=$?
  if [[ "$original_status" -ne 0 ]]; then
    exit "$original_status"
  fi
  exit "$cleanup_status"
}
trap on_exit EXIT
```

Keep the signal routes immediately afterward:

```bash
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
```

These explicit exits are required because Apple Bash does not run an `EXIT`
trap for an otherwise unhandled terminating signal. On the successful path,
call `cleanup`, require status `0`, disarm the EXIT trap, and only then emit
`evidence=...`, `signature_verified=true`, and
`tampered_fixture_rejected=true`.

- [ ] **Step 3: Implement guarded hidden input**

Replace `read_hidden` with:

```bash
read_hidden() {
  local prompt="$1"
  local destination="$2"
  local read_status=0

  saved_tty_state="$(stty -g < /dev/tty)" ||
    fail "tty-state"
  stty -echo < /dev/tty ||
    fail "tty-state"
  IFS= read -r -s -p "$prompt " "$destination" < /dev/tty ||
    read_status=$?
  if ! printf '\n' > /dev/tty; then
    read_status=1
  fi
  restore_tty ||
    fail "tty-restore"
  return "$read_status"
}
```

Do not replace `read -s -p`: the explicit guard closes the pre-prompt window,
while `-s` remains defense in depth.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```bash
/bin/bash -n scripts/verify-updater-signing-backup.sh
pnpm exec vitest run src/__tests__/updaterBackupRecovery.spec.ts
```

Expected: Bash syntax passes; the recovery suite passes, including all three
semantic signal rows, `signal_injected=true`, ordinary
`tty_state_restored=true`, the injected restoration failure returning `1`, and
every no-secret assertion.

- [ ] **Step 5: Commit the tested behavior**

```bash
git add scripts/verify-updater-signing-backup.sh \
  src/__tests__/updaterBackupRecovery.spec.ts
git diff --cached --check
git commit -m "fix: guard updater recovery terminal echo"
```

### Task 3: Correct readiness text without changing evidence

**Files:**
- Modify:
  `docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md:162-189`
- Modify:
  `docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md:247-261`
- Preserve:
  `docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt`
- Preserve:
  `docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt.sig`
- Preserve:
  `docs/superpowers/reports/0.4.0-updater-backup-recovery/verification.json`

**Interfaces:**
- Consumes: the verified historical commits and the final focused test count.
- Produces: an accurate historical account that separates version preparation,
  first prompt hardening, and the Apple Bash premerge correction.

- [ ] **Step 1: Correct the version-preparation scope paragraph**

State that commit `795bb0f1a7f447d15b7bf66c88b4f0649a634c7e`
contains exactly the four version-source changes, while the final PR also
contains the approved recovery design/plan, verifier dependencies, recovery
script/tests, immutable evidence, line-ending contract, and readiness records.
Do not continue to call the final branch a version-only diff.

- [ ] **Step 2: Correct the prompt-hardening timeline**

Record three distinct facts:

1. Ubuntu exposed the original gap between a separately printed prompt and
   entry into `read -s`.
2. `0102453a8fe5c358f4b891a7696aadc484bc6275` moved prompt output into
   `read -s -p`, closing that cross-shell gap but not Apple Bash's internal
   prompt-before-`ttnoecho()` order.
3. Premerge source review on Apple `bash-142` found the remaining internal
   window; the explicit pre-prompt `stty -echo` guard, exact-state restoration,
   signal cleanup, and deterministic PTY regression close it without changing
   cryptographic evidence boundaries.

Update focused test counts only from fresh command output.

- [ ] **Step 3: Prove public evidence is unchanged**

Run:

```bash
git diff --exit-code ed4aea500e8999f179216d775036878fbc89530a -- \
  docs/superpowers/reports/0.4.0-updater-backup-recovery
shasum -a 256 \
  docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt \
  docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt.sig \
  docs/superpowers/reports/0.4.0-updater-backup-recovery/verification.json
```

Expected: the Git diff against the evidence-introducing commit is empty and
the hashes equal the values already recorded in `verification.json` and the
readiness report. Keep the distinct fixture `source_commit`
`ca6f551867dab8b4ec34eca7df8a1c958d6a3e0c` unchanged.

- [ ] **Step 4: Commit the report correction**

```bash
git add docs/superpowers/reports/0.4.0-updater-bootstrap-readiness.md
git diff --cached --check
git commit -m "docs: correct updater recovery tty evidence"
```

### Task 4: Verify, review, and publish the corrected PR head

**Files:**
- Verify all changed files; do not create additional source files.
- Update PR #19 body only through GitHub after local verification.

**Interfaces:**
- Consumes: a clean local head containing Tasks 1-3 and the approved spec
  commits.
- Produces: a matching remote PR head with successful local, CI, and review
  evidence.

- [ ] **Step 1: Run the complete local gate serially**

Run:

```bash
pnpm test
pnpm build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
git diff --check origin/main...HEAD
```

Expected: every command passes. Record the exact Vitest file/test counts, Vite
module count, Rust target counts, and HEAD.

- [ ] **Step 2: Replay only public signature evidence**

Run:

```bash
cargo run --locked --offline \
  --manifest-path src-tauri/Cargo.toml \
  --example verify_updater_signature -- \
  src-tauri/tauri.conf.json \
  docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt \
  docs/superpowers/reports/0.4.0-updater-backup-recovery/fixture.txt.sig
```

Expected: `signature_verified=true`. Do not invoke the real recovery script
and do not request any secret.

- [ ] **Step 3: Recheck repository and PR boundaries**

Fetch `origin`, then verify:

```bash
git status --short
git merge-base --is-ancestor origin/main HEAD
git rev-parse HEAD
git ls-remote origin refs/heads/codex/release-0.4.0
```

Expected: clean worktree, `origin/main` is an ancestor, and the local head is
the only intended unpublished head.

- [ ] **Step 4: Push the feature branch and select exact-head CI**

```bash
git push origin HEAD:codex/release-0.4.0
ci_head="$(git rev-parse HEAD)"
ci_runs="$(
  gh run list \
    --repo qwertyerge/codex-pulse \
    --workflow CI \
    --commit "$ci_head" \
    --json databaseId,headSha,status,conclusion,url \
    --limit 20
)"
ci_run_id="$(
  printf '%s' "$ci_runs" |
    jq -r --arg sha "$ci_head" \
      '[.[] | select(.headSha == $sha)] | max_by(.databaseId).databaseId'
)"
gh run watch "$ci_run_id" \
  --repo qwertyerge/codex-pulse \
  --exit-status
```

Select only a GitHub Actions run whose `headSha` exactly equals the pushed
commit. Wait until Frontend, Rust, Rust (Windows), Windows NSIS build, and
package verification are terminal successes. A run for any older SHA is not
evidence.

- [ ] **Step 5: Obtain two independent review gates**

Request a fresh read-only reviewer pass over `origin/main...HEAD`, with special
attention to:

- echo disabled before any hidden prompt;
- exact-state restoration after success, read failure, `HUP`, `INT`, and
  `TERM`;
- no secret values in argv, output, evidence, or test environment inherited by
  the child;
- unchanged recovery evidence and cryptographic boundaries.

Also require CodeRabbit to complete a substantive review of the exact pushed
head. Resolve every actionable thread; do not treat a status check alone as a
substantive review.

- [ ] **Step 6: Refresh PR body with exact facts**

Update PR #19's body with the new frontend test count, exact-head CI run URL,
exact reviewer result, unchanged-evidence statement, and remaining
non-goals. Preserve CodeRabbit-managed content. Re-read the body and verify
that it contains no secret, path, or unsupported completion claim.

### Task 5: Squash merge PR #19 and verify exact main

**Files:**
- No repository file changes.

**Interfaces:**
- Consumes: exact reviewed PR head and all-success required checks.
- Produces: one squash commit on `main`, plus exact-main CI evidence.

- [ ] **Step 1: Revalidate the merge transaction**

Read PR #19 immediately before merging. Require:

- state `OPEN`, Ready, and `mergeable=MERGEABLE`;
- merge state `CLEAN`;
- remote head exactly equals the reviewed SHA;
- unresolved review-thread count `0`;
- every required check successful;
- no `0.4.0` tag or Release exists.

Stop through AskHuman if any value differs or any new review finding appears.

- [ ] **Step 2: Squash merge with head protection**

Run:

```bash
reviewed_head="$(
  gh pr view 19 \
    --repo qwertyerge/codex-pulse \
    --json headRefOid \
    --jq .headRefOid
)"
test "$reviewed_head" = "$(git rev-parse HEAD)"
gh pr merge 19 \
  --repo qwertyerge/codex-pulse \
  --squash \
  --match-head-commit "$reviewed_head" \
  --subject "chore: prepare 0.4.0 updater bootstrap" \
  --body "Prepare the 0.4.0 updater bootstrap with verified recovery tooling, immutable public evidence, and guarded TTY secret input."
```

Do not add `--delete-branch`; repository settings may delete the remote
branch, but the host-managed local worktree must remain untouched.

- [ ] **Step 3: Verify the merge object and exact-main CI**

Fetch `origin/main`, read PR #19's `mergeCommit`, and verify that
`origin/main` equals that squash SHA. Select only the new main-branch workflow
run whose `headSha` equals the squash SHA:

```bash
git fetch origin main
merge_sha="$(
  gh pr view 19 \
    --repo qwertyerge/codex-pulse \
    --json mergeCommit \
    --jq .mergeCommit.oid
)"
test "$(git rev-parse origin/main)" = "$merge_sha"
main_runs="$(
  gh run list \
    --repo qwertyerge/codex-pulse \
    --workflow CI \
    --branch main \
    --commit "$merge_sha" \
    --json databaseId,headSha,status,conclusion,url \
    --limit 20
)"
main_run_id="$(
  printf '%s' "$main_runs" |
    jq -r --arg sha "$merge_sha" \
      '[.[] | select(.headSha == $sha)] | max_by(.databaseId).databaseId'
)"
gh run watch "$main_run_id" \
  --repo qwertyerge/codex-pulse \
  --exit-status
```

Require all jobs in that exact-main run to finish successfully.

- [ ] **Step 4: Reconfirm untouched release boundaries**

Verify that no `0.4.0` tag or GitHub Release exists and that no install,
publication, GitHub Secret mutation, or real recovery drill occurred. Report
merge success and exact-main CI separately from those still-unstarted gates.
