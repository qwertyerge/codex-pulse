# Default weekly quota and free-height window design

## Goal

Fix two independent Codex Pulse regressions:

1. The footer must show the default Codex weekly quota and must not become
   pinned to a model-specific 100% observation.
2. The main window must retain its current width constraints while allowing
   unrestricted vertical resizing.

## Root cause

Codex JSONL `token_count` records can report multiple quota families. The
default product quota uses `rate_limits.limit_id == "codex"`, while model- or
tier-specific quotas use identifiers such as `codex_bengalfox` or `premium`.
The current parser discards that identity and the monitor selects the newest
weekly observation across every family. A newer model-specific value can
therefore replace the default quota with 0% or 100%.

The main Tauri window has two vertical limits. The builder initially applies a
`10_000` pixel maximum, then startup replaces it with the current display work
area minus a safety margin. The second constraint is the visible height cap.

## Quota parsing and selection

`src-tauri/src/codex/jsonl.rs` will accept a weekly quota only when
`rate_limits.limit_id` is exactly `codex`. It will then preserve the existing
selection of a `primary` or `secondary` bucket whose `window_minutes` is
`10080`.

Missing, model-specific, and tier-specific limit identifiers produce no
`WeeklyQuota` record. Actual local records from the current Codex format carry
an explicit identifier, so accepting an unidentified limit would reintroduce
an ambiguous quota family.

Filtering at the JSONL input boundary keeps both downstream paths consistent:

- the active-session `ScanCache`;
- the bounded, incremental `QuotaSourceCache`.

Neither cache needs new state or a new public contract. Expiration,
latest-observation selection, integer normalization, reset timestamps, the
one-minute refresh cadence, and the frontend `WeeklyQuota` shape remain
unchanged.

## Window constraints

`src-tauri/src/app.rs` will apply one explicit `WindowSizeConstraints` value:

- minimum width: 320 logical pixels;
- minimum height: 360 logical pixels;
- maximum width: 480 logical pixels;
- maximum height: none.

The builder-level maximum size and the monitor-work-area height calculation
will be removed. Initial size, resizability, decorations, transparency,
always-on-top state, liquid-glass behavior, close-to-hide behavior, and the
startup maximize request remain unchanged.

## Error handling

Malformed or unsupported quota records remain ignored, matching the existing
best-effort local transcript parser. A model-specific record is valid input but
not the product quota displayed by this footer, so it is intentionally ignored
rather than treated as an error.

Tauri window-constraint application remains fallible and propagates its error
from `create_main_window`, consistent with the current startup behavior.

## Tests and acceptance

Rust parser tests will prove that:

- `limit_id == "codex"` is accepted from either weekly bucket;
- `codex_bengalfox`, `premium`, and missing identifiers are ignored;
- non-weekly windows remain ignored.

Quota-source regression tests will cover a model-specific 100% observation
followed by a default Codex 64% observation, and a newer model-specific
observation that must not replace the latest default quota.

Window constraints will be constructed by a pure helper so a Rust unit test can
assert the unchanged minimum size, the 480 pixel maximum width, and the absent
maximum height without requiring a live desktop.

Verification will include the focused RED/GREEN tests, the full Rust and
frontend suites, the frontend production build, and Rust formatting. Desktop
acceptance will compare the footer against the newest local
`limit_id == "codex"` record and confirm that vertical resizing is no longer
bounded by an application-supplied maximum height.

## Scope

This change does not add a multi-quota UI, show the most recently used model's
quota, change the footer presentation, change refresh timing, remove the
maximum width, or alter any session/context/AskHuman feature work.
