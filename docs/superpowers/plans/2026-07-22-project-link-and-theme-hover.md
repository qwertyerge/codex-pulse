# Project Link and Theme Hover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display each session's project directory as a basename link that opens in Finder/Explorer, while keeping theme selected and hover states legible in resolved light and dark modes.

**Architecture:** A pure frontend helper derives the display label, while `SessionCard` emits the original path through `App` to `usePulse`. A narrow Rust command validates that the path is an existing directory before handing it to `tauri-plugin-opener`; CSS uses explicit `aria-pressed` selectors so theme hover can never replace the selected surface.

**Tech Stack:** Vue 3 Composition API, TypeScript, Vitest 4, CSS, Rust, Tauri 2, `tauri-plugin-opener`.

## Global Constraints

- Display the path basename, with the exact full path in `title`; support both `/` and `\` separators and fall back to the original root path.
- Open the unmodified `cwd` with the operating system default directory application.
- Keep the existing Open Codex Task icon and `open_thread` flow unchanged.
- Do not grant the WebView a broad filesystem opener capability.
- Reject empty, missing, and non-directory paths in Rust and surface failures through the existing `pulse.error` state.
- Keep the current theme-group layout, icons, dimensions, focus ring, active scaling, blue `#3478f6` selected background, and theme persistence.
- A selected theme button remains white-on-blue while hovered; only `aria-pressed="false"` theme buttons receive light/dark hover surfaces.
- Add no dependencies, toast system, dialog, repository-metadata lookup, or unrelated top-bar redesign.

---

### Task 1: Render and emit the project link

**Files:**
- Create: `src/lib/projectName.ts`
- Create: `src/__tests__/projectName.spec.ts`
- Modify: `src/components/SessionCard.vue`
- Modify: `src/__tests__/SessionCard.spec.ts`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `SessionSnapshot.cwd: string`.
- Produces: `projectName(cwd: string): string` and `SessionCard` event `"open-project": [path: string]`.

- [ ] **Step 1: Write failing basename-helper tests**

Create `src/__tests__/projectName.spec.ts`:

```ts
import { describe, expect, it } from "vitest";
import { projectName } from "../lib/projectName";

describe("projectName", () => {
  it.each([
    ["/workspace/codex-pulse", "codex-pulse"],
    ["/workspace/codex-pulse/", "codex-pulse"],
    ["C:\\workspace\\codex-pulse", "codex-pulse"],
    ["C:\\workspace\\codex-pulse\\", "codex-pulse"],
    ["/", "/"],
    ["C:\\", "C:\\"]
  ])("derives the display label from %s", (cwd, expected) => {
    expect(projectName(cwd)).toBe(expected);
  });
});
```

- [ ] **Step 2: Run the helper test and verify RED**

Run:

```bash
pnpm test -- src/__tests__/projectName.spec.ts
```

Expected: FAIL because `src/lib/projectName.ts` does not exist.

- [ ] **Step 3: Add the minimal basename helper**

Create `src/lib/projectName.ts`:

```ts
export function projectName(cwd: string) {
  if (/^[A-Za-z]:[\\/]+$/.test(cwd)) return cwd;
  const withoutTrailingSeparators = cwd.replace(/[\\/]+$/, "");
  return withoutTrailingSeparators.split(/[\\/]/).pop() || cwd;
}
```

