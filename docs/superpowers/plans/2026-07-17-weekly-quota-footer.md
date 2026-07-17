# Weekly Quota Footer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the locally observed Codex weekly quota in a persistent footer without adding foreground I/O or reparsing historical transcript lines.

**Architecture:** `token_count` records become optional `WeeklyQuota` observations while a per-file `ScanCache` retains an accumulated parsed transcript and a line/byte cursor. The existing background refresh derives sessions and the newest quota from that cache, and Vue renders the resulting optional snapshot in a non-scrolling hourglass footer.

**Tech Stack:** Rust 2021, Tauri 2, `serde_json`, Vue 3, TypeScript, Vitest, Vue Test Utils.

## Global Constraints

- Resolve Codex data through the existing `CODEX_HOME` path; never hard-code a user home path.
- Treat the rate-limit slot as data: select `primary` or `secondary` only when `window_minutes == 10080`.
- Run all filesystem and JSON parsing in the existing background blocking task; `get_snapshot` must read cached memory only.
- Cache per-file `processed_line_count` and `byte_offset`; unchanged files parse zero lines and append-only files parse only complete appended lines.
- Rebuild a cache entry after replacement, truncation, or non-append modification; do not present unavailable or stale data as current quota.
- Keep the footer outside `.session-list`; it must remain visible while cards scroll.
- Use Chinese quota copy: `周额度`, `已用`, `剩余`, `后重置`, and `暂不可用`.

---

## File Structure

| Path | Responsibility |
| --- | --- |
| `src-tauri/src/model.rs` | Serializable `WeeklyQuota` and the optional `AppSnapshot.weekly_quota` contract. |
| `src-tauri/src/codex/jsonl.rs` | Parse `token_count` weekly rate-limit records from either rate-limit slot. |
| `src-tauri/src/codex/discovery.rs` | Maintain incremental per-transcript line/byte cursors and accumulated parsed transcript data. |
| `src-tauri/src/monitor.rs` | Select candidates, update the cache, and return sessions plus the newest quota observation. |
| `src-tauri/src/commands.rs` | Own the scan cache and the cached weekly quota exposed to Tauri commands. |
| `src/types.ts` | Mirror the serialized quota contract for Vue. |
| `src/lib/duration.ts` | Format the reset countdown. |
| `src/components/FooterStatus.vue` | Render quota, hourglass, progress semantics, and unavailable state. |
| `src/App.vue` | Place the footer after the scrolling/empty content. |
| `src/styles.css` | Footer, progress, dark-mode, and non-scrolling layout rules. |
| `src/__tests__/FooterStatus.spec.ts` | Footer available/unavailable and accessibility tests. |
| `src/__tests__/duration.spec.ts` | Reset-countdown formatter tests. |

## Task 1: Add a weekly-quota domain contract and JSONL parser

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/codex/jsonl.rs`
- Test: `src-tauri/src/codex/jsonl.rs`

**Interfaces:**
- Produces `WeeklyQuota { used_percent: u8, remaining_percent: u8, resets_at_ms: i64, observed_at_ms: i64 }`, where `observed_at_ms` is skipped during snapshot serialization.
- Produces `ParsedRecord::WeeklyQuota(WeeklyQuota)` for a valid `event_msg/token_count` line.
- Consumes JSON fields `payload.rate_limits.primary` and `payload.rate_limits.secondary`.

- [ ] **Step 1: Write failing parser tests for current and legacy slots**

  Add these focused tests to the `jsonl.rs` test module. Keep the JSON on one line so it exercises the real line parser.

  ```rust
  #[test]
  fn parses_weekly_quota_from_primary_or_secondary_window() {
      let primary = r#"{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":81.0,"window_minutes":10080,"resets_at":1784870653},"secondary":null}}}"#;
      let secondary = r#"{"timestamp":"2026-07-17T12:01:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":12.0,"window_minutes":300,"resets_at":1784800000},"secondary":{"used_percent":22.0,"window_minutes":10080,"resets_at":1784871000}}}}"#;

      for (line, used, remaining) in [(primary, 81, 19), (secondary, 22, 78)] {
          let Some(ParsedRecord::WeeklyQuota(quota)) = parse_line(line).unwrap() else {
              panic!("expected weekly quota");
          };
          assert_eq!(quota.used_percent, used);
          assert_eq!(quota.remaining_percent, remaining);
      }
  }

  #[test]
  fn ignores_rate_limit_buckets_that_are_not_weekly() {
      let line = r#"{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":81.0,"window_minutes":300,"resets_at":1784870653}}}}"#;
      assert!(matches!(parse_line(line).unwrap(), Some(ParsedRecord::Ignored)));
  }
  ```

- [ ] **Step 2: Run the new tests and verify they fail**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml codex::jsonl::tests::parses_weekly_quota_from_primary_or_secondary_window
  ```

  Expected: compilation fails because `ParsedRecord::WeeklyQuota` and `WeeklyQuota` do not exist.

