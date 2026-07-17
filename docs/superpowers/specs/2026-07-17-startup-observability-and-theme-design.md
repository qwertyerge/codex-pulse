# Startup observability, bounded quota scanning, and display controls

## Goal

Make Codex Pulse start visibly and predictably: reduce weekly-quota cold reads,
show the live initialization phases without persisting diagnostic data, and make
session details easier to read through safe Markdown, stable timestamps, Lucide
icons, a larger type scale, and a user-selected appearance mode.

## Bounded data and storage governance

`QuotaSourceCache` discovers at most 16 files, not 64. It still limits
discovery to today's and yesterday's session directories, orders files by
modification time, and reads at most the final 256 KiB of a newly selected file.
The cold-read upper bound is therefore roughly 4 MiB. Existing per-file byte
cursors handle append-only updates; candidates falling out of the 16-file set
are evicted from memory.

Initialization diagnostics are intentionally not a database feature. The
backend holds an in-memory `VecDeque<InitializationEvent>` capped at 64
records, clears it when a new initialization run begins, and drops it at process
exit. Each record contains a `run_id`, a monotonically increasing sequence, an
observed time, a phase, and a concise human-readable summary. No refresh path
writes the events to disk, so their disk footprint is always zero and cannot
grow with runtime. The only new persisted value is the user's theme selection;
it writes to the existing configuration file only after an explicit setting
change.

The active-session parser is bounded as well. It retains at most 64 compacted
lifecycle records per candidate transcript, plus only the winning Recent event
and newest user message. The compaction preserves the active turn's original
start time, so long-running transcripts cannot produce linearly growing process
memory while their timers remain accurate.

## Initialization event flow

The blocking reconciliation emits events for: start, active-candidate discovery,
quota-source discovery/read, session reconciliation, completion, and failures.
`AppSnapshot` carries both the current initialization state and the current
event ring. The frontend also listens for incremental initialization events.
This snapshot-plus-stream model means events emitted before the WebView listener
is registered are still visible.

During the first screen load, while no session cards are ready, the empty state
includes the last six events. New rows slide in from the bottom, the previous
rows translate upward, and overflow is clipped. Once cards become available,
the feed is removed rather than inserted ahead of or alongside the task list.
Rows use a continuous log treatment rather than independent card-like shapes:
they share one subtle left rule, a monospace prefix, and the active final line
reveals a looping `......` suffix.

Subsequent background refreshes use only the latest event, rendered in a
single-line status row directly above the weekly-quota footer. The status row
and quota footer are an absolute, window-level bottom stack and do not join the
scrolling task-list layout. The task-list scrollport itself ends above the
bottom, rather than merely adding trailing content padding, so its scrollbar
cannot extend through the footer's vertical area. The reserve follows the actual
footer state: 48px for quota alone and 72px while the one-line status is visible.
The final task card can still scroll clear of the floating stack without a large
permanent dead area. One strong (`blur(32px)`) translucent glass surface sits
beneath both the status row and quota content; neither child renders its own
competing panel. The footer keeps symmetric 12px side gutters, while the list's
native scrollbar is intentionally hidden and takes no layout space. A subtle
`END` separator is rendered after the final session card as the visible scroll
boundary. The transient status
row fades after terminal refresh phases; it is diagnostic only and has no
controls that mutate monitoring configuration.

## Detail content

Last prompt and Recent keep their compact one-line plain-text summaries while
collapsed. Their expanded panels use `marked` with raw-HTML rendering disabled,
then apply a DOMPurify tag/attribute allowlist before the result reaches
`v-html`. Transcript compaction normalizes line endings but retains Markdown
line structure, so lists, fenced blocks, and source links survive into the
expanded panel. Raw HTML, scripts, and event handlers are removed; ordinary
paragraphs, lists, inline code, code blocks, emphasis, and links remain
readable. Every rendered link is handed to a native command which accepts only
`http`, `https`, and `mailto` URLs and opens them through the operating system's
default handler. Image Markdown deliberately renders an inline icon-and-title
placeholder (never a remote `<img>` fetch) and uses the same native hand-off.
Markdown is rendered only for expanded content, so the list view does not
acquire parsing or layout cost from hidden details.

All custom control glyphs in scope are replaced by Lucide Vue components from
`@lucide/vue`:
expand/collapse disclosure, Pin/PinOff, and Sun/Moon/Monitor for theme mode.
Buttons retain semantic labels and tooltips, while the icon is decorative to
screen readers.

## Theme, type, and timestamp layout

The configuration gains `theme: "system" | "light" | "dark"`, defaulting to
`system`. The frontend applies the choice at the document root and persists it
through the existing command/config pathway. `system` follows
`prefers-color-scheme`; explicit choices override it immediately. The overall
base type scale increases one step while preserving hierarchy: titles remain
larger and stronger than supporting paths, timing, and event metadata.

Recent timestamps split into a measured value field and a fixed `ago` suffix.
At mount the component lays out every supported compact value (`1s`, `10s`,
`1m`, `10m`, `1h`, `10h`, `1d`, `10d`, `99d+`) inside the same `small` element
as the visible text, then reserves the largest true layout width. The suffix
therefore begins at the same horizontal position regardless of the changing age
string or proportional glyph widths. Expanding Recent freezes that displayed
age and places an outlined, centered Lucide pause indicator immediately after
`ago`; collapse resumes the live age.

## Error handling and verification

Failed event parsing or a failed scan emits one concise failure event and leaves
the existing in-memory snapshot intact. Sessions and quota live in a single
cached snapshot and are replaced together only after a complete successful
scan. A single failed candidate does not prevent other candidates from
completing. Event storage truncates from the oldest end at 64 records.

Tests cover the 16-file quota cap, event ordering/truncation and snapshot
replay, safe Markdown sanitization, preserved list structure, default-app link
hand-off, image placeholders, fixed age suffix positioning, theme
persistence/default behavior, and icon button labels.
Final verification builds and installs the Tauri bundle, then checks startup
feed animation, the non-scrolling background-refresh row and bottom safety
space, light/dark/system appearance, card expansion, and the fixed timestamp
suffix in the live application.