- [ ] **Step 4: Run the helper test and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/projectName.spec.ts
```

Expected: PASS, including Unix, Windows, trailing-separator, and root-path cases.

- [ ] **Step 5: Write failing SessionCard link tests**

In `src/__tests__/SessionCard.spec.ts`, replace the first test with:

```ts
it("shows title, project link, and both timers", async () => {
  const wrapper = mount(SessionCard, {
    props: {
      session: {
        threadId: "00000000-0000-4000-8000-000000000001",
        title: "Implement session monitor",
        cwd: "/workspace/project",
        sessionCreatedAtMs: 1_000,
        currentRunStartedAtMs: 61_000
      },
      nowMs: 121_000
    },
    global: { plugins: [i18n] }
  });

  expect(wrapper.text()).toContain("Implement session monitor");
  expect(wrapper.text()).toContain("Current run");
  expect(wrapper.text()).toContain("01:00");
  expect(wrapper.text()).toContain("Session age");
  expect(wrapper.text()).toContain("02:00");
  const projectLink = wrapper.get("a.session-card__path");
  expect(projectLink.text()).toBe("project");
  expect(projectLink.attributes("title")).toBe("/workspace/project");
  expect(wrapper.get("button").attributes("aria-label")).toContain("Open Codex task");

  await projectLink.trigger("click");
  expect(wrapper.emitted("open-project")).toEqual([["/workspace/project"]]);
  expect(wrapper.emitted("open")).toBeUndefined();
});
```

In the locale test, retain the existing Open Codex Task assertion so the old action remains covered.

- [ ] **Step 6: Run SessionCard tests and verify RED**

Run:

```bash
pnpm test -- src/__tests__/SessionCard.spec.ts
```

Expected: FAIL because `.session-card__path` is a `span`, still renders the full path, and does not emit `open-project`.

- [ ] **Step 7: Implement the anchor and preserve the exact path payload**

In `src/components/SessionCard.vue`, add the helper import and computed label:

```ts
import { projectName } from "../lib/projectName";

defineEmits<{
  open: [threadId: string];
  "open-project": [path: string];
}>();

const displayedProjectName = computed(() => projectName(props.session.cwd));
```

Replace the current path span with:

```vue
<a
  class="session-card__path"
  href="#"
  :title="session.cwd"
  @click.prevent="$emit('open-project', session.cwd)"
