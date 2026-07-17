# Footer stability and session-open design

## Scope

Keep the floating footer status legible during hook-driven refreshes, and make
the session deeplink an explicit action. This change does not add Radar,
token statistics, chart dependencies, persistent statistics, or backend data
collection.

## Footer state model

The first-screen initialization remains in the empty-state feed and does not
render in the floating footer.

After first-screen initialization:

1. While the footer row is hidden, each incoming initialization snapshot
   replaces the pending snapshot and restarts a 2,000 ms global quiet period.
   The row mounts only when that period completes without a newer snapshot.
2. Once mounted, the row remains mounted for the active display cycle. Incoming
   snapshots update its contents in place, so neither an enter nor a leave
   transition is replayed.
3. A non-terminal snapshot remains visible. A `complete` or `failed` snapshot
   starts a 2,000 ms leave delay.
4. Any snapshot received during that leave delay updates the same row and
   restarts the leave delay. The row leaves once only after 2,000 ms without a
   newer snapshot.
5. Run/epoch guards prevent stale timer callbacks from hiding a newer snapshot.

The two duration constants are named for their distinct roles and both are
2,000 ms: the hidden-state quiet period and the terminal-state leave delay.

## Session open interaction

`SessionCard` keeps its heading, path tooltip, and timers as static content.
The card body is not a button and has no click or keyboard activation path to
the Codex deeplink. The existing 15 px ExternalLink icon becomes a dedicated
Open button at the heading's right edge. It retains the localized accessible
name and is the only source of the component's `open(threadId)` event.

## Verification

- Unit tests use fake timers to cover quiet-period replacement, non-terminal
  persistence, terminal leave timing, and a new snapshot during leave delay
  without an intermediate hidden state.
- SessionCard tests prove body clicks do not emit `open`, while the labelled
  Open icon button emits exactly once.
- Run frontend tests, Rust tests, production build, and native-window resize
  checks after reinstalling the debug application.
