# Footer Stability and Session Open Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the floating initialization row stable across hook refreshes and restrict Codex deeplinks to the explicit Open icon.

**Architecture:** `useFooterInitialization` owns one hidden/visible row lifecycle, two independent 2,000 ms timers, and timer epochs so stale callbacks cannot alter a newer event. `SessionCard` presents its primary content as inert layout and emits `open` solely from a dedicated ExternalLink button.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest, Vue Test Utils, existing Lucide Vue icon.

## Global Constraints

- The initial-screen initialization feed must not render in the floating footer.
- The hidden-state quiet period is exactly 2,000 ms.
- The terminal-state leave delay is exactly 2,000 ms.
- A mounted footer row must update in place; an update must not cause a second enter/leave transition.
- A non-terminal row remains visible until a terminal snapshot arrives.
- Do not add Radar, token statistics, chart dependencies, persistent statistics, or backend data collection.
- The 15 px ExternalLink/Open button is the only `SessionCard` source of `open(threadId)`.

---

### Task 1: Stabilize the footer status lifecycle

**Files:**
- Modify: `src/composables/useFooterInitialization.ts`
- Modify: `src/__tests__/useFooterInitialization.spec.ts`

**Interfaces:**
- Consumes: `InitializationSnapshot` with `runId`, `phase`, and `events` from `src/types.ts`.
- Produces: `useFooterInitialization(source)` with unchanged `{ visible, initialization, stop }` return shape.

- [ ] **Step 1: Write the failing lifecycle tests**

Add a 2s timing assertion and an in-place update case to `src/__tests__/useFooterInitialization.spec.ts`:

```ts
expect(FOOTER_STATUS_QUIET_MS).toBe(2_000);
expect(FOOTER_STATUS_LEAVE_MS).toBe(2_000);

initialization.value = snapshot(2, "complete");
await nextTick();
await vi.advanceTimersByTimeAsync(FOOTER_STATUS_QUIET_MS);
expect(footer.visible.value).toBe(true);

initialization.value = snapshot(3, "starting");
await nextTick();
expect(footer.visible.value).toBe(true);
expect(footer.initialization.value?.runId).toBe(3);
await vi.advanceTimersByTimeAsync(FOOTER_STATUS_LEAVE_MS);
expect(footer.visible.value).toBe(true);

initialization.value = snapshot(3, "complete");
await nextTick();
await vi.advanceTimersByTimeAsync(FOOTER_STATUS_LEAVE_MS - 1);
expect(footer.visible.value).toBe(true);
await vi.advanceTimersByTimeAsync(1);
expect(footer.visible.value).toBe(false);
```

Retain the existing continuous-refresh test, but advance by `FOOTER_STATUS_QUIET_MS` so it proves only the newest pending snapshot mounts.

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
pnpm vitest run src/__tests__/useFooterInitialization.spec.ts
```

Expected: FAIL because the current module exports a 600 ms throttle and hides the row whenever a new run begins.

- [ ] **Step 3: Write the minimal state machine**

In `src/composables/useFooterInitialization.ts`, replace the constants and scheduling branches with this shape:

```ts
export const FOOTER_STATUS_QUIET_MS = 2_000;
export const FOOTER_STATUS_LEAVE_MS = 2_000;

function updateVisible(snapshot: InitializationSnapshot, terminal: boolean) {
  initialization.value = snapshot;
  clearLeaveTimer();
  if (terminal) scheduleLeave(snapshot.runId);
}