>{{ displayedProjectName }}</a>
```

In `src/styles.css`, replace the path rule and extend the focus selector:

```css
.session-card__path { display: block; width: fit-content; max-width: 100%; overflow: hidden; margin-top: 5px; color: #2467cc; font-size: 12px; text-decoration-line: underline; text-decoration-thickness: 1px; text-underline-offset: 2px; text-overflow: ellipsis; white-space: nowrap; }
.session-card__path:hover { color: #174fa6; }
.top-bar button:focus-visible, .session-card button:focus-visible, .session-card__path:focus-visible { outline: 3px solid rgba(52, 120, 246, 0.55); outline-offset: 2px; }
```

Replace the combined dark count/path color rule with separate rules:

```css
:root[data-theme="dark"] .top-bar__count { color: #a9b4c8; }
:root[data-theme="dark"] .session-card__path { color: #8ac2ff; }
:root[data-theme="dark"] .session-card__path:hover { color: #b9ddff; }
```

- [ ] **Step 8: Run focused frontend tests and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/projectName.spec.ts src/__tests__/SessionCard.spec.ts src/__tests__/footerLayout.spec.ts
```

Expected: PASS; the existing Open action and 24 px hit-target assertions remain green.

- [ ] **Step 9: Commit the project-link presentation**

Run:

```bash
git add src/lib/projectName.ts src/__tests__/projectName.spec.ts src/components/SessionCard.vue src/__tests__/SessionCard.spec.ts src/styles.css
git commit -m "feat: render project paths as links"
```

Expected: one commit containing only basename display, project-link emission, and link styling.

---

### Task 2: Wire project opening through the frontend

**Files:**
- Modify: `src/composables/usePulse.ts`
- Modify: `src/__tests__/usePulse.spec.ts`
- Modify: `src/App.vue`
- Modify: `src/__tests__/App.spec.ts`

**Interfaces:**
- Consumes: `SessionCard` event `open-project(path: string)` from Task 1.
- Produces: `usePulse.openProjectPath(path: string): Promise<void>` invoking `open_project_path` with `{ path }`.

- [ ] **Step 1: Write failing usePulse command and error tests**

Add to `src/__tests__/usePulse.spec.ts`:

```ts
it("opens a project through the validated native path command", async () => {
  const pulse = usePulse();
  invoke.mockResolvedValueOnce(undefined);

  await pulse.openProjectPath("/workspace/codex-pulse");

  expect(invoke).toHaveBeenCalledWith("open_project_path", {
    path: "/workspace/codex-pulse"
  });
  expect(pulse.error.value).toBeUndefined();
});

it("surfaces a native project-open failure", async () => {
  const pulse = usePulse();
  invoke.mockRejectedValueOnce(new Error("Project path is not a directory"));

  await pulse.openProjectPath("/workspace/file.txt");

  expect(pulse.error.value).toBe("Project path is not a directory");
});
```

- [ ] **Step 2: Run usePulse tests and verify RED**

Run:

```bash
pnpm test -- src/__tests__/usePulse.spec.ts
```

Expected: FAIL because `openProjectPath` is not part of the composable.

- [ ] **Step 3: Add the minimal frontend command wrapper**

In `src/composables/usePulse.ts`, place this next to `openThread`:

```ts
async function openProjectPath(path: string) {
  try {
    error.value = undefined;
    await invoke("open_project_path", { path });
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  }
}
```

Add `openProjectPath` to the returned object:

```ts
return { snapshot, error, load, togglePin, openThread, openProjectPath, enableMonitoring, mergeInitializationEvent, setTheme, setLocale };
```

- [ ] **Step 4: Run usePulse tests and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/usePulse.spec.ts
```

Expected: PASS, including exact command payload and existing error behavior.

- [ ] **Step 5: Write the failing App wiring contract**

In `src/__tests__/App.spec.ts`, add:

```ts
it("routes project-link events to the project path command", () => {
  const source = readFileSync(resolve(process.cwd(), "src/App.vue"), "utf8");
  expect(source).toContain('@open-project="pulse.openProjectPath"');
});
```

- [ ] **Step 6: Run the App test and verify RED**

Run:

```bash
pnpm test -- src/__tests__/App.spec.ts
```

Expected: FAIL because `App.vue` does not bind the new event.

- [ ] **Step 7: Connect SessionCard to usePulse**

In the `SessionCard` instance in `src/App.vue`, add:

```vue
@open-project="pulse.openProjectPath"
```

Keep `@open="pulse.openThread"` unchanged.

- [ ] **Step 8: Run frontend wiring tests and verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/usePulse.spec.ts src/__tests__/App.spec.ts src/__tests__/SessionCard.spec.ts
```

Expected: PASS.

- [ ] **Step 9: Commit the frontend command wiring**

Run:

```bash
git add src/composables/usePulse.ts src/__tests__/usePulse.spec.ts src/App.vue src/__tests__/App.spec.ts
git commit -m "feat: route project links to native opener"
```

Expected: one commit containing only the frontend command wrapper and event wiring.

---

### Task 3: Validate and open project directories in Rust

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/app.rs`

**Interfaces:**
- Consumes: Tauri invoke `open_project_path { path: String }` from Task 2.
- Produces: `validate_project_path(path: &str) -> Result<PathBuf, String>` and Tauri command `open_project_path(path: String, app: tauri::AppHandle) -> Result<(), String>`.

- [ ] **Step 1: Write failing Rust path-validation tests**

In the `commands.rs` test module, import `validate_project_path` and add:

```rust
#[test]
fn validates_project_directories_before_opening() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("notes.txt");
    std::fs::write(&file, "not a directory").unwrap();
    let missing = temp.path().join("missing");

    assert_eq!(
        validate_project_path(temp.path().to_str().unwrap()).unwrap(),
        temp.path()
    );
    assert!(validate_project_path("   ").unwrap_err().contains("empty"));
    assert!(validate_project_path(missing.to_str().unwrap())
        .unwrap_err()
        .contains("Could not access project path"));
    assert!(validate_project_path(file.to_str().unwrap())
        .unwrap_err()
        .contains("not a directory"));
}
```

- [ ] **Step 2: Run the Rust test and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml commands::tests::validates_project_directories_before_opening
```

Expected: compilation FAIL because `validate_project_path` does not exist.

- [ ] **Step 3: Add directory validation and the opener command**

In `src-tauri/src/commands.rs`, use the already imported `Path`/`PathBuf` types and add before `validate_external_url`:

```rust
fn validate_project_path(path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("Project path is empty".into());
    }
    let path = PathBuf::from(path);
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("Could not access project path {}: {error}", path.display()))?;
    if !metadata.is_dir() {
        return Err(format!("Project path is not a directory: {}", path.display()));
    }
    Ok(path)
}

#[tauri::command]
pub fn open_project_path(path: String, app: tauri::AppHandle) -> Result<(), String> {
    let path = validate_project_path(&path)?;
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<String>)
        .map_err(|error| error.to_string())
}
```

In the test-module import list, add `validate_project_path`.

- [ ] **Step 4: Register the Tauri command**

In `src-tauri/src/app.rs`, add this handler directly after `open_thread`:

```rust
crate::commands::open_project_path,
```

Do not add an opener capability to `src-tauri/capabilities/default.json`; the Rust extension API performs the handoff.

- [ ] **Step 5: Format and run focused Rust tests to verify GREEN**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml commands::tests::validates_project_directories_before_opening
cargo test --manifest-path src-tauri/Cargo.toml commands::tests::accepts_only_safe_external_handoff_schemes
```

Expected: PASS; the existing external URL scheme boundary remains unchanged.

- [ ] **Step 6: Commit the native opener boundary**

Run:

```bash
git add src-tauri/src/commands.rs src-tauri/src/app.rs
git commit -m "feat: open validated project directories"
```

Expected: one commit with the command, validation tests, and handler registration.

---

### Task 4: Isolate theme selected and hover states

**Files:**
- Create: `src/__tests__/themeControls.spec.ts`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `TopBar` buttons with `aria-pressed="true" | "false"` and resolved `:root[data-theme="dark"]` from `useTheme`.
- Produces: selected white-on-`#3478f6` rules and unselected scheme-specific hover rules.

- [ ] **Step 1: Write the failing CSS regression test**

Create `src/__tests__/themeControls.spec.ts`:

```ts
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const stylesheet = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

function rule(selector: string) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return stylesheet.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`))?.[1] ?? "";
}

describe("theme control states", () => {
  it("keeps selected controls white on blue even while hovered", () => {
    const light = rule('.top-bar__theme-group button[aria-pressed="true"]:hover');
    const dark = rule(':root[data-theme="dark"] .top-bar__theme-group button[aria-pressed="true"]:hover');
    expect(light).toContain("color: #fff;");
    expect(light).toContain("background: #3478f6;");
    expect(dark).toContain("color: #fff;");
    expect(dark).toContain("background: #3478f6;");
  });

  it("applies scheme-specific hover surfaces only to unselected controls", () => {
    expect(rule('.top-bar__theme-group button[aria-pressed="false"]:hover'))
      .toContain("background: rgba(52, 120, 246, 0.14);");
    expect(rule(':root[data-theme="dark"] .top-bar__theme-group button[aria-pressed="false"]:hover'))
      .toContain("background: rgba(138, 194, 255, 0.18);");
    expect(stylesheet).not.toContain(".top-bar button:hover { background:");
  });
});
```

- [ ] **Step 2: Run the CSS test and verify RED**

Run:

```bash
pnpm test -- src/__tests__/themeControls.spec.ts
```

Expected: FAIL because selected-hover and unselected-hover selectors do not exist and the generic top-bar hover rule still applies to the theme group.

- [ ] **Step 3: Replace generic theme hover with explicit states**

In the light rules of `src/styles.css`, replace the selected and generic hover blocks with:

```css
.top-bar__theme-group button[aria-pressed="true"], .top-bar__theme-group button[aria-pressed="true"]:hover { color: #fff; background: #3478f6; box-shadow: 0 1px 4px rgba(38, 89, 185, 0.28); }
.top-bar__theme-group button[aria-pressed="false"]:hover { background: rgba(52, 120, 246, 0.14); }
.top-bar__locale button:hover, .top-bar__pin:hover { background: rgba(255, 255, 255, 0.78); }
```

In the dark rules, replace the current generic theme selected and top-bar hover blocks with:

```css
:root[data-theme="dark"] .top-bar__theme-group button[aria-pressed="true"], :root[data-theme="dark"] .top-bar__theme-group button[aria-pressed="true"]:hover { color: #fff; background: #3478f6; }
:root[data-theme="dark"] .top-bar__theme-group button[aria-pressed="false"]:hover { background: rgba(138, 194, 255, 0.18); }
:root[data-theme="dark"] .top-bar__locale button:hover, :root[data-theme="dark"] .top-bar__pin:hover, :root[data-theme="dark"] .session-card:hover { background: rgba(47, 65, 98, 0.72); }
```

Retain the existing `button:active` and focus-visible rules.

- [ ] **Step 4: Run theme and TopBar tests to verify GREEN**

Run:

```bash
pnpm test -- src/__tests__/themeControls.spec.ts src/__tests__/TopBar.spec.ts
```

Expected: PASS; emitted theme values and CSS state contracts are both green.

- [ ] **Step 5: Commit the theme state repair**

Run:

```bash
git add src/__tests__/themeControls.spec.ts src/styles.css
git commit -m "fix: preserve theme selection on hover"
```

Expected: one commit containing only the theme CSS regression and repair.

---

### Task 5: Verify the integrated application

**Files:**
- Verify: `src/**/*.{ts,vue,css}`
- Verify: `src-tauri/src/**/*.rs`
- Build output: `dist/` and the local Tauri debug application bundle.

**Interfaces:**
- Consumes: completed Tasks 1-4.
- Produces: automated and desktop evidence that project opening and theme states work together without regressions.

- [ ] **Step 1: Run formatting and repository hygiene checks**

Run:

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml
git diff --check
git status --short
```

Expected: formatting and whitespace checks exit 0; status contains no uncommitted implementation files.

- [ ] **Step 2: Run the complete frontend and Rust verification**

Run:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all Vitest files pass, `vue-tsc --noEmit` and Vite build succeed, and all Rust tests pass.

- [ ] **Step 3: Build and launch a debug desktop bundle**

Run:

```bash
pnpm tauri build --debug
test -x "src-tauri/target/debug/bundle/macos/Codex Pulse.app/Contents/MacOS/CodexPulse"
open "src-tauri/target/debug/bundle/macos/Codex Pulse.app"
```

Expected on macOS: the debug bundle builds, its executable exists, and Codex Pulse opens. On Windows, run the generated debug executable instead of the macOS `test`/`open` commands.

- [ ] **Step 4: Exercise the approved desktop behavior**

With at least one active session visible:

1. Confirm the path row displays only the directory basename and its native tooltip shows the complete path.
2. Click it and confirm Finder/Explorer opens the exact directory; confirm the existing Open Codex Task icon still opens the Codex task instead.
3. In resolved light mode, hover each unselected theme button and confirm a pale blue surface appears; hover the selected button and confirm it remains white-on-blue.
4. In resolved dark mode, repeat and confirm the unselected blue-gray surface is visible while selected remains white-on-blue.
5. Select `system`, change the operating-system appearance, and confirm the appropriate resolved hover palette is used while the system button remains selected.

- [ ] **Step 5: Re-run affected gates if desktop inspection required a fix**

If inspection changes source, first add a failing automated regression, then repeat its RED/GREEN cycle and run:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

Expected: all commands exit 0 before any completion claim. If inspection requires no change, do not create an empty commit.
