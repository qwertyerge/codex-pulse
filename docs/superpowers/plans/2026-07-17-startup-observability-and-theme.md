# Startup Observability and Display Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make startup observable without disk growth, bound weekly-quota cold reads to 16 files, and improve session-detail readability and control affordances.

**Architecture:** The Rust backend owns a fixed-capacity, process-local initialization event ring and includes a snapshot of it in every `AppSnapshot`; it also emits new events through Tauri for immediate frontend animation. Quota discovery retains its separate tail cache but limits candidates to 16. Vue renders the first-run feed only in the loading empty state; later refreshes use a single event row in the floating bottom footer stack, outside the scrolling task list. Expanded Markdown is safe, the appearance choice persists at the document root, and paired controls use Lucide.

**Tech Stack:** Rust 2021, Tauri 2, `VecDeque`, `serde`, Vue 3, TypeScript, Vitest, Vue Test Utils, `@lucide/vue`, `marked`, DOMPurify.

## Global Constraints

- Resolve Codex paths through the existing `CODEX_HOME`; never hard-code a user directory.
- Keep all session/weekly-quota parsing in `spawn_blocking`; `get_snapshot` remains memory-only.
- Set `QUOTA_SOURCE_FILE_LIMIT` to exactly `16`; retain the 256 KiB per-file cold tail bound.
- Initialization diagnostics remain in memory only: a `VecDeque` has capacity `120`, uses a run id plus monotonic sequence, is cleared at the start of every refresh run, and is never written to SQLite or another file.
- Persist only `theme: system | light | dark` in the existing config, and only after an explicit theme command.
- Expanded Last prompt and Recent content must be rendered by `marked` and sanitized by DOMPurify before `v-html`; collapsed content remains one-line text.
- Use Lucide Vue icons for disclosure, pin state, and theme controls, with accessible labels/tooltips.
- Measure `1s`, `10s`, `1m`, `10m`, `1h`, `10h`, `1d`, `10d`, and `99d+` in the same rendered `small` context as the visible age; reserve the maximum true layout width so the `ago` suffix has a fixed x-position.
- Keep the quota footer and later one-line refresh status as one strong glass bottom stack. End the task-list scrollport above it with a state-aware 48px/72px reserve, so cards never run beneath it without creating a large blank area; keep symmetric 12px footer gutters, hide the native scrollbar without reserving space, and use a subtle END strip at the list boundary.
- Respect `prefers-reduced-motion` for the initialization event animation.
- Do not create incremental commits. After verification, squash all current feature-branch changes since `main` into the single user-requested commit.

---

## File Structure

| Path | Responsibility |
| --- | --- |
| `package.json`, `pnpm-lock.yaml` | Declare the Markdown, sanitization, and Lucide dependencies. |
| `src-tauri/src/model.rs` | Serialize initialization phases/events/snapshot and the selected theme in `AppSnapshot`. |
| `src-tauri/src/config.rs` | Persist a validated `ThemeMode` field with the safe `system` default. |
| `src-tauri/src/initialization.rs` | Own the 120-event in-memory ring, sequencing, snapshots, and event names. |
| `src-tauri/src/commands.rs` | Record background phase events, expose them in snapshots, and persist a theme command. |
| `src-tauri/src/app.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs` | Register native commands, including validated default-app hand-off for rendered Markdown links. |
| `src-tauri/src/monitor.rs` | Cap quota discovery at 16 candidates and test the cap. |
| `src/types.ts`, `src/composables/usePulse.ts`, `src/composables/useTheme.ts` | Mirror contracts, merge streamed initialization events, and apply theme mode. |
| `src/components/InitializationFeed.vue`, `src/components/InitializationStatusRow.vue` | Render the first-run six-row feed and the later one-line floating footer status. |
| `src/components/MarkdownContent.vue` | Render sanitized expanded Markdown, preserve lists, hand off links through the native command, and replace images with safe inline placeholders. |
| `src/components/SessionCard.vue` | Use Markdown panels, Lucide disclosure/open icons, and the fixed-width age layout. |
| `src/components/TopBar.vue`, `src/App.vue` | Render icon-only pin/theme controls and connect the initialization stream. |
| `src/lib/duration.ts`, `src/lib/recentAgeWidth.ts` | Split age value/suffix and measure the largest compact age label. |
| `src/styles.css` | Add the larger type scale, explicit light/dark rules, Lucide control styles, Markdown typography, and reduced-motion-safe feed animation. |
| `src/__tests__/*.spec.ts` | Cover persistence, ring bounds/order, quota cap, Markdown safety, timestamp layout, event feed, and controls. |

