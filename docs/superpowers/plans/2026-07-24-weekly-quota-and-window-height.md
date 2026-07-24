# Default Weekly Quota and Free-Height Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the footer select only the default Codex weekly quota and remove the main window's vertical maximum while preserving its current width and minimum-size constraints.

**Architecture:** Filter quota identity at the JSONL parser boundary so both active-session and bounded quota caches receive only `limit_id == "codex"` observations. Replace the coupled Tauri min/max size calls with one explicit `WindowSizeConstraints` value whose `max_height` is `None`.

**Tech Stack:** Rust, serde_json, Tauri 2.11, Cargo test, Vue 3, TypeScript, Vitest, pnpm

## Global Constraints

- The footer displays only `rate_limits.limit_id == "codex"`.
- Model-specific, tier-specific, and unidentified limits do not produce `WeeklyQuota`.
- Weekly buckets still require `window_minutes == 10080`.
- Keep minimum width 320, minimum height 360, and maximum width 480 logical pixels.
- Set no application maximum height.
- Keep initial size, maximization, resizability, decorations, transparency, always-on-top, liquid glass, close-to-hide, refresh timing, expiration, and the frontend quota contract unchanged.
- Do not add a multi-quota UI or include session/context/AskHuman branch work.

---

## File map

| File | Responsibility |
| --- | --- |
| `src-tauri/src/codex/jsonl.rs` | Reject non-default quota families at the parser boundary and own parser regression tests. |
| `src-tauri/src/monitor.rs` | Prove the incremental quota source cannot be overwritten by a newer model-specific 100% observation. |
| `src-tauri/src/codex/discovery.rs` | Keep existing transcript-cache quota fixtures explicit about the default quota identity. |
| `src-tauri/src/commands.rs` | Keep existing snapshot quota fixtures explicit about the default quota identity. |
| `src-tauri/src/app.rs` | Construct and apply independent logical window size constraints and own their unit test. |

### Task 1: Select only the default Codex weekly quota

**Files:**

- Modify: `src-tauri/src/codex/jsonl.rs:189-210,346-368`
- Modify: `src-tauri/src/monitor.rs:430-520`
- Modify: `src-tauri/src/codex/discovery.rs:450-515`
- Modify: `src-tauri/src/commands.rs:589-614`

**Interfaces:**

- Consumes: JSONL `event_msg` records with `payload.type == "token_count"` and `payload.rate_limits.limit_id`.
- Produces: the existing `ParsedRecord::WeeklyQuota(WeeklyQuota)` only for `limit_id == "codex"`; no public type changes.

- [ ] **Step 1: Add the parser identity regression test**

Update the existing accepted fixtures to carry the default identity and add a
test that rejects every other identity:

```rust
#[test]
fn parses_default_codex_weekly_quota_from_primary_or_secondary_window() {
    let primary = r#"{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":81.0,"window_minutes":10080,"resets_at":1784870653},"secondary":null}}}"#;
    let secondary = r#"{"timestamp":"2026-07-17T12:01:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":12.0,"window_minutes":300,"resets_at":1784800000},"secondary":{"used_percent":22.0,"window_minutes":10080,"resets_at":1784871000}}}}"#;

    for (line, used, remaining) in [(primary, 81, 19), (secondary, 22, 78)] {
        let ParsedRecord::WeeklyQuota(quota) = parse_line(line).unwrap().unwrap() else {
            panic!("expected weekly quota");
        };
        assert_eq!(quota.used_percent, used);
        assert_eq!(quota.remaining_percent, remaining);
    }
}

#[test]
fn ignores_weekly_quota_that_is_not_the_default_codex_limit() {
    for limit_id in [Some("codex_bengalfox"), Some("premium"), None] {
        let limit_id = limit_id
            .map(|value| format!(r#""limit_id":"{value}","#))
            .unwrap_or_default();
        let line = format!(
            r#"{{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{{limit_id}"primary":{{"used_percent":100.0,"window_minutes":10080,"resets_at":1784870653}}}}}}}}"#
        );

        assert!(matches!(
            parse_line(&line).unwrap(),
            Some(ParsedRecord::Ignored)
        ));
    }
}
```