- [ ] **Step 3: Define the serialized quota type and parse it**

  Add this model next to `RecentEvent` in `model.rs`:

  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct WeeklyQuota {
      pub used_percent: u8,
      pub remaining_percent: u8,
      pub resets_at_ms: i64,
      #[serde(skip)]
      pub observed_at_ms: i64,
  }
  ```

  Extend `ParsedRecord` and `parse_event` so `token_count` delegates to a
  helper that searches `[primary, secondary]` for an object with
  `window_minutes == 10_080`. Require numeric `used_percent` and integer
  `resets_at`, round and clamp the former, multiply the latter by `1_000`, and
  return `ParsedRecord::WeeklyQuota`. Return `Ignored` when no qualifying
  object exists. The helper must set `observed_at_ms` from the enclosing record
  timestamp rather than from filesystem metadata.

  ```rust
  fn parse_weekly_quota(rate_limits: &Value, observed_at_ms: i64) -> Option<WeeklyQuota> {
      let bucket = ["primary", "secondary"].into_iter().find_map(|slot| {
          rate_limits.get(slot).filter(|bucket| {
              bucket.get("window_minutes").and_then(Value::as_i64) == Some(10_080)
          })
      })?;
      let used_percent = bucket.get("used_percent")?.as_f64()?.round().clamp(0.0, 100.0) as u8;
      let resets_at_ms = bucket.get("resets_at")?.as_i64()?.saturating_mul(1_000);
      Some(WeeklyQuota {
          used_percent,
          remaining_percent: 100 - used_percent,
          resets_at_ms,
          observed_at_ms,
      })
  }
  ```

- [ ] **Step 4: Run parser tests and the Rust formatter**

  Run:

  ```bash
  cargo fmt --check --manifest-path src-tauri/Cargo.toml
  cargo test --manifest-path src-tauri/Cargo.toml codex::jsonl::tests
  ```

  Expected: formatter and all JSONL tests pass.

- [ ] **Step 5: Commit the parser contract**

  ```bash
  git add src-tauri/src/model.rs src-tauri/src/codex/jsonl.rs
  git commit -m "feat: parse weekly Codex quota"
  ```

## Task 2: Make transcript parsing incremental at line and byte granularity

**Files:**
- Modify: `src-tauri/src/codex/discovery.rs`
- Modify: `src-tauri/src/monitor.rs`
- Test: `src-tauri/src/codex/discovery.rs`
- Test: `src-tauri/src/monitor.rs`

**Interfaces:**
- Produces `ScanCache::refresh(&mut self, paths: &[PathBuf]) -> Vec<ParsedTranscript>`.
- Each cached transcript retains `processed_line_count`, `byte_offset`, file identity, modification time, byte length, parsed events, recent events, user messages, and `weekly_quota`.
- `ParsedTranscript` exposes `weekly_quota: Option<WeeklyQuota>` and retains the newest observation by `observed_at_ms`.
- `scan_active_sessions` becomes `scan_active_sessions(codex_home, now_ms, &mut ScanCache) -> Result<ScanResult>`.

- [ ] **Step 1: Write failing cache tests for append, partial tail, and truncation**

  Add a discovery test that writes metadata and one complete event, refreshes a
  fresh cache, appends one complete `token_count` line plus a partial line, and
  refreshes twice. Assert that the first append increases the processed line
  count only for the complete line, the quota is visible, and the second
  refresh consumes the completed tail exactly once. Add a separate test that
  rewrites the file to a shorter valid transcript and asserts the cache no
  longer retains the old quota or event vectors.

  ```rust
  assert_eq!(cache.cursor_for(&path).unwrap().0, 3);
  assert_eq!(cache.refresh(&[path.clone()])[0].weekly_quota.as_ref().unwrap().used_percent, 81);
  assert_eq!(cache.cursor_for(&path).unwrap().0, 4);
  ```

- [ ] **Step 2: Run one append test and verify it fails**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml codex::discovery::tests::increments_from_the_last_complete_line
  ```

  Expected: compilation fails because `ScanCache` and the cursor accessor do not exist.