## Task 1: Add dependencies and serializable domain/config contracts

**Files:**
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/config.rs`
- Test: `src/__tests__/usePulse.spec.ts`

**Interfaces:**
- Produces `ThemeMode::{System, Light, Dark}` serialized as `system | light | dark`.
- Produces `InitializationPhase::{Idle, Starting, DiscoveringCandidates, ReadingQuota, ReconcilingSessions, Complete, Failed}`.
- Produces `InitializationEvent { sequence: u64, occurred_at_ms: i64, phase: InitializationPhase, summary: String }` and `InitializationSnapshot { phase: InitializationPhase, events: Vec<InitializationEvent> }`.
- Extends `AppSnapshot` with `initialization` and `theme`.

- [ ] **Step 1: Add the frontend packages**

  Run:

  ```bash
  pnpm add @lucide/vue marked dompurify
  ```

  Expected: `package.json` records the three runtime packages and the lockfile is updated; no dependency is installed globally. DOMPurify ships its own TypeScript declarations.

- [ ] **Step 2: Write failing config and snapshot tests**

  Add this Rust test beside `persists_the_pin_state_across_loads`:

  ```rust
  #[test]
  fn persists_an_explicit_theme_choice() {
      let temp = tempfile::tempdir().unwrap();
      let store = ConfigStore::new(temp.path().join("config.json"));
      let config = AppConfig { theme: ThemeMode::Dark, ..AppConfig::default() };

      store.save(&config).unwrap();

      assert_eq!(store.load().unwrap().theme, ThemeMode::Dark);
      assert_eq!(AppConfig::default().theme, ThemeMode::System);
  }
  ```

  Extend the `usePulse` fixture with an `initialization` object and `theme`, then assert their default values are available after `load()`:

  ```ts
  expect(pulse.snapshot.value.theme).toBe("system");
  expect(pulse.snapshot.value.initialization.events).toEqual([]);
  ```

- [ ] **Step 3: Run the focused tests and verify they fail**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml config::tests::persists_an_explicit_theme_choice
  pnpm test -- src/__tests__/usePulse.spec.ts
  ```

  Expected: Rust fails because `ThemeMode`/`AppConfig.theme` are absent, and TypeScript fails because `AppSnapshot` has no initialization/theme contract.