Also make the non-weekly fixture explicit:

```rust
let line = r#"{"timestamp":"2026-07-17T12:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":81.0,"window_minutes":300,"resets_at":1784870653}}}}"#;
```

- [ ] **Step 2: Run the parser test to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex::jsonl::tests::ignores_weekly_quota_that_is_not_the_default_codex_limit -- --exact
```

Expected: FAIL because the current parser returns `ParsedRecord::WeeklyQuota`
for `codex_bengalfox`, `premium`, and a missing identifier.

- [ ] **Step 3: Add the incremental cache regression test**

Add this test to `src-tauri/src/monitor.rs`:

```rust
#[test]
fn quota_source_keeps_default_quota_when_newer_model_limit_is_at_one_hundred() {
    let temp = tempfile::tempdir().unwrap();
    let day = chrono::Local::now().format("%Y/%m/%d").to_string();
    let sessions = temp.path().join("sessions").join(day);
    fs::create_dir_all(&sessions).unwrap();
    let transcript = sessions.join("mixed-limits.jsonl");
    fs::write(
        &transcript,
        concat!(
            "{\"timestamp\":\"2026-07-17T07:01:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"limit_id\":\"codex_bengalfox\",\"primary\":{\"used_percent\":100.0,\"window_minutes\":10080,\"resets_at\":3}}}}\n",
            "{\"timestamp\":\"2026-07-17T07:02:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"rate_limits\":{\"limit_id\":\"codex\",\"primary\":{\"used_percent\":64.0,\"window_minutes\":10080,\"resets_at\":2}}}}\n"
        ),
    )
    .unwrap();
    let mut cache = QuotaSourceCache::default();

    assert_eq!(
        cache
            .latest_weekly_quota(temp.path(), 1_000)
            .unwrap()
            .used_percent,
        64
    );

    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&transcript)
        .unwrap();
    writeln!(
        file,
        "{{\"timestamp\":\"2026-07-17T07:03:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"rate_limits\":{{\"limit_id\":\"codex_bengalfox\",\"primary\":{{\"used_percent\":100.0,\"window_minutes\":10080,\"resets_at\":3}}}}}}}}"
    )
    .unwrap();

    assert_eq!(
        cache
            .latest_weekly_quota(temp.path(), 1_000)
            .unwrap()
            .used_percent,
        64
    );
}
```

- [ ] **Step 4: Run the cache test to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml monitor::tests::quota_source_keeps_default_quota_when_newer_model_limit_is_at_one_hundred -- --exact
```

Expected: FAIL. The first assertion currently returns 64, but after the append
the cache advances to the newer model-specific 100% observation.

- [ ] **Step 5: Add the parser-boundary identity gate**

Make `parse_weekly_quota` reject every non-default or unidentified family
before examining buckets:

```rust
fn parse_weekly_quota(rate_limits: &Value, observed_at_ms: i64) -> Option<WeeklyQuota> {
    if rate_limits.get("limit_id").and_then(Value::as_str) != Some("codex") {
        return None;
    }

    let bucket = ["primary", "secondary"].into_iter().find_map(|slot| {
        rate_limits
            .get(slot)
            .filter(|bucket| bucket.get("window_minutes").and_then(Value::as_i64) == Some(10_080))
    })?;
    let used_percent = bucket
        .get("used_percent")
        .and_then(Value::as_f64)?
        .round()
        .clamp(0.0, 100.0) as u8;
    let resets_at_ms = bucket
        .get("resets_at")
        .and_then(Value::as_i64)?
        .saturating_mul(1_000);

    Some(WeeklyQuota {
        used_percent,
        remaining_percent: 100 - used_percent,
        resets_at_ms,
        observed_at_ms,
    })
}
```

- [ ] **Step 6: Make all existing default-quota fixtures explicit**

In each existing fixture in these files:

