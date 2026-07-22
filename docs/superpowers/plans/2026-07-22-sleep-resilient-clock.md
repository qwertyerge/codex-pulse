# Sleep-Resilient Clock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep every Codex Pulse relative-time display current across macOS sleep without ever moving displayed time backward.

**Architecture:** Replace the one-time `Date.now()` plus `performance.now()` projection in the shared Vue clock with a wall-clock sample on every tick, clamped against the last published value. `SessionCard` and `FooterStatus` keep consuming the same `nowMs` ref, so no Rust model, snapshot, parser, or component contract changes.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest 4 fake timers, Tauri 2, Rust.

## Global Constraints

- Do not change JSONL parsing, SQLite lookup, session reconciliation, Rust models, or Tauri commands.
- Do not change the 60-second fallback snapshot refresh or five-second recent-event coalescing.
- Do not add dependencies, persisted state, or a new UI error state.
- The first timer callback after wake must include the complete sleep interval.
- A backward wall-clock adjustment must never decrease published `nowMs`.
- Preserve the existing second-boundary scheduling and `stop()` cleanup contract.

---

### Task 1: Make the shared frontend clock sleep-resilient

**Files:**
- Create: `src/__tests__/useMonotonicClock.spec.ts`
- Modify: `src/composables/useMonotonicClock.ts`

**Interfaces:**
- Consumes: browser `Date.now()`, `setTimeout`, and `clearTimeout`; Vue `ref`.
- Produces: unchanged `useMonotonicClock(): { nowMs: Ref<number>; start: () => void; stop: () => void }` contract.

- [ ] **Step 1: Write the failing sleep-gap and non-regression tests**

Create `src/__tests__/useMonotonicClock.spec.ts`:

```typescript
import { afterEach, describe, expect, it, vi } from "vitest";
import { useMonotonicClock } from "../composables/useMonotonicClock";

describe("useMonotonicClock", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("catches up to wall time on the first tick after sleep", async () => {
    vi.useFakeTimers();
    const startedAt = Date.parse("2026-07-22T00:00:00.000Z");
    const wakeAt = startedAt + 6 * 60 * 60 * 1_000;
    vi.setSystemTime(startedAt);
    vi.spyOn(performance, "now").mockReturnValue(1_000);
    const clock = useMonotonicClock();

    clock.start();
    vi.setSystemTime(wakeAt);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(clock.nowMs.value).toBe(wakeAt + 1_000);
    clock.stop();
  });

  it("does not move backward when wall time is adjusted backward", async () => {
    vi.useFakeTimers();
    const startedAt = Date.parse("2026-07-22T06:00:00.000Z");
    vi.setSystemTime(startedAt);
    vi.spyOn(performance, "now").mockReturnValue(1_000);
    const clock = useMonotonicClock();

    clock.start();
    vi.setSystemTime(startedAt - 60 * 60 * 1_000);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(clock.nowMs.value).toBe(startedAt);
    clock.stop();
  });
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
pnpm test -- src/__tests__/useMonotonicClock.spec.ts
```

Expected: FAIL in `catches up to wall time on the first tick after sleep`; the current implementation remains near `startedAt` because the frozen `performance.now()` does not include the six-hour wall-clock jump.

- [ ] **Step 3: Replace the stale one-time projection with a monotonic wall-clock sample**

Change `src/composables/useMonotonicClock.ts` to:

```typescript
import { ref } from "vue";

export function useMonotonicClock() {
  const nowMs = ref(Date.now());
  let timer: ReturnType<typeof setTimeout> | undefined;

  const update = () => {
    nowMs.value = Math.max(nowMs.value, Date.now());
    const delay = 1_000 - (Math.floor(nowMs.value) % 1_000);
    timer = setTimeout(update, Math.max(1, delay));
  };

  return {
    nowMs,
    start: update,
    stop: () => {
      if (timer) clearTimeout(timer);
      timer = undefined;
    }
  };
}
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/useMonotonicClock.spec.ts
```

Expected: PASS, 2 tests.

- [ ] **Step 5: Run the complete frontend regression suite and build**

Run:

```bash
pnpm test
pnpm build
```

Expected: all Vitest files pass; `vue-tsc --noEmit` and the Vite production build complete without errors.

- [ ] **Step 6: Commit the focused repair**

Run:

```bash
git add src/__tests__/useMonotonicClock.spec.ts src/composables/useMonotonicClock.ts
git commit -m "fix: keep pulse clock current across sleep"
```

Expected: one commit containing only the new clock regression tests and the minimal composable change.

---

### Task 2: Verify the native application and the running-session behavior

**Files:**
- Verify: `src-tauri/src/**/*.rs`
- Build output: `src-tauri/target/debug/bundle/macos/Codex Pulse.app`
- Runtime target: `/Applications/Codex Pulse.app`

**Interfaces:**
- Consumes: the unchanged Tauri `get_snapshot` payload and the current local Codex JSONL/SQLite data.
- Produces: an updated installed debug application whose three relative-time displays recover from sleep and advance from the same clock.

- [ ] **Step 1: Run native and repository hygiene verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
git status --short
```

Expected: all Rust tests pass, `git diff --check` is silent, and the only repository state is the intended committed work.

- [ ] **Step 2: Build the debug macOS application bundle**

Run:

```bash
pnpm tauri build --debug
test -x "src-tauri/target/debug/bundle/macos/Codex Pulse.app/Contents/MacOS/CodexPulse"
```

Expected: the Tauri build succeeds and the debug application binary exists.

- [ ] **Step 3: Replace the installed application recoverably**

Use Computer Use to send `Command+Q` to Codex Pulse and verify that its process exits. Then run:

```bash
install_backup_dir="$(mktemp -d "${TMPDIR%/}/codex-pulse-backup.XXXXXX")"
mv "/Applications/Codex Pulse.app" "$install_backup_dir/Codex Pulse.app"
ditto "src-tauri/target/debug/bundle/macos/Codex Pulse.app" "/Applications/Codex Pulse.app"
test -x "/Applications/Codex Pulse.app/Contents/MacOS/CodexPulse"
printf 'Backup: %s\n' "$install_backup_dir/Codex Pulse.app"
```

Expected: the previous bundle remains recoverable in `install_backup_dir`, and the new installed executable exists. If installation or launch fails, move the new bundle aside and restore the backup before investigating.

- [ ] **Step 4: Verify the live task twice without producing an intervening Codex event**

Use Computer Use to open Codex Pulse and read the current task card. In one bounded tool operation, capture the accessibility state, wait five seconds, and capture it again so no assistant commentary is written between samples.

Expected for `修复 Codex Pulse 运行数据刷新`:

- `当前运行` is greater than `00:00` and increases by approximately five seconds;
- `会话时长` is greater than `00:00` and increases by approximately five seconds;
- `最近事件` age is greater than `0s` and increases when the displayed event is unchanged;
- at least one older card continues advancing by the same interval.

- [ ] **Step 5: Confirm the installed process and final repository state**

Run:

```bash
pgrep -fl "/Applications/Codex Pulse.app/Contents/MacOS/CodexPulse"
git status --short --branch
git log -3 --oneline --decorate
```

Expected: one installed Codex Pulse process is running; the worktree has no uncommitted source changes; the design and implementation commits are visible at `HEAD`.
