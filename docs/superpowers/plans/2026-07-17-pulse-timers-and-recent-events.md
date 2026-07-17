# Codex Pulse Timers and Recent Events Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a stable per-second timer, title-adjacent active count, and one throttled recent event per active session card.

**Architecture:** Extend transcript parsing and the session registry with displayable event records, then throttle snapshot event changes per root session in `AppState`. The Vue UI consumes the enriched snapshot, uses a monotonic clock composable, and renders the latest event below the timers.

**Tech Stack:** Rust, Tauri 2, serde, Vue 3, TypeScript, Vitest.

## Global Constraints

- macOS desktop app; no new network or persisted event-content storage.
- Show only one recent meaningful event per root session.
- Coalesce displayed events per session in a five-second window.
- Preserve existing root/descendant aggregation semantics.

---

### Task 1: Enrich active session snapshots with recent events

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/codex/jsonl.rs`
- Modify: `src-tauri/src/codex/discovery.rs`
- Modify: `src-tauri/src/registry.rs`
- Test: `src-tauri/src/codex/jsonl.rs`
- Test: `src-tauri/src/registry.rs`

- [ ] Write failing Rust tests for parsing a meaningful agent event and selecting the newest descendant event for a root snapshot.
- [ ] Run the focused tests and verify they fail because the event model is unavailable.
- [ ] Add a serializable `RecentEvent` model, parse only meaningful event message variants, and retain the newest record while aggregating root snapshots.
- [ ] Run the focused tests and then `cargo test`.

### Task 2: Coalesce event updates in application state

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/commands.rs`

- [ ] Write a failing test that accepts an immediate event replacement but keeps the currently displayed event for another update inside five seconds.
- [ ] Run it and verify the missing merge behavior fails.
- [ ] Add a per-thread event display cache and merge refresh results with a 5,000 ms window.
- [ ] Run `cargo test` and `cargo clippy -- -D warnings`.

### Task 3: Stabilize timer updates and render card events

**Files:**
- Modify: `src/types.ts`
- Create: `src/composables/useMonotonicClock.ts`
- Modify: `src/App.vue`
- Modify: `src/components/SessionCard.vue`
- Modify: `src/styles.css`
- Test: `src/__tests__/SessionCard.spec.ts`
- Test: `src/__tests__/App.spec.ts`

- [ ] Write failing frontend tests for one recent-event line and a monotonic timer input.
- [ ] Run the focused tests and verify they fail.
- [ ] Use a `performance.now()`-anchored clock that ticks at the next second boundary; add the recent-event line under timers and style it as secondary metadata.
- [ ] Run `pnpm test -- --run` and `pnpm build`.

### Task 4: Move the active count into the title brand

**Files:**
- Modify: `src/components/TopBar.vue`
- Modify: `src/styles.css`
- Test: `src/__tests__/TopBar.spec.ts`

- [ ] Write a failing top-bar test that finds the count inside the brand and not the controls.
- [ ] Run it and verify it fails.
- [ ] Move the count markup and add compact title-adjacent styling.
- [ ] Run frontend tests and build.

### Task 5: Package and runtime verify

**Files:**
- Output: `src-tauri/target/debug/bundle/dmg/Codex Pulse_0.1.0_aarch64.dmg`

- [ ] Run full Rust and frontend verification.
- [ ] Run `pnpm tauri build --debug`.
- [ ] Replace `/Applications/Codex Pulse.app` with the built application, reopen it, and verify the rendered card list using Computer Use.