- [ ] **Step 4: Add exact Rust contracts and defaults**

  Add to `model.rs`:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub enum ThemeMode { System, Light, Dark }

  impl Default for ThemeMode { fn default() -> Self { Self::System } }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub enum InitializationPhase {
      Idle, Starting, DiscoveringCandidates, ReadingQuota, ReconcilingSessions, Complete, Failed,
  }

  impl Default for InitializationPhase { fn default() -> Self { Self::Idle } }

  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct InitializationEvent {
      pub sequence: u64,
      pub occurred_at_ms: i64,
      pub phase: InitializationPhase,
      pub summary: String,
  }

  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct InitializationSnapshot {
      pub phase: InitializationPhase,
      pub events: Vec<InitializationEvent>,
  }
  ```

  Add `theme: ThemeMode` to `AppConfig` and its default, then add
  `initialization: InitializationSnapshot` and `theme: ThemeMode` to every
  `AppSnapshot` constructor. Do not change the existing `locale` behavior.

- [ ] **Step 5: Mirror the TypeScript contract**

  Add to `src/types.ts`:

  ```ts
  export type ThemeMode = "system" | "light" | "dark";
  export type InitializationPhase = "idle" | "starting" | "discoveringCandidates" | "readingQuota" | "reconcilingSessions" | "complete" | "failed";
  export interface InitializationEvent { sequence: number; occurredAtMs: number; phase: InitializationPhase; summary: string; }
  export interface InitializationSnapshot { phase: InitializationPhase; events: InitializationEvent[]; }
  ```

  Extend `AppSnapshot` and `emptySnapshot` with `initialization: { phase: "idle", events: [] }` and `theme: "system"`.

- [ ] **Step 6: Run the focused tests and typecheck**

  Run:

  ```bash
  cargo fmt --check --manifest-path src-tauri/Cargo.toml
  cargo test --manifest-path src-tauri/Cargo.toml config::tests
  pnpm test -- src/__tests__/usePulse.spec.ts
  pnpm build
  ```

  Expected: formatter, config tests, frontend fixture tests, and TypeScript build pass.

## Task 2: Implement the bounded in-memory initialization event ring

**Files:**
- Create: `src-tauri/src/initialization.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/initialization.rs`

**Interfaces:**
- Produces `INITIALIZATION_PROGRESS_EVENT: &str = "initialization-progress"`.
- Produces `InitializationFeed::begin`, `InitializationFeed::record`, and `InitializationFeed::snapshot`.
- `AppState` owns `initialization: Mutex<InitializationFeed>`.

- [ ] **Step 1: Write failing ring tests**

  Create `src-tauri/src/initialization.rs` with only this test module first:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::{InitializationFeed, INITIALIZATION_EVENT_CAPACITY};
      use crate::model::InitializationPhase;

      #[test]
      fn keeps_only_the_newest_events_in_sequence_order() {
          let mut feed = InitializationFeed::default();
          feed.begin(1);
          for index in 0..=INITIALIZATION_EVENT_CAPACITY {
              feed.record(2 + index as i64, InitializationPhase::ReadingQuota, format!("event {index}"));
          }

          let snapshot = feed.snapshot();
          assert_eq!(snapshot.events.len(), INITIALIZATION_EVENT_CAPACITY);
          assert_eq!(snapshot.events.first().unwrap().summary, "event 1");
          assert_eq!(snapshot.events.last().unwrap().summary, format!("event {INITIALIZATION_EVENT_CAPACITY}"));
          assert!(snapshot.events.windows(2).all(|pair| pair[0].sequence < pair[1].sequence));
      }

      #[test]
      fn begin_clears_prior_run_and_marks_starting() {
          let mut feed = InitializationFeed::default();
          feed.record(1, InitializationPhase::Failed, "old failure".into());
          feed.begin(2);

          let snapshot = feed.snapshot();
          assert_eq!(snapshot.phase, InitializationPhase::Starting);
          assert_eq!(snapshot.events.len(), 1);
          assert_eq!(snapshot.events[0].summary, "Starting Codex Pulse reconciliation");
      }
  }
  ```