```text
src-tauri/src/codex/jsonl.rs
src-tauri/src/codex/discovery.rs
src-tauri/src/monitor.rs
src-tauri/src/commands.rs
```

apply these exact structural replacements:

```text
"rate_limits":{"primary":
"rate_limits":{"limit_id":"codex","primary":
```

and, inside escaped `format!`/`writeln!` JSON:

```text
\"rate_limits\":{{\"primary\":
\"rate_limits\":{{\"limit_id\":\"codex\",\"primary\":
```

Do not change the new `codex_bengalfox` regression fixtures. Confirm that every
pre-existing accepted default fixture is explicit:

```bash
rg -n 'rate_limits.*window_minutes.*10080' src-tauri/src/{codex/jsonl.rs,codex/discovery.rs,monitor.rs,commands.rs}
```

Expected: accepted default fixtures include `limit_id` set to `codex`; the only
other identifiers belong to the new rejection/regression tests.

- [ ] **Step 7: Run quota tests to verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml quota
```

Expected: all parser, transcript-cache, quota-source, and snapshot tests
matching `quota` PASS, including the two new regressions.

- [ ] **Step 8: Commit the quota fix**

```bash
git add src-tauri/src/codex/jsonl.rs src-tauri/src/codex/discovery.rs src-tauri/src/monitor.rs src-tauri/src/commands.rs
git commit -m "fix: select default Codex weekly quota"
```

### Task 2: Remove only the main window's height maximum

**Files:**

- Modify: `src-tauri/src/app.rs:38-69,87`

**Interfaces:**

- Consumes: Tauri `LogicalUnit` and `WindowSizeConstraints`.
- Produces: `main_window_size_constraints() -> WindowSizeConstraints`, used by the main `WebviewWindowBuilder`.

- [ ] **Step 1: Add the window-constraint regression test**

Append a focused test module to `src-tauri/src/app.rs`:

```rust
#[cfg(test)]
mod tests {
    use tauri::{LogicalUnit, PixelUnit};

    use super::main_window_size_constraints;

    #[test]
    fn main_window_has_bounded_width_and_no_maximum_height() {
        let constraints = main_window_size_constraints();

        assert_eq!(
            constraints.min_width,
            Some(PixelUnit::Logical(LogicalUnit::new(320.0)))
        );
        assert_eq!(
            constraints.min_height,
            Some(PixelUnit::Logical(LogicalUnit::new(360.0)))
        );
        assert_eq!(
            constraints.max_width,
            Some(PixelUnit::Logical(LogicalUnit::new(480.0)))
        );
        assert_eq!(constraints.max_height, None);
    }
}
```

- [ ] **Step 2: Run the window test to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml app::tests::main_window_has_bounded_width_and_no_maximum_height -- --exact
```

Expected: compilation FAILS because `main_window_size_constraints` does not
exist.

- [ ] **Step 3: Construct one independent constraint value**

Add this helper above `create_main_window`:

```rust
fn main_window_size_constraints() -> tauri::WindowSizeConstraints {
    use tauri::{LogicalUnit, WindowSizeConstraints};

    WindowSizeConstraints {
        min_width: Some(LogicalUnit::new(320.0).into()),
        min_height: Some(LogicalUnit::new(360.0).into()),
        max_width: Some(LogicalUnit::new(480.0).into()),
        max_height: None,
    }
}
```

Update the builder to use that value:

```rust
fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    use tauri_plugin_liquid_glass::{LiquidGlassConfig, LiquidGlassExt};

    let always_on_top = app
        .state::<crate::commands::AppState>()
        .config
        .lock()
        .map(|config| config.always_on_top)
        .unwrap_or_default();

    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title(product_name())
        .inner_size(360.0, 420.0)
        .inner_size_constraints(main_window_size_constraints())
        .transparent(true)
        .decorations(true)
        .always_on_top(always_on_top)
        .resizable(true)
        .build()?;

    let _ = window.maximize();

    let _ = app.liquid_glass().set_effect(
        &window,
        LiquidGlassConfig {
            corner_radius: 22.0,
            tint_color: Some("#10131d24".into()),
            ..Default::default()
        },
    );
    let window_for_close = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_for_close.hide();
        }
    });
    Ok(())
}
```

