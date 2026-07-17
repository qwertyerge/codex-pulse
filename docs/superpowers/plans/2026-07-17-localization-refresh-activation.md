# Localization, Refresh, Footer Motion, and Codex Hand-off Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Localize Codex Pulse into Chinese, English, French, and German, reduce fallback scans to 60 seconds, animate background-event footer expansion, and retain the existing Codex task deep-link hand-off.

**Architecture:** Rust owns validated persisted locale preferences and the 60-second fallback scheduler. Vue owns resolved system-locale selection, translated product copy, the locale menu, and footer animation. Hook events continue to bypass the fallback timer, while the existing deep link remains the only mechanism that selects a Codex task.

**Tech Stack:** Rust 2021, Tauri 2, Vue 3, TypeScript, `vue-i18n` 11, Lucide Vue, Vitest, Vue Test Utils.

## Global Constraints

- Persist only `system | zh-CN | en | fr | de`; an unknown locale must never be written to `config.json`.
- Translate Codex Pulse-owned copy, labels, tooltips, and known initialization phase summaries; never translate task data, paths, prompt text, Recent text, or backend error details.
- System locale maps `zh*`, `fr*`, and `de*` respectively; every other system language resolves to English.
- Fallback reconciliation and WebView snapshot polling run every 60 seconds; startup and hook-driven refreshes remain immediate.
- Task opening remains the existing validated `codex://threads/<uuid>` hand-off; do not add a second activation path.
- Footer remains bottom-anchored, reserves 48px without an event and 72px with one, and respects `prefers-reduced-motion`.

---

### Task 1: Add typed locale persistence and 60-second reconciliation

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/app.rs`
- Test: `src-tauri/src/config.rs`
- Test: `src-tauri/src/commands.rs`

**Interfaces:**
- Produces `LocaleMode::{System, ZhCn, English, French, German}` serialized as `system`, `zh-CN`, `en`, `fr`, and `de`.
- Produces `set_locale(locale: LocaleMode, state: State<AppState>) -> Result<LocaleMode, String>`.
- Produces `FALLBACK_RECONCILIATION_SECONDS: u64 = 60`.

- [ ] **Step 1: Write failing Rust tests for the closed locale contract and scheduler**

  Add tests that persist French, assert the default remains System, and assert the fallback interval constant is 60:

  ```rust
  #[test]
  fn persists_an_explicit_locale_choice() {
      let temp = tempfile::tempdir().unwrap();
      let store = ConfigStore::new(temp.path().join("config.json"));
      store.save(&AppConfig { locale: LocaleMode::French, ..AppConfig::default() }).unwrap();
      assert_eq!(store.load().unwrap().locale, LocaleMode::French);
  }

  #[test]
  fn fallback_reconciliation_is_limited_to_one_minute() {
      assert_eq!(FALLBACK_RECONCILIATION_SECONDS, 60);
  }
  ```

- [ ] **Step 2: Run the focused Rust tests and verify they fail**

  Run:

  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml locale && \
  cargo test --manifest-path src-tauri/Cargo.toml fallback_reconciliation
  ```

  Expected: compilation fails because `LocaleMode` and `FALLBACK_RECONCILIATION_SECONDS` do not exist.