- [ ] **Step 2: Run the tests and verify they fail**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml initialization::tests
  ```

  Expected: compile failure because the module and feed API do not exist.

- [ ] **Step 3: Implement the ring without any disk I/O**

  Implement the public boundary:

  ```rust
  pub const INITIALIZATION_PROGRESS_EVENT: &str = "initialization-progress";
  pub const INITIALIZATION_EVENT_CAPACITY: usize = 120;

  #[derive(Default)]
  pub struct InitializationFeed {
      phase: InitializationPhase,
      next_sequence: u64,
      events: VecDeque<InitializationEvent>,
  }

  impl InitializationFeed {
      pub fn begin(&mut self, now_ms: i64) -> InitializationEvent {
          self.phase = InitializationPhase::Starting;
          self.next_sequence = 0;
          self.events.clear();
          self.record(now_ms, InitializationPhase::Starting, "Starting Codex Pulse reconciliation".into())
      }
      pub fn record(&mut self, now_ms: i64, phase: InitializationPhase, summary: String) -> InitializationEvent {
          self.next_sequence += 1;
          self.phase = phase;
          let event = InitializationEvent { sequence: self.next_sequence, occurred_at_ms: now_ms, phase, summary };
          self.events.push_back(event.clone());
          while self.events.len() > INITIALIZATION_EVENT_CAPACITY { self.events.pop_front(); }
          event
      }
      pub fn snapshot(&self) -> InitializationSnapshot {
          InitializationSnapshot { phase: self.phase, events: self.events.iter().cloned().collect() }
      }
  }
  ```

  `begin` must reset `next_sequence` to zero, remove all old events, set the
  phase to `Starting`, and record exactly `Starting Codex Pulse reconciliation`.
  `record` increments before assigning a sequence, updates the phase, and
  removes the oldest event only after inserting the new one.

- [ ] **Step 4: Connect the feed to `AppState` and the cached snapshot**

  Add `initialization: Mutex<InitializationFeed>` to both `AppState`
  constructors. Add a private `cached_initialization()` clone helper. Include
  that value in `get_snapshot` and create an idle empty feed in
  `snapshot_for_home`.

  Add a helper used only by `schedule_refresh`:

  ```rust
  fn publish_initialization_event(
      app: &tauri::AppHandle,
      state: &AppState,
      now_ms: i64,
      phase: InitializationPhase,
      summary: impl Into<String>,
  ) {
      let Ok(mut feed) = state.initialization.lock() else { return; };
      let event = feed.record(now_ms, phase, summary.into());
      let _ = app.emit(INITIALIZATION_PROGRESS_EVENT, event);
  }
  ```

  At the accepted start of `schedule_refresh`, call `begin` and emit its
  starting event. In the blocking task emit exactly these phase summaries:

  ```text
  Discovering recent active-session candidates
  Reading bounded weekly quota observations
  Reconciling active Codex sessions
  Active session reconciliation complete
  Reconciliation failed: <error>
  ```

  Emit `Complete` only after cached sessions/quota are replaced. On an error,
  emit `Failed` and leave the last successful sessions/quota unchanged. Do not
  emit one event per JSONL line or candidate file.

- [ ] **Step 5: Run the focused tests**

  Run:

  ```bash
  cargo fmt --check --manifest-path src-tauri/Cargo.toml
  cargo test --manifest-path src-tauri/Cargo.toml initialization::tests commands::tests
  ```

  Expected: the 120-event cap, reset behavior, config-free snapshot, and
  existing command tests pass.

## Task 3: Tighten quota discovery and preserve its tail-only behavior

**Files:**
- Modify: `src-tauri/src/monitor.rs`
- Test: `src-tauri/src/monitor.rs`

**Interfaces:**
- `QUOTA_SOURCE_FILE_LIMIT` is exactly `16`.
- `recent_quota_candidate_paths(codex_home)` returns no more than 16 candidates.
- First inspection of a candidate continues to start at `length - 256 * 1024`.

- [ ] **Step 1: Write the failing cap test**

  Add this monitor test:

  ```rust
  #[test]
  fn quota_source_limits_daily_candidates_to_sixteen_files() {
      let temp = tempfile::tempdir().unwrap();
      let day = chrono::Local::now().format("%Y/%m/%d").to_string();
      let sessions = temp.path().join("sessions").join(day);
      fs::create_dir_all(&sessions).unwrap();
      for index in 0..17 {
          fs::write(sessions.join(format!("quota-{index}.jsonl")), "{}\n").unwrap();
      }

      assert_eq!(super::recent_quota_candidate_paths(temp.path()).len(), 16);
  }
  ```

- [ ] **Step 2: Run the test and verify it fails**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml monitor::tests::quota_source_limits_daily_candidates_to_sixteen_files
  ```

  Expected: the assertion reports `64` candidates under the current constant.

- [ ] **Step 3: Change the only capacity constant**

  Replace:

  ```rust
  const QUOTA_SOURCE_FILE_LIMIT: usize = 64;
  ```

  with:

  ```rust
  const QUOTA_SOURCE_FILE_LIMIT: usize = 16;
  ```

  Do not change `QUOTA_INITIAL_TAIL_BYTES`, daily-directory selection, append
  handling, or expired-observation filtering.

- [ ] **Step 4: Run all quota-source tests**

  Run:

  ```bash
  cargo fmt --check --manifest-path src-tauri/Cargo.toml
  cargo test --manifest-path src-tauri/Cargo.toml monitor::tests
  ```

  Expected: the cap test and existing tail/expiry tests pass.

## Task 4: Render streamed startup progress and apply appearance mode

**Files:**
- Create: `src/components/InitializationFeed.vue`
- Create: `src/composables/useTheme.ts`
- Modify: `src/types.ts`
- Modify: `src/composables/usePulse.ts`
- Modify: `src/components/EmptyState.vue`
- Modify: `src/components/TopBar.vue`
- Modify: `src/App.vue`
- Modify: `src-tauri/src/app.rs`
- Test: `src/__tests__/InitializationFeed.spec.ts`
- Test: `src/__tests__/usePulse.spec.ts`
- Test: `src/__tests__/App.spec.ts`