- [ ] **Step 3: Add an accumulator and a cursor-based reader**

  Refactor `read_transcript` around an internal accumulator that can apply one
  `ParsedRecord` without losing the first `session_meta`. Add `WeeklyQuota`
  handling that replaces `ParsedTranscript.weekly_quota` only when its
  `observed_at_ms` is newer. Preserve the current behavior for malformed full
  lines: skip them and continue.

  Add cache types equivalent to:

  ```rust
  #[derive(Default)]
  pub struct ScanCache {
      entries: HashMap<PathBuf, CachedTranscript>,
  }

  #[derive(Clone, PartialEq, Eq)]
  struct FileIdentity { dev: u64, ino: u64 }

  struct CachedTranscript {
      identity: FileIdentity,
      modified: SystemTime,
      length: u64,
      processed_line_count: u64,
      byte_offset: u64,
      parsed: ParsedTranscript,
  }

  #[cfg(test)]
  fn cursor_for(&self, path: &Path) -> Option<(u64, u64)> {
      self.entries.get(path).map(|entry| (entry.processed_line_count, entry.byte_offset))
  }
  ```

  On an append-only growth, use `File::seek(SeekFrom::Start(byte_offset))` and
  `BufRead::read_line`. Advance `processed_line_count` and `byte_offset` only
  for a line ending in `\n`; retain the starting offset when EOF yields an
  incomplete tail. If identity changes, length shrinks, or modification occurs
  without a growth relationship, replace the entry by replaying from byte zero.
  After `refresh`, retain entries only for the supplied candidate paths.

- [ ] **Step 4: Have the monitor derive sessions and quota from one cache pass**

  Introduce the monitor result and select its quota after cache refresh:

  ```rust
  pub struct ScanResult {
      pub sessions: Vec<SessionSnapshot>,
      pub weekly_quota: Option<WeeklyQuota>,
  }

  let transcripts = cache.refresh(&transcript_paths);
  let weekly_quota = transcripts
      .iter()
      .filter_map(|transcript| transcript.weekly_quota.as_ref())
      .max_by_key(|quota| quota.observed_at_ms)
      .cloned();
  ```

  Continue selecting at most `SQLITE_CANDIDATE_LIMIT` paths. Do not add a quota
  directory walk or a second JSONL read.

- [ ] **Step 5: Run cache and monitor tests**

  Run:

  ```bash
  cargo fmt --check --manifest-path src-tauri/Cargo.toml
  cargo test --manifest-path src-tauri/Cargo.toml codex::discovery::tests
  cargo test --manifest-path src-tauri/Cargo.toml monitor::tests
  ```

  Expected: all cache, partial-line, replacement, and monitor tests pass.

- [ ] **Step 6: Commit incremental scanning**

  ```bash
  git add src-tauri/src/codex/discovery.rs src-tauri/src/monitor.rs
  git commit -m "perf: incrementally scan Codex transcripts"
  ```