- [ ] **Step 3: Implement the locale enum, setter, and scheduler constant**

  In `model.rs`, use explicit serde names so the persisted and IPC values match the frontend exactly:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum LocaleMode {
      #[serde(rename = "system")] System,
      #[serde(rename = "zh-CN")] ZhCn,
      #[serde(rename = "en")] English,
      #[serde(rename = "fr")] French,
      #[serde(rename = "de")] German,
  }

  impl Default for LocaleMode {
      fn default() -> Self { Self::System }
  }
  ```

  Replace `AppConfig.locale: String` and `AppSnapshot.locale: String` with `LocaleMode`. Add `set_locale` beside `set_theme`, cloning/saving/replacing configuration atomically. Register it in `app.rs`.

  Replace the literal `15` in `start_fallback_reconciliation` with:

  ```rust
  pub const FALLBACK_RECONCILIATION_SECONDS: u64 = 60;
  tokio::time::sleep(std::time::Duration::from_secs(FALLBACK_RECONCILIATION_SECONDS)).await;
  ```

- [ ] **Step 4: Run focused Rust tests and format**

  Run:

  ```bash
  cargo fmt --manifest-path src-tauri/Cargo.toml --check
  cargo test --manifest-path src-tauri/Cargo.toml locale && \
  cargo test --manifest-path src-tauri/Cargo.toml fallback_reconciliation
  ```

  Expected: all focused tests pass.

- [ ] **Step 5: Commit the native contract**

  ```bash
  git add src-tauri/src/model.rs src-tauri/src/config.rs src-tauri/src/commands.rs src-tauri/src/app.rs
  git commit -m "feat: persist locale preferences"
  ```

### Task 2: Establish the vue-i18n runtime and persisted locale behavior

**Files:**
- Create: `src/i18n.ts`
- Create: `src/composables/useLocale.ts`
- Modify: `src/main.ts`
- Modify: `src/types.ts`
- Modify: `src/composables/usePulse.ts`
- Test: `src/__tests__/i18n.spec.ts`
- Test: `src/__tests__/usePulse.spec.ts`

**Interfaces:**
- Produces `LocaleMode = "system" | "zh-CN" | "en" | "fr" | "de"`.
- Produces `resolveLocale(preference, browserLanguage): ResolvedLocale` and `useLocale(preference)`.
- Produces `setLocale(locale: LocaleMode): Promise<void>` from `usePulse`.

- [ ] **Step 1: Write failing frontend tests for resolution and persistence rollback**

  ```ts
  it.each([
    ["system", "zh-Hans-CN", "zh-CN"],
    ["system", "fr-CA", "fr"],
    ["system", "de-AT", "de"],
    ["system", "ja-JP", "en"],
    ["de", "zh-CN", "de"]
  ] as const)("resolves %s with %s to %s", (preference, browserLanguage, expected) => {
    expect(resolveLocale(preference, browserLanguage)).toBe(expected);
  });

  it("rolls back a locale when native persistence fails", async () => {
    invoke.mockRejectedValueOnce(new Error("config unavailable"));
    const pulse = usePulse();
    await pulse.setLocale("fr");
    expect(pulse.snapshot.value.locale).toBe("system");
  });
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run:

  ```bash
  pnpm test -- src/__tests__/i18n.spec.ts src/__tests__/usePulse.spec.ts
  ```

  Expected: missing locale module and `setLocale` cause failures.

- [ ] **Step 3: Implement `src/i18n.ts`, `useLocale`, and locale persistence**

  Create one i18n instance and translation dictionary for all product-owned keys. The dictionary must contain `en`, `zh-CN`, `fr`, and `de` entries for: top-bar labels; locale menu labels; empty/monitoring states; quota labels; session-card timer/disclosure text; accessibility text; initialization phase labels; and `END`.

  ```ts
  export const i18n = createI18n({
    legacy: false,
    locale: "en",
    fallbackLocale: "en",
    messages
  });

  export function resolveLocale(preference: LocaleMode, browserLanguage = navigator.language): ResolvedLocale {
    if (preference !== "system") return preference;
    const language = browserLanguage.toLowerCase();
    if (language.startsWith("zh")) return "zh-CN";
    if (language.startsWith("fr")) return "fr";
    if (language.startsWith("de")) return "de";
    return "en";
  }
  ```

  `useLocale` watches the stored preference, assigns `i18n.global.locale.value`, and listens for `languagechange` only while preference is System. Update `main.ts` to install `i18n` before mounting. Extend `usePulse` with the same optimistic/rollback pattern already used by `setTheme`:

  ```ts
  async function setLocale(locale: LocaleMode) {
    const previous = snapshot.value;
    snapshot.value = { ...previous, locale };
    try {
      const saved = await invoke<LocaleMode>("set_locale", { locale });
      snapshot.value = { ...snapshot.value, locale: saved };
    } catch (reason) {
      snapshot.value = previous;
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  }
  ```

- [ ] **Step 4: Run locale tests and TypeScript build**

  Run:

  ```bash
  pnpm test -- src/__tests__/i18n.spec.ts src/__tests__/usePulse.spec.ts
  pnpm build
  ```

  Expected: system mapping and persistence rollback pass; Vue type checking succeeds.

- [ ] **Step 5: Commit the locale runtime**

  ```bash
  git add src/i18n.ts src/composables/useLocale.ts src/main.ts src/types.ts src/composables/usePulse.ts src/__tests__/i18n.spec.ts src/__tests__/usePulse.spec.ts
  git commit -m "feat: add localized UI runtime"
  ```

### Task 3: Localize product copy and add the accessible language menu