**Interfaces:**
- `InitializationFeed` consumes `InitializationSnapshot` and renders only the last six events.
- `usePulse()` produces `setTheme(theme: ThemeMode)` and `mergeInitializationEvent(event: InitializationEvent)`.
- The Rust command `set_theme(theme: ThemeMode, state: State<AppState>) -> Result<ThemeMode, String>` is registered with Tauri.
- `useTheme(theme: Ref<ThemeMode>)` updates `document.documentElement.dataset.theme` with the resolved `light` or `dark` mode.

- [ ] **Step 1: Write failing feed and theme tests**

  Create `InitializationFeed.spec.ts`:

  ```ts
  it("shows only the newest six startup events in sequence order", () => {
    const events = Array.from({ length: 7 }, (_, index) => ({
      sequence: index + 1, occurredAtMs: index, phase: "readingQuota" as const, summary: `event ${index + 1}`
    }));
    const wrapper = mount(InitializationFeed, { props: { initialization: { phase: "readingQuota", events } } });

    expect(wrapper.findAll(".initialization-feed__event")).toHaveLength(6);
    expect(wrapper.text()).not.toContain("event 1");
    expect(wrapper.text()).toContain("event 7");
  });
  ```

  Add a `usePulse` test that invokes `set_theme` and updates the local snapshot:

  ```ts
  invoke.mockResolvedValueOnce("dark");
  await pulse.setTheme("dark");
  expect(invoke).toHaveBeenLastCalledWith("set_theme", { theme: "dark" });
  expect(pulse.snapshot.value.theme).toBe("dark");
  ```

- [ ] **Step 2: Run the tests and verify they fail**

  Run:

  ```bash
  pnpm test -- src/__tests__/InitializationFeed.spec.ts src/__tests__/usePulse.spec.ts
  ```

  Expected: imports and the `setTheme` API fail because no feed/theme code exists.

- [ ] **Step 3: Implement snapshot-plus-stream merging**

  In `usePulse.ts`, add a pure merge function that de-duplicates by `sequence`,
  sorts ascending, and keeps the newest 120 events. `mergeInitializationEvent`
  must ignore an event whose sequence already exists. `setTheme` must perform
  optimistic local replacement, invoke `set_theme`, and restore the old snapshot
  plus an error message on failure, matching `togglePin` behavior.

  In `App.vue`, register a second listener:

  ```ts
  unlistenInitialization = await listen<InitializationEvent>("initialization-progress", (event) => {
    pulse.mergeInitializationEvent(event.payload);
  });
  ```

  Dispose it on unmount. Keep the existing two-second snapshot poll; the
  snapshot is the recovery path for early events and dropped UI events.

- [ ] **Step 4: Implement the transient feed**

  `InitializationFeed.vue` must use a `TransitionGroup` named
  `initialization-event`, expose `aria-live="polite"`, and derive rows with:

  ```ts
  const visibleEvents = computed(() => props.initialization.events.slice(-6));
  ```

  Each row renders phase text only as `data-phase` for styling and renders the
  human summary. It must have no button and no persistent store.

  Extend `EmptyState` with an optional `initialization` prop and render the feed
  beneath the loading copy only when `loading && initialization.events.length`.
  Pass the snapshot value from `App.vue`. Leave the empty/no-active copy alone
  after a completed run.

- [ ] **Step 5: Implement and wire theme controls**

  Replace the TopBar control props with:

  ```ts
  defineProps<{ activeCount: number; alwaysOnTop: boolean; theme: ThemeMode }>();
  defineEmits<{ "toggle-pin": []; "set-theme": [theme: ThemeMode] }>();
  ```

  Render `Sun`, `Moon`, and `Monitor` Lucide buttons inside a labelled theme
  group. Each button has `:aria-pressed="theme === mode"`, an exact title
  (`Use light appearance`, `Use dark appearance`, `Follow system appearance`),
  and emits the corresponding mode. Replace Pin text with `Pin`/`PinOff`, but
  keep the existing Pin/Unpin aria labels and titles.

  Create `useTheme.ts` with a `matchMedia("(prefers-color-scheme: dark)")`
  listener. Its `apply()` function resolves `system` to a concrete `light` or
  `dark` and sets `document.documentElement.dataset.theme`. It reapplies on a
  system change only if the selected mode remains `system`, and removes the
  listener in `stop()`.

  In `App.vue`, start/stop the composable around component lifecycle, pass
  `snapshot.theme` to TopBar, and call `pulse.setTheme` from `@set-theme`.
  Register `set_theme` in `app.rs`; implement its Rust command by cloning the
  config, assigning its `theme`, saving it, then replacing the mutex value.