## Task 3: Publish cached quota through the Tauri snapshot

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`
- Modify: `src/types.ts`
- Modify: `src/composables/usePulse.ts`
- Test: `src/__tests__/usePulse.spec.ts`

**Interfaces:**
- `AppSnapshot.weekly_quota: Option<WeeklyQuota>` serializes as `weeklyQuota`.
- `AppState` owns `scan_cache: Mutex<ScanCache>` and `weekly_quota: RwLock<Option<WeeklyQuota>>`.
- The TypeScript `WeeklyQuota` interface has `usedPercent`, `remainingPercent`, and `resetsAtMs`.

- [ ] **Step 1: Write failing snapshot and frontend fixture tests**

  Extend the empty-home command test and the default frontend snapshot fixture
  to require `weeklyQuota: undefined`. Add a command test whose fixture scan
  contains a valid 10080-minute `token_count` line and assert the JSON-facing
  snapshot has `Some(WeeklyQuota { used_percent: 81, remaining_percent: 19, .. })`.

  ```rust
  assert!(snapshot.weekly_quota.is_none());
  ```

  ```ts
  expect(snapshot.weeklyQuota).toEqual({ usedPercent: 81, remainingPercent: 19, resetsAtMs: 1_784_870_653_000 });
  ```

- [ ] **Step 2: Run focused tests and verify failure**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml commands::tests::empty_codex_home_returns_an_empty_snapshot
  pnpm test -- src/__tests__/usePulse.spec.ts
  ```

  Expected: the Rust struct and TypeScript fixture are missing `weeklyQuota`.

- [ ] **Step 3: Cache both result fields during background refresh**

  Add `weekly_quota: Option<WeeklyQuota>` to `AppSnapshot`. Initialize a
  `ScanCache` and a quota lock in both `AppState::new` and the fallback
  constructor. In `schedule_refresh`, lock the scan cache inside
  `spawn_blocking`, call the new monitor signature once, coalesce only the
  returned sessions, then replace both cached sessions and cached quota from
  the same `ScanResult`. `get_snapshot` clones the quota lock into its response.
  `snapshot_for_home` creates a default cache for its one-shot test path.

  Mirror the serialised fields in `src/types.ts` and include
  `weeklyQuota: undefined` in `emptySnapshot`.