**Files:**
- Modify: `src/App.vue`
- Modify: `src/components/TopBar.vue`
- Modify: `src/components/EmptyState.vue`
- Modify: `src/components/MonitoringBanner.vue`
- Modify: `src/components/FooterStatus.vue`
- Modify: `src/components/InitializationFeed.vue`
- Modify: `src/components/InitializationStatusRow.vue`
- Modify: `src/components/SessionCard.vue`
- Modify: `src/components/MarkdownContent.vue`
- Modify: `src/styles.css`
- Test: `src/__tests__/TopBar.spec.ts`
- Test: `src/__tests__/SessionCard.spec.ts`
- Test: `src/__tests__/FooterStatus.spec.ts`
- Test: `src/__tests__/InitializationStatusRow.spec.ts`

**Interfaces:**
- `TopBar` consumes `locale: LocaleMode` and emits `set-locale` with a `LocaleMode`.
- `App` starts `useLocale(computed(() => pulse.snapshot.value.locale))` and passes `pulse.setLocale` to `TopBar`.
- Known initialization phases render through translation keys while unknown error summaries render verbatim.

- [ ] **Step 1: Write failing component tests for the menu and localized controls**

  ```ts
  it("opens the language menu and emits French selection", async () => {
    const wrapper = mount(TopBar, { props: { activeCount: 2, alwaysOnTop: false, theme: "system", locale: "en" }, global: { plugins: [i18n] } });
    await wrapper.get('[aria-label="Choose language"]').trigger("click");
    await wrapper.get('[data-locale="fr"]').trigger("click");
    expect(wrapper.emitted("set-locale")?.[0]).toEqual(["fr"]);
    expect(wrapper.text()).toContain("Français");
  });

  it("uses translated task and timer labels", () => {
    i18n.global.locale.value = "de";
    const wrapper = mount(SessionCard, { props: { session, nowMs: 60_000 }, global: { plugins: [i18n] } });
    expect(wrapper.get("button").attributes("aria-label")).toContain("Codex-Aufgabe");
  });
  ```

- [ ] **Step 2: Run component tests to verify they fail**

  Run:

  ```bash
  pnpm test -- src/__tests__/TopBar.spec.ts src/__tests__/SessionCard.spec.ts src/__tests__/FooterStatus.spec.ts src/__tests__/InitializationStatusRow.spec.ts
  ```

  Expected: the locale prop, menu, and translated labels are missing.

- [ ] **Step 3: Migrate only product-owned text to `t(...)` calls**

  Add `Languages` from Lucide to `TopBar`. Place a menu trigger after `.top-bar__theme-group` and before `.top-bar__pin`; give it `aria-label="Choose language"`, menu items with `data-locale`, and close the menu after selection. Translate active-count, appearance, pin, timer, prompt, recent, quota, monitoring, empty-state, initialization, accessibility copy, and the list-end label. Keep `session.title`, `session.cwd`, prompt content, event summaries, and `degradedReason` as raw values. Translate the image placeholder fallback only when Markdown supplies neither alt text nor title.

  Replace the CSS-generated `END` pseudo-element with a keyed, presentational `.session-list__end` DOM label in the existing `TransitionGroup`. Preserve its one-pixel separator, centered text gap, and scroll behavior, while allowing the label to render through `t("list.end")`.

  In footer copy use interpolation rather than string concatenation:

  ```vue
  <span>{{ t("quota.usedRemaining", { used: usedPercent, remaining: quota.remainingPercent }) }}</span>
  <time>{{ t("quota.resets", { countdown: resetCountdown }) }}</time>
  ```

  Style `.top-bar__locale-menu` as an anchored compact popover, and use the existing control dimensions, focus outline, dark-mode colors, and z-index above cards.

- [ ] **Step 4: Run component and full frontend tests**

  Run:

  ```bash
  pnpm test
  pnpm build
  ```

  Expected: all component tests pass and no product-owned untranslated labels remain in rendered output.

- [ ] **Step 5: Commit translated UI controls**

  ```bash
  git add src/App.vue src/components src/styles.css src/__tests__/TopBar.spec.ts src/__tests__/SessionCard.spec.ts src/__tests__/FooterStatus.spec.ts src/__tests__/InitializationStatusRow.spec.ts
  git commit -m "feat: localize Pulse controls and copy"
  ```

### Task 4: Animate footer expansion and align polling cadence

**Files:**
- Modify: `src/App.vue`
- Modify: `src/styles.css`
- Test: `src/__tests__/App.spec.ts`
- Test: `src/__tests__/footerLayout.spec.ts`