- [ ] **Step 6: Run frontend and command tests**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml commands::tests
  pnpm test -- src/__tests__/InitializationFeed.spec.ts src/__tests__/usePulse.spec.ts src/__tests__/App.spec.ts
  pnpm build
  ```

  Expected: event rendering, snapshot recovery, optimistic theme rollback, and
  the compiled TopBar/App contracts pass.

## Task 5: Render safe Markdown, Lucide disclosure, stable ages, and larger typography

**Files:**
- Create: `src/components/MarkdownContent.vue`
- Create: `src/lib/recentAgeWidth.ts`
- Modify: `src/lib/duration.ts`
- Modify: `src/components/SessionCard.vue`
- Modify: `src/styles.css`
- Test: `src/__tests__/MarkdownContent.spec.ts`
- Test: `src/__tests__/SessionCard.spec.ts`
- Test: `src/__tests__/duration.spec.ts`

**Interfaces:**
- `MarkdownContent` consumes `source: string` and renders sanitized HTML in `.markdown-content`.
- `formatRecentAgeValue(milliseconds)` returns values such as `1s` and `99d+`, never including `ago`.
- `RECENT_AGE_WIDTH_SAMPLES` exports the nine required values; `measureRecentAgeWidth(element)` returns the largest measured pixel width.

- [ ] **Step 1: Write failing Markdown and age tests**

  Create `MarkdownContent.spec.ts`:

  ```ts
  it("renders Markdown but removes unsafe HTML", () => {
    const wrapper = mount(MarkdownContent, { props: { source: "**safe**\n\n<script>alert(1)</script>\n\n[link](https://example.com)" } });

    expect(wrapper.html()).toContain("<strong>safe</strong>");
    expect(wrapper.html()).toContain('href="https://example.com"');
    expect(wrapper.html()).not.toContain("<script");
  });
  ```

  Extend duration tests:

  ```ts
  expect(formatRecentAgeValue(18_000)).toBe("18s");
  expect(formatRecentAgeValue(8_640_000_000)).toBe("99d+");
  expect(RECENT_AGE_WIDTH_SAMPLES).toEqual(["1s", "10s", "1m", "10m", "1h", "10h", "1d", "10d", "99d+"]);
  ```

  Extend the expanded-card fixture with Markdown and assert that the expanded
  detail contains a `.markdown-content strong` element, while the collapsed row
  remains text-only and one-line.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run:

  ```bash
  pnpm test -- src/__tests__/MarkdownContent.spec.ts src/__tests__/duration.spec.ts src/__tests__/SessionCard.spec.ts
  ```

  Expected: missing Markdown component/value formatter/sample export causes failures.

- [ ] **Step 3: Implement safe Markdown once**

  Create `MarkdownContent.vue`:

  ```vue
  <script setup lang="ts">
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import { computed } from "vue";

  const props = defineProps<{ source: string }>();
  const html = computed(() => DOMPurify.sanitize(marked.parse(props.source, { async: false }) as string));
  </script>

  <template><div class="markdown-content" v-html="html" /></template>
  ```

  Do not call `v-html` from `SessionCard` directly. In the expanded prompt and
  event panels, replace the current `<p>` with `MarkdownContent`; preserve the
  current frozen expanded values so an open panel does not refresh underneath
  the user.

- [ ] **Step 4: Split the relative-age value and reserve measured width**

  Implement:

  ```ts
  export function formatRecentAgeValue(milliseconds: number): string {
    const seconds = Math.floor(Math.max(0, milliseconds) / 1_000);
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3_600) return `${Math.floor(seconds / 60)}m`;
    if (seconds < 86_400) return `${Math.floor(seconds / 3_600)}h`;
    const days = Math.floor(seconds / 86_400);
    return `${Math.min(99, days)}${days >= 99 ? "d+" : "d"}`;
  }
  export function formatRecentAge(milliseconds: number): string { return `${formatRecentAgeValue(milliseconds)} ago`; }
  ```

  In `recentAgeWidth.ts`, create an offscreen canvas, assign the computed font
  from the rendered `<small>` node, and return
  `Math.ceil(Math.max(...RECENT_AGE_WIDTH_SAMPLES.map((sample) => context.measureText(sample).width)))`.
  If a canvas context is unavailable, return `0`; CSS uses `min-width: 4ch` as
  the fallback. In `SessionCard`, set the measured pixel width on the age value
  span only when it is positive, render a sibling static `ago` span, and do not
  retain the old hidden `Recent · 99d+` sizing element.

- [ ] **Step 5: Replace glyphs and update typography/styles**

  Use `ChevronDown`/`ChevronUp` in the disclosure buttons and `ExternalLink`
  for the task opener. Keep all icons `aria-hidden="true"`; their enclosing
  buttons retain accessible text through labels/titles.

  Replace the media-query-only dark stylesheet with explicit
  `:root[data-theme="dark"]` selectors so `useTheme` controls all three modes.
  Increase the base text scale by one level: brand `16px`, title `15px`, path
  `12px`, card metadata `11px`, timer labels `10px`, timer values `16px`, and
  footer copy `11px`. Keep supporting labels visually smaller than titles.

  Add Markdown rules for paragraphs, lists, inline code, preformatted code,
  and links; cap expanded code blocks with horizontal scrolling. Add
  `.initialization-event-enter-*`/`leave-*` slide-and-fade rules and a
  `prefers-reduced-motion` override that removes the translation.

- [ ] **Step 6: Run all detailed-content tests and build**

  Run:

  ```bash
  pnpm test -- src/__tests__/MarkdownContent.spec.ts src/__tests__/SessionCard.spec.ts src/__tests__/duration.spec.ts
  pnpm build
  ```

  Expected: safe Markdown, frozen expanded content, static `ago` positioning,
  and TypeScript/Vue compilation all pass.

## Task 6: Full verification, installed-app inspection, and one final commit

**Files:**
- Modify if needed from verification: exact failing test/source file only
- Modify: `docs/superpowers/specs/2026-07-17-startup-observability-and-theme-design.md`
- Create: `docs/superpowers/plans/2026-07-17-startup-observability-and-theme.md`

- [ ] **Step 1: Run all automated verification**

  Run:

  ```bash
  cargo fmt --check --manifest-path src-tauri/Cargo.toml
  cargo test --manifest-path src-tauri/Cargo.toml
  pnpm test
  pnpm build
  pnpm tauri build --debug
  ```

  Expected: all Rust and Vitest suites pass; Tauri generates
  `src-tauri/target/debug/bundle/macos/Codex Pulse.app`.

- [ ] **Step 2: Reinstall the debug bundle and verify it is current**

  Run after quitting the existing local application:

  ```bash
  ditto "src-tauri/target/debug/bundle/macos/Codex Pulse.app" "/Applications/Codex Pulse.app"
  shasum -a 256 "src-tauri/target/debug/CodexPulse" "/Applications/Codex Pulse.app/Contents/MacOS/CodexPulse"
  open -n -a "/Applications/Codex Pulse.app"
  ```

  Expected: both hashes match before runtime inspection.

- [ ] **Step 3: Inspect the live application**

  Verify all of the following in the installed app:

  - During cold reconciliation, the six-row initialization feed receives slide-in rows and then disappears after cards arrive.
  - The quota footer remains responsive and its candidate discovery uses the 16-file limit.
  - Opening Last prompt and Recent displays formatted Markdown without raw HTML/script rendering.
  - The relative `ago` suffix stays fixed while the numeric value changes.
  - Pin/Unpin and all three theme choices use Lucide icons with usable tooltips.
  - Light, dark, and system each apply correctly; relaunch preserves the explicit light/dark selection.
  - The larger type scale still leaves card/scroll/footer layout intact at the minimum window width.

- [ ] **Step 4: Create the one requested commit**

  The user requested that all current feature-branch work, including the prior
  quota footer commits and this design/implementation, land as one commit. Do
  not use a hard reset. After all verification succeeds, stage the complete
  delta from `main` and create exactly one replacement commit:

  ```bash
  git reset --soft main
  git add docs/superpowers/specs/2026-07-17-weekly-quota-footer-design.md docs/superpowers/specs/2026-07-17-startup-observability-and-theme-design.md docs/superpowers/plans/2026-07-17-weekly-quota-footer.md docs/superpowers/plans/2026-07-17-startup-observability-and-theme.md package.json pnpm-lock.yaml src src-tauri
  git commit -m "feat: improve Codex Pulse observability and controls"
  git status --short
  ```

  Expected: one commit sits on `main` and `git status --short` has no output.
