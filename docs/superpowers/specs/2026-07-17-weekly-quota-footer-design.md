# Codex Pulse weekly quota footer design

## Goal

Add a persistent footer that shows the Codex weekly quota with an hourglass icon,
used and remaining percentages, a reset countdown, and a used-percentage progress
bar. The feature must not add foreground I/O or reparse unchanged transcript data.

## Data source and contract

Codex Pulse will derive the weekly quota solely from local Codex session JSONL
records under the already-resolved `CODEX_HOME`. It will inspect `event_msg`
`token_count` records and select the rate-limit bucket whose
`window_minutes == 10080`.

The bucket may be present as either `rate_limits.primary` or
`rate_limits.secondary`; the field name is not the contract. A valid bucket
provides `used_percent` and `resets_at` (seconds since the Unix epoch). The
backend converts it to this optional snapshot value:

```text
weeklyQuota {
  usedPercent: integer 0..100,
  remainingPercent: integer 0..100,
  resetsAtMs: integer
}
```

`usedPercent` is rounded to the nearest whole percentage and clamped to
`0..100`; `remainingPercent` is `100 - usedPercent`. If no valid 10080-minute
bucket is found in the current scan, `weeklyQuota` is absent. The application
must not retain an older value and present it as current.

No Codex file is written, no fixed user directory is used, and no external
quota API is called.

## Incremental scan design

The existing background reconciliation remains the only I/O path. It continues
to be triggered by hooks and the 15-second fallback timer, with the existing
single-flight guard. `get_snapshot` continues to return cached memory only.

`ScanCache` is owned by the background scan state and has one entry per active
candidate transcript. Each entry stores:

- the file identity, modification time, and byte length;
- `processed_line_count` for the number of complete lines already applied;
- `byte_offset` immediately after the last complete line;
- the accumulated parsed transcript state, including its latest weekly quota
  observation.

On first observation, a transcript is read from the start. When the same file
is append-only and larger, the reader seeks to `byte_offset` and parses only
new complete lines. It updates the line count and offset after each newline.
A partial JSON tail does not advance either cursor, so the next scan retries it.

If the file identity changes, its length shrinks, or it is otherwise changed
without an append-only growth relationship, that entry is discarded and
reconstructed from byte zero. Entries whose paths leave the current candidate
set are evicted. Therefore a normal refresh performs only inexpensive metadata
checks for at most the existing 32 SQLite-selected candidates; unchanged files
read and parse zero lines, and appended files read only their delta.

The session registry is rebuilt from the cached accumulated transcripts. The
latest valid weekly quota across those transcripts, ordered by its record
timestamp, is included in the same `AppSnapshot` as the active sessions. This
adds no second directory walk, no second JSONL pass, no new SQLite query, and
no WebView-thread work.

## UI design

`FooterStatus` is rendered after the session scroll region in `App.vue`. It is
a non-shrinking footer, so session cards remain in the sole scroll container and
the footer is always visible.

When quota is available it contains:

- an inline, decorative hourglass SVG;
- `周额度` and the text `已用 NN% · 剩余 MM%`;
- a compact live countdown such as `2d 4h 后重置`;
- a semantic progress bar where the filled amount equals `usedPercent`.

The countdown uses the existing monotonic frontend clock. It does not invoke
Rust between snapshot updates. The bar exposes `role="progressbar"` with
`aria-valuemin="0"`, `aria-valuemax="100"`, and `aria-valuenow=usedPercent`.

When quota is unavailable, the footer stays visible but shows an hourglass and
`周额度 · 暂不可用`; no percentage, countdown, or fabricated progress is shown.

## Error handling

Malformed, incomplete, or unsupported JSONL records are ignored without
failing the scan. A malformed trailing line remains eligible for retry only if
it has no newline; a complete malformed line is skipped and its cursor advances.
File read errors affect only that transcript; other cached transcripts can still
produce sessions and quota. If all valid quota observations disappear from the
current candidate set, the footer transitions to the unavailable state.

## Verification

Backend tests cover both `primary` and `secondary` weekly buckets, rejection of
non-weekly buckets, newest-observation selection, integer clamping, initial
scan, append-only delta scans, partial-tail retry, and cache reset after
truncation or replacement.

Frontend tests cover available and unavailable states, percentage text,
countdown formatting, progress-bar ARIA values, and footer placement outside the
session scroll container. Final verification builds the Tauri application,
installs the generated bundle, opens it, and checks the live footer and list
scroll behavior.