- [ ] **Step 4: Run snapshot contract tests**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml commands::tests
  pnpm test -- src/__tests__/usePulse.spec.ts
  ```

  Expected: the backend and frontend snapshot contracts pass with both defined and absent quota.

- [ ] **Step 5: Commit the snapshot contract**

  ```bash
  git add src-tauri/src/model.rs src-tauri/src/commands.rs src/types.ts src/composables/usePulse.ts src/__tests__/usePulse.spec.ts
  git commit -m "feat: expose weekly quota snapshot"
  ```

## Task 4: Render the fixed quota footer with a semantic progress bar

**Files:**
- Create: `src/components/FooterStatus.vue`
- Create: `src/__tests__/FooterStatus.spec.ts`
- Modify: `src/lib/duration.ts`
- Modify: `src/__tests__/duration.spec.ts`
- Modify: `src/App.vue`
- Modify: `src/__tests__/App.spec.ts`
- Modify: `src/styles.css`

**Interfaces:**
- `formatQuotaReset(milliseconds: number): string` returns compact `Xd Yh`, `Xh Ym`, or `Xm` values without a suffix.
- `FooterStatus` accepts `quota?: WeeklyQuota` and `nowMs: number`.
- Available markup includes `[role="progressbar"]` with `aria-valuenow=quota.usedPercent`; unavailable markup contains no progress bar.

- [ ] **Step 1: Write failing formatter and component tests**

  Add formatter examples and create a footer test suite:

  ```ts
  expect(formatQuotaReset(2 * 86_400_000 + 4 * 3_600_000)).toBe("2d 4h");
  expect(formatQuotaReset(2 * 3_600_000 + 9 * 60_000)).toBe("2h 9m");
  expect(formatQuotaReset(30_000)).toBe("0m");
  ```

  ```ts
  const wrapper = mount(FooterStatus, {
    props: { nowMs: 1_000_000, quota: { usedPercent: 81, remainingPercent: 19, resetsAtMs: 1_000_000 + 2 * 86_400_000 + 4 * 3_600_000 } }
  });
  expect(wrapper.text()).toContain("已用 81% · 剩余 19%");
  expect(wrapper.text()).toContain("2d 4h 后重置");
  expect(wrapper.get('[role="progressbar"]').attributes("aria-valuenow")).toBe("81");
  ```

  Add an unavailable assertion for `周额度 · 暂不可用` and no `[role="progressbar"]`.

- [ ] **Step 2: Run the focused frontend tests and verify failure**

  Run:

  ```bash
  pnpm test -- src/__tests__/duration.spec.ts src/__tests__/FooterStatus.spec.ts
  ```

  Expected: imports fail because `formatQuotaReset` and `FooterStatus` do not exist.

- [ ] **Step 3: Implement the formatter and component**

  Implement the formatter with clamped non-negative milliseconds and exact
  boundary handling:

  ```ts
  export function formatQuotaReset(milliseconds: number): string {
    const minutes = Math.floor(Math.max(0, milliseconds) / 60_000);
    const days = Math.floor(minutes / 1_440);
    const hours = Math.floor((minutes % 1_440) / 60);
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    return `${minutes}m`;
  }
  ```

  In `FooterStatus.vue`, compute a reset label only when `quota` is supplied,
  draw an `aria-hidden="true"` inline hourglass SVG, and bind the progress bar
  width to `quota.usedPercent + "%"`. Do not set a live region: a ticking
  countdown must not repeatedly announce itself to screen readers.

- [ ] **Step 4: Integrate the footer without expanding the scroll area**

  Import `FooterStatus` in `App.vue` and render it after the
  `TransitionGroup`/`EmptyState` branch:

  ```vue
  <FooterStatus :quota="pulse.snapshot.value.weeklyQuota" :now-ms="clock.nowMs.value" />
  ```

  Add `.quota-footer { flex: 0 0 auto; margin-top: 8px; }` and its dark-mode
  companion rules. Keep `.session-list { flex: 1 1 auto; min-height: 0; }` as
  the only vertical scroll container. Style the filled bar to be visibly
  distinct and clamp it with `width: min(100%, var(--quota-used));`.

  Extend the app test to assert `.quota-footer` exists and is not contained by
  `.session-list`.

- [ ] **Step 5: Run all frontend tests and build**

  Run:

  ```bash
  pnpm test
  pnpm build
  ```

  Expected: all Vitest suites pass and `vue-tsc --noEmit` completes before Vite builds `dist`.

- [ ] **Step 6: Commit the footer**

  ```bash
  git add src/components/FooterStatus.vue src/__tests__/FooterStatus.spec.ts src/lib/duration.ts src/__tests__/duration.spec.ts src/App.vue src/__tests__/App.spec.ts src/styles.css
  git commit -m "feat: show weekly quota footer"
  ```

## Task 5: Run full verification and inspect the installed application

**Files:**
- Modify only if a test exposes a defect in the files listed above.

**Interfaces:**
- Consumes the complete `AppSnapshot.weeklyQuota` contract and `FooterStatus` rendering.
- Produces a debug bundle whose installed binary is byte-identical to the verified bundle.

- [ ] **Step 1: Run the full automated suite**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml
  pnpm test
  pnpm build
  pnpm tauri build --debug
  ```

  Expected: Rust tests, Vitest, TypeScript build, and Tauri debug build all exit zero.

- [ ] **Step 2: Install and launch the verified debug bundle**

  Quit the existing application, copy the generated bundle into
  `/Applications/Codex Pulse.app`, relaunch it, and compare the SHA-256 values
  of both `Contents/MacOS/CodexPulse` executables. They must be identical
  before visual verification.

  ```bash
  ditto 'src-tauri/target/debug/bundle/macos/Codex Pulse.app/' '/Applications/Codex Pulse.app/'
  shasum -a 256 '/Applications/Codex Pulse.app/Contents/MacOS/CodexPulse' 'src-tauri/target/debug/bundle/macos/Codex Pulse.app/Contents/MacOS/CodexPulse'
  open -n '/Applications/Codex Pulse.app'
  ```

- [ ] **Step 3: Inspect the live window**

  Verify the bottom footer stays visible while the session cards scroll, the
  hourglass is visible, the progress bar is filled to the used percentage, the
  text reports used/remaining/reset countdown, and a missing local quota record
  switches the footer to `周额度 · 暂不可用`. Confirm the installed app is the
  just-built bundle before reporting the result.
