# Project Link and Theme Hover Design

## Goal

Make each session card's project path easier to recognize and open, and make the light/dark/system theme controls retain clear hover and selected states in both resolved color schemes.

## Project Link

Replace the current plain project-path text with an anchor-style control. Its visible label is the final path component after removing trailing `/` or `\` separators. For example, `/workspace/codex-pulse` is displayed as `codex-pulse`. The anchor's `title` remains the exact full path. If a root path has no ordinary final component, the label falls back to the original path.

Clicking the project link opens that directory with the operating system's default application: Finder on macOS or Explorer on Windows. This interaction remains separate from the existing Open Codex Task icon, which continues to deep-link to the session and is not otherwise changed.

`SessionCard` emits `open-project` with the unmodified `cwd`. `App` forwards that event to `usePulse.openProjectPath`, which invokes a new `open_project_path` Tauri command. The command verifies that the value is a non-empty, existing directory before passing it to `tauri-plugin-opener` as a path.

The WebView does not receive a broad filesystem opener capability. The Rust command is the controlled boundary and returns validation or opener errors through the existing `pulse.error` state.

## Theme Control States

Keep the current three-button layout, dimensions, icons, focus ring, active scaling, and blue selected treatment.

The current defect is caused by cascade order: a generic top-bar hover rule has the same effective specificity as the theme selected rules and appears later, so hovering can replace the selected background. In the light scheme this can leave white selected text over an almost-white background; in the dark scheme it suppresses the blue selected surface with a weak dark hover surface.

Model the states explicitly:

- A selected theme button always uses white foreground text and the existing `#3478f6` background, including while hovered.
- Only theme buttons with `aria-pressed="false"` receive a hover background.
- The light scheme uses a clearly visible pale blue hover surface for unselected buttons.
- The dark scheme uses a clearly visible blue-gray hover surface for unselected buttons.
- Non-theme top-bar controls retain their existing hover behavior.
- The `system` preference continues to resolve through `useTheme`; it automatically uses the active light or dark scheme while retaining `aria-pressed="true"` on the system button.

## Error Handling

The backend rejects an empty path, a missing path, or a non-directory path. The frontend clears a stale error before invoking the command and records any returned failure in the existing page-level error state. No new toast or dialog system is introduced.

## Testing and Verification

Implementation follows red-green-refactor and adds regression coverage for:

- path-basename display for Unix and Windows separators, full-path `title`, and root-path fallback;
- project-link emission of the exact `cwd` without triggering the existing session-open action;
- App and `usePulse` wiring to `open_project_path`, including error propagation;
- Rust validation of valid directories, missing paths, and ordinary files;
- CSS contracts that limit theme hover styling to unselected controls and preserve the selected background in light and dark schemes.

Verification runs focused Vitest and Rust tests first, followed by the full `pnpm test`, `pnpm build`, and `cargo test --manifest-path src-tauri/Cargo.toml` gates. When the desktop environment is available, the final check also exercises light and dark hover states and opens a real directory through the packaged interaction.

## Alternatives Considered

Calling the opener plugin directly from the frontend would be shorter but requires a broader WebView path permission and bypasses the existing error boundary. Reusing `open_external_url` with a `file://` URL would conflict with its deliberate HTTP/HTTPS/mailto validation and introduce cross-platform path-encoding problems. A dedicated Rust command therefore provides the narrowest and most reusable boundary.

## Out of Scope

- Changing the Open Codex Task icon or session deep-link behavior.
- Renaming projects from repository metadata instead of the path basename.
- Redesigning the top bar, theme icons, or theme persistence.
- Adding notifications beyond the existing page-level error message.