function scheduleHidden(snapshot: InitializationSnapshot) {
  clearQuietTimer();
  quietTimer = setTimeout(() => {
    initialization.value = snapshot;
    visible.value = true;
    if (isTerminal(snapshot)) scheduleLeave(snapshot.runId);
  }, FOOTER_STATUS_QUIET_MS);
}
```

For a hidden row, replace the pending snapshot and restart `quietTimer`. For a visible row, set `initialization` directly, clear any leave timer, and schedule one only for a terminal snapshot. Store an incrementing `timerEpoch` with each scheduled callback and ignore callbacks whose epoch is no longer current. Do not set `visible` false until the terminal leave callback succeeds.

- [ ] **Step 4: Run the focused tests and verify success**

Run:

```bash
pnpm vitest run src/__tests__/useFooterInitialization.spec.ts
```

Expected: PASS, including the in-place update case with no intermediate `visible === false`.

- [ ] **Step 5: Commit the completed footer lifecycle**

```bash
git add src/composables/useFooterInitialization.ts src/__tests__/useFooterInitialization.spec.ts
git commit -m "fix: stabilize footer refresh lifecycle"
```

### Task 2: Make Open the only session deeplink action

**Files:**
- Modify: `src/components/SessionCard.vue`
- Modify: `src/styles.css`
- Modify: `src/__tests__/SessionCard.spec.ts`

**Interfaces:**
- Consumes: `SessionSnapshot` and existing `open: [threadId: string]` emit.
- Produces: a non-interactive `.session-card__main` content region and a `button.session-card__open` that emits `open`.

- [ ] **Step 1: Write failing interaction tests**

Add a test that asserts the card primary region is not a button and emits no event when clicked, while the icon button emits once:

```ts
expect(wrapper.find(".session-card__main").element.tagName).not.toBe("BUTTON");
await wrapper.get(".session-card__main").trigger("click");
expect(wrapper.emitted("open")).toBeUndefined();

await wrapper.get(".session-card__open").trigger("click");
expect(wrapper.emitted("open")).toEqual([["00000000-0000-4000-8000-000000000001"]]);
expect(wrapper.get(".session-card__open").attributes("aria-label")).toContain("Open Codex task");
```

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
pnpm vitest run src/__tests__/SessionCard.spec.ts
```

Expected: FAIL because `.session-card__main` is currently the Open button and there is no dedicated `.session-card__open` action.

- [ ] **Step 3: Split inert content from the Open action**

Replace the heading portion in `src/components/SessionCard.vue` with a static content region and a sibling action group:

```vue
<div class="session-card__main">
  <span class="session-card__heading">
    <span class="session-card__active-dot" aria-hidden="true" />
    <span class="session-card__title" :title="session.title">{{ session.title }}</span>
    <button class="session-card__open" type="button" :aria-label="t('session.open', { title: session.title })" @click="$emit('open', session.threadId)">
      <ExternalLink aria-hidden="true" />
    </button>
  </span>
  <!-- existing path and timers remain here -->
</div>
```

Update `src/styles.css` so `.session-card__main` retains 11 px padding but has no button chrome, cursor, or active transform; `.session-card__open` is a 15-by-15 px borderless inline grid with the old icon stroke styles and a visible keyboard focus outline. Keep the icon at the heading's right edge and do not change card dimensions when it receives hover/focus.

- [ ] **Step 4: Run the focused tests and verify success**

Run:

```bash
pnpm vitest run src/__tests__/SessionCard.spec.ts
```

Expected: PASS, including existing timer, tooltip, Markdown, and localization coverage.

- [ ] **Step 5: Commit the session action change**

```bash
git add src/components/SessionCard.vue src/styles.css src/__tests__/SessionCard.spec.ts
git commit -m "fix: limit session deeplinks to open action"
```

### Task 3: Verify the integrated desktop behavior

**Files:**
- No production-file changes expected.

**Interfaces:**
- Consumes: completed Tasks 1 and 2.
- Produces: evidence that the frontend build, Rust suite, and installed native app preserve the intended layout and interaction behavior.

- [ ] **Step 1: Run full automated verification**

Run:

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
pnpm test
pnpm build
```

Expected: all commands exit 0.

- [ ] **Step 2: Install and inspect the native window**

Build/reinstall using the repository's established debug-app command, then resize the window to both a short and a tall height. Verify visually that the footer row opens once, text updates without re-entry, a terminal row leaves after 2s of quiet, the card body does nothing on click, and the 15 px Open icon alone launches the existing Codex deeplink.

- [ ] **Step 3: Commit only if native validation required a production fix**

If inspection changes source files, run the affected focused test plus the full verification command above, then commit those files with a conventional `fix:` message. If no source changes are needed, do not create an empty commit.
