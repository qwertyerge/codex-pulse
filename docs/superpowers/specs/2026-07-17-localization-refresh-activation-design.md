# Localization, refresh cadence, footer motion, and Codex activation

## Goal

Make Codex Pulse usable in Simplified Chinese, English, French, and German;
reduce fallback scanning work; give the floating footer a bottom-anchored status
transition; and retain the existing Codex task deep-link hand-off.

## Locale model

`vue-i18n` is the single frontend translation runtime. The stored locale is the
closed set `system | zh-CN | en | fr | de`. `system` resolves the browser's
language at runtime: `zh*` resolves to `zh-CN`, `fr*` to `fr`, `de*` to `de`,
and all other values to `en`. The resolved locale is an implementation detail;
the persisted preference remains `system` when that option is selected.

The Rust configuration validates locale values before saving them. The snapshot
and frontend type use the same closed set. A frontend locale change is
optimistic, persists through one `set_locale` command, and rolls back both the
stored preference and active i18n locale if that command fails.

The header places a Lucide `Languages` control between appearance controls and
Pin/Unpin. It opens a small accessible menu with System, 中文, English,
Français, and Deutsch. Selection immediately changes the visible locale and
closes the menu. Static product UI copy, tooltips, labels, and known
initialization phase text are translated. Session titles, file paths, user
prompts, Recent content, and backend errors remain unmodified external text.

## Refresh cadence

The native fallback reconciliation remains an eventual-consistency fallback but
runs every 60 seconds rather than every 15 seconds. The WebView's snapshot poll
also runs every 60 seconds. Startup still performs an immediate load, and hook
notifications still schedule reconciliation and emit `sessions-changed`
immediately; lowering the fallback cadence therefore does not delay ordinary
active-session updates.

## Footer status transition

The quota footer and one-line background event remain one bottom-anchored glass
surface. When a background event appears, the surface stretches upward from its
fixed bottom edge from the quota-only reserve (48px) to the event reserve
(72px). The status content fades and translates into the newly available space;
leaving reverses the same transition. The end result preserves the separate
scrollport and its `END` boundary marker. `prefers-reduced-motion` disables the
stretch and translation while preserving the final states.

## Codex task hand-off

Task clicks retain the existing validated `codex://threads/<uuid>` hand-off.
Codex and the operating system already bring the corresponding application
window forward, so this change deliberately introduces no AppKit dependency,
bundle-id lookup, or second activation path. Non-macOS behavior remains
unchanged for the same reason.

## Error handling and verification

Unknown locale values fail before configuration changes. Locale persistence
errors restore the prior locale in the UI. Malformed thread IDs still fail
before any URL hand-off. The existing deep-link validation remains the only
native task-opening boundary.

Tests cover locale validation/persistence, system locale resolution, translated
menu labels and selection behavior, the 60-second native and frontend fallback
intervals, footer transition classes, and the retained deep-link boundary. Final
native verification checks all four explicit languages, System fallback,
footer expansion/collapse after a refresh, hook-driven refresh without waiting
for the fallback interval, and the existing task deep-link hand-off.