**Interfaces:**
- `footer-stack--with-event` is present exactly while `showBackgroundInitialization` is true.
- The WebView refresh interval is `60_000` milliseconds.
- `footer-status-enter-*` and `footer-status-leave-*` animate only the one-line event; the footer surface animates `max-height` from 48px to 72px with bottom anchoring.

- [ ] **Step 1: Write failing tests for cadence and animated state**

  ```ts
  it("uses a 60-second snapshot fallback", () => {
    expect(readFileSync(resolve(process.cwd(), "src/App.vue"), "utf8")).toContain("setInterval(() => { void pulse.load(); }, 60_000)");
  });

  it("uses a stretchable event footer state", () => {
    const stylesheet = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");
    expect(stylesheet).toContain(".footer-stack--with-event");
    expect(stylesheet).toContain("max-height: 72px");
    expect(stylesheet).toContain(".footer-status-enter-from");
  });

  it("uses a localizable list-end label rather than CSS-generated text", () => {
    const stylesheet = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");
    expect(stylesheet).toContain(".session-list__end");
    expect(stylesheet).not.toContain('content: "END"');
  });
  ```

- [ ] **Step 2: Run tests to verify they fail**

  Run:

  ```bash
  pnpm test -- src/__tests__/App.spec.ts src/__tests__/footerLayout.spec.ts
  ```

  Expected: the source still uses a 2-second frontend fallback and lacks stretch-transition selectors.

- [ ] **Step 3: Implement bottom-anchored stretch motion**

  Change App's snapshot interval to 60 seconds. Apply the state class and transition wrapper:

  ```vue
  <div class="footer-stack" :class="{ 'footer-stack--with-event': showBackgroundInitialization }">
    <Transition name="footer-status">
      <InitializationStatusRow v-if="showBackgroundInitialization" :initialization="pulse.snapshot.value.initialization" />
    </Transition>
    <FooterStatus ... />
  </div>
  ```

  Add CSS that keeps `bottom: 12px`, clips expansion with `overflow: hidden`, transitions `max-height` and padding, uses 48px quota-only and 72px event maximums, and has `footer-status-enter/leave` opacity plus vertical transform. Extend the existing reduced-motion media rule to remove both transition and transform.

- [ ] **Step 4: Run focused animation tests and the frontend build**

  Run:

  ```bash
  pnpm test -- src/__tests__/App.spec.ts src/__tests__/footerLayout.spec.ts
  pnpm build
  ```

  Expected: 60-second cadence and footer stretch contracts pass.

- [ ] **Step 5: Commit footer motion and cadence**

  ```bash
  git add src/App.vue src/styles.css src/__tests__/App.spec.ts src/__tests__/footerLayout.spec.ts
  git commit -m "feat: animate footer refresh status"
  ```

### Task 5: Full verification and installed-app proof

**Files:**
- Modify: `docs/superpowers/specs/2026-07-17-localization-refresh-activation-design.md`
- Modify: `docs/superpowers/plans/2026-07-17-localization-refresh-activation.md`

- [ ] **Step 1: Verify all automated checks**

  Run:

  ```bash
  cargo fmt --manifest-path src-tauri/Cargo.toml --check
  cargo test --manifest-path src-tauri/Cargo.toml
  pnpm test
  pnpm build
  git diff --check
  ```

  Expected: Rust and frontend suites pass, production build succeeds, and no whitespace errors remain.

- [ ] **Step 2: Build, install, and inspect the native app**

  Run:

  ```bash
  pnpm tauri build --debug
  pkill -x CodexPulse || :
  ditto 'src-tauri/target/debug/bundle/macos/Codex Pulse.app' '/Applications/Codex Pulse.app'
  open -a 'Codex Pulse'
  ```

  Verify by resizing the native window that the footer grows from its bottom edge when a background event arrives, every explicit language and System fallback update all static UI copy, the language menu is keyboard reachable, hook refreshes remain immediate, and clicking a task retains the existing Codex task hand-off.

- [ ] **Step 3: Update evidence and commit verification documentation**

  Replace any stale 15-second/2-second wording with 60-second behavior, record the retained deep-link-only task-opening boundary, then run:

  ```bash
  git add docs/superpowers/specs/2026-07-17-localization-refresh-activation-design.md docs/superpowers/plans/2026-07-17-localization-refresh-activation.md
  git commit -m "docs: verify localized Pulse behavior"
  git status --short
  ```

  Expected: the worktree is clean.