Remove both of these old mechanisms:

```rust
.min_inner_size(320.0, 360.0)
.max_inner_size(480.0, 10_000.0)
```

and remove the complete monitor work-area block:

```rust
if let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) {
    let work_area = monitor.work_area().size;
    let scale_factor = monitor.scale_factor();
    let max_height = ((work_area.height as f64 / scale_factor) - 16.0).max(360.0);
    window.set_max_size(Some(LogicalSize::new(480.0, max_height)))?;
}
```

- [ ] **Step 4: Run the window test to verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml app::tests::main_window_has_bounded_width_and_no_maximum_height -- --exact
```

Expected: PASS.

- [ ] **Step 5: Format and commit the window fix**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml app::tests::main_window_has_bounded_width_and_no_maximum_height -- --exact
```

Expected: formatting completes and the focused test PASSes.

Commit:

```bash
git add src-tauri/src/app.rs
git commit -m "fix: allow unrestricted window height"
```

### Task 3: Verify the combined release behavior

**Files:**

- Verify only: `src-tauri/src/codex/jsonl.rs`
- Verify only: `src-tauri/src/monitor.rs`
- Verify only: `src-tauri/src/app.rs`
- Verify only: `src/**`

**Interfaces:**

- Consumes: both independently committed fixes.
- Produces: automated gate evidence plus a desktop comparison against the newest local default Codex quota.

- [ ] **Step 1: Run Rust formatting and the full Rust suite**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: formatting check exits 0 and every Rust test PASSes.

- [ ] **Step 2: Run the frontend suite and production build**

```bash
pnpm test
pnpm build
```

Expected: every Vitest test PASSes; `vue-tsc` and Vite finish successfully and
produce `dist/`.

- [ ] **Step 3: Build the desktop debug bundle**

```bash
pnpm tauri build --debug
```

Expected: Tauri produces a debug `Codex Pulse.app` bundle without compile or
bundle errors.

- [ ] **Step 4: Establish the local default-quota oracle**

Run this read-only command:

```bash
task_codex_home="${CODEX_HOME:-$HOME/.codex}"
find "$task_codex_home/sessions" -type f -name '*.jsonl' -print0 \
  | xargs -0 jq -rc 'select(.type == "event_msg" and .payload.type == "token_count" and .payload.rate_limits.limit_id == "codex") | ([.payload.rate_limits.primary, .payload.rate_limits.secondary] | map(select(.window_minutes == 10080)) | first) as $bucket | select($bucket != null) | {timestamp,used:$bucket.used_percent,resets_at:$bucket.resets_at}' \
  | jq -s 'sort_by(.timestamp) | last'
```

Expected: one JSON object containing the newest locally observed default
Codex quota, selected deterministically by its observation timestamp.

- [ ] **Step 5: Perform the desktop acceptance check**

Before replacing or stopping the running installed app, ask through AskHuman
for approval to restart Codex Pulse with the newly built debug bundle. Then:

1. inspect the footer and confirm its used percentage and reset time match the
   Step 4 `limit_id == "codex"` oracle;
2. restore the window from maximized state;
3. drag the vertical edge beyond the previous application cap where the
   current display allows it;
4. confirm the maximum width remains bounded and the minimum size remains
   usable;
5. report any OS-level screen boundary separately from application-supplied
   constraints.

Expected: the footer matches the default Codex limit, no model-specific 0% or
100% replaces it, and no application maximum height is present.

- [ ] **Step 6: Verify repository state and exact commits**

```bash
git status --short
git log -3 --oneline --decorate
git show --stat --oneline HEAD
git diff HEAD~2..HEAD --check
```

Expected: the worktree is clean; the quota and window commits are present; the
two-commit implementation diff has no whitespace errors.
