# Codex Pulse Sleep-Resilient Clock Design

## Problem

Codex Pulse derives all card-relative time from `useMonotonicClock`. The
current implementation samples `Date.now()` once at application startup and
then advances that wall-clock anchor exclusively with `performance.now()`.
On macOS, the WebView monotonic clock did not account for the full time spent
asleep. After wake, the displayed clock remained hours behind the JSONL and
SQLite epoch timestamps.

The running application exposed the same clock drift through independent
cards. At approximately 09:21 on 2026-07-22, three cards produced the same
effective frontend time when their stored creation timestamp was added to the
displayed session age: 03:56:30. The active JSONL and SQLite timestamps were
current and correct. New run, session, and recent-event timestamps therefore
appeared to be in the future, and the existing duration formatters defensively
clamped all three negative ages to zero.

## Goals

- Include macOS sleep time in current-run, session-age, and recent-event ages.
- Preserve the existing guarantee that displayed time never moves backward.
- Recover on the first timer tick after wake without waiting for a snapshot
  refresh, focus event, or visibility change.
- Keep the repair isolated to the shared frontend clock.

## Non-Goals

- Change JSONL parsing, SQLite lookup, session reconciliation, or Rust models.
- Change the 60-second fallback snapshot refresh.
- Change recent-event coalescing or expanded-event freeze behavior.
- Add a new error state for future timestamps.

## Chosen Design

`useMonotonicClock` will use the wall clock on every tick and preserve
monotonicity by clamping the sampled value against the previously published
value:

```text
nextNowMs = max(previousNowMs, Date.now())
```

The clock remains initialized from `Date.now()`. Each update publishes the
clamped value and schedules the next update on the next wall-clock second
boundary. The one-time `performance.now()` anchor is removed.

This gives the three required behaviors:

1. Normal operation advances once per wall-clock second.
2. After sleep, the first resumed callback samples the advanced wall clock and
   immediately includes the complete sleep interval.
3. If the system clock is adjusted backward, the published clock holds its
   previous value until wall time catches up, so durations do not regress.

`SessionCard` and `FooterStatus` continue to consume the same `nowMs` ref. The
duration helpers continue clamping negative values to zero as a defensive
fallback for genuinely future source timestamps.

## Alternatives Considered

### Re-anchor on focus or visibility events

This retains the existing monotonic projection but depends on WebView lifecycle
events. A pinned background window may not receive a focus transition, and
platform differences make wake detection less reliable than sampling the clock
that owns the epoch timestamps.

### Re-anchor from backend snapshots

This uses an IPC response to correct frontend time. It would leave the UI wrong
until the next event-driven or 60-second fallback refresh and would couple a
display-only clock to session reconciliation.

## Testing

Add `src/__tests__/useMonotonicClock.spec.ts` with deterministic fake-timer
coverage:

- Simulate wall time advancing by several hours while `performance.now()` does
  not advance equivalently. The current implementation must fail because it
  remains near its startup anchor; the repaired clock must catch up on the next
  tick.
- Move wall time backward and verify that a later tick never publishes a value
  below the previously observed `nowMs`.

Existing `SessionCard` tests continue to prove that one `nowMs` input drives
current-run duration, session age, and recent-event age. No component change is
required.

Run the focused test in RED and GREEN states, then run:

- `pnpm test`
- `pnpm build`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `git diff --check`

## Runtime Acceptance

Build and install the updated macOS debug application, then restart Codex Pulse.
Inspect this active task twice several seconds apart and verify:

- current-run duration is greater than zero and advances;
- session age is greater than zero and advances;
- recent-event age advances when no newer event replaces it;
- older active-session timers continue advancing.

The automated sleep-gap test is the acceptance evidence for wake behavior; the
Mac does not need to be forced to sleep during verification.
