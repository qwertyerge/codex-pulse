import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";
import TopBar from "../components/TopBar.vue";
import type { UpdaterState } from "../composables/useUpdater";
import { i18n } from "../i18n";

interface TopBarTestProps {
  activeCount: number;
  alwaysOnTop: boolean;
  theme: "system" | "light" | "dark";
  locale: "system" | "zh-CN" | "en" | "fr" | "de";
  updateState?: UpdaterState;
}

function mountTopBar(props: TopBarTestProps) {
  return mount(TopBar, {
    props: {
      updateState: { phase: "idle" },
      ...props
    },
    global: { plugins: [i18n] }
  });
}

describe("TopBar", () => {
  beforeEach(() => {
    i18n.global.locale.value = "en";
  });

  it("shows active count and emits a Pin to Top request", async () => {
    const wrapper = mountTopBar({ activeCount: 2, alwaysOnTop: false, theme: "system", locale: "en" });

    expect(wrapper.text()).toContain("2 active");
    const button = wrapper.get(".top-bar__pin");
    expect(button.attributes("aria-label")).toBe("Pin window to top");
    expect(button.attributes("title")).toBe("Pin window to top");
    await button.trigger("click");
    expect(wrapper.emitted("toggle-pin")).toHaveLength(1);
  });

  it("uses Unpin when the window is already pinned", () => {
    const wrapper = mountTopBar({ activeCount: 1, alwaysOnTop: true, theme: "dark", locale: "en" });
    expect(wrapper.get(".top-bar__pin").attributes("aria-label")).toBe("Unpin window");
  });

  it("emits the selected appearance mode from icon controls", async () => {
    const wrapper = mountTopBar({ activeCount: 1, alwaysOnTop: false, theme: "system", locale: "en" });

    await wrapper.get('[aria-label="Use dark appearance"]').trigger("click");

    expect(wrapper.emitted("set-theme")?.[0]).toEqual(["dark"]);
  });

  it("opens the language menu and emits French selection", async () => {
    const wrapper = mountTopBar({ activeCount: 2, alwaysOnTop: false, theme: "system", locale: "en" });

    await wrapper.get('[aria-label="Choose language"]').trigger("click");
    expect(wrapper.get('[data-locale="fr"]').text()).toBe("Français");
    await wrapper.get('[data-locale="fr"]').trigger("click");

    expect(wrapper.emitted("set-locale")?.[0]).toEqual(["fr"]);
    expect(wrapper.find('[role="menu"]').exists()).toBe(false);
  });

  it("keeps the active count when updater activity is not visible", () => {
    const wrapper = mountTopBar({
      activeCount: 3,
      alwaysOnTop: false,
      theme: "system",
      locale: "en",
      updateState: { phase: "checking" }
    });

    expect(wrapper.text()).toContain("3 active");
    expect(wrapper.find(".top-bar__update").exists()).toBe(false);
  });

  it.each([
    [
      {
        phase: "downloading",
        version: "0.4.0",
        downloaded: 42,
        total: 100,
        percent: 42
      },
      "Update 42%",
      true
    ],
    [
      {
        phase: "downloading",
        version: "0.4.0",
        downloaded: 42
      },
      "Updating",
      true
    ],
    [{ phase: "ready", version: "0.4.0" }, "Update", false],
    [{ phase: "installing", version: "0.4.0" }, "Updating", true],
    [{ phase: "failed", stage: "download" }, "Update failed", false]
  ] as const)(
    "renders updater state %o as %s",
    (updateState, label, disabled) => {
      const wrapper = mountTopBar({
        activeCount: 3,
        alwaysOnTop: false,
        theme: "system",
        locale: "en",
        updateState
      });
      const badge = wrapper.get(".top-bar__update");

      expect(wrapper.find(".top-bar__count").exists()).toBe(false);
      expect(badge.text()).toBe(label);
      expect(badge.attributes("aria-live")).toBe("polite");
      expect(badge.attributes("disabled") !== undefined).toBe(disabled);
    }
  );

  it("emits activation only from an enabled update badge", async () => {
    const wrapper = mountTopBar({
      activeCount: 1,
      alwaysOnTop: false,
      theme: "system",
      locale: "en",
      updateState: { phase: "ready", version: "0.4.0" }
    });

    const badge = wrapper.get(".top-bar__update");
    expect(badge.attributes("title")).toBe("Install version 0.4.0");
    expect(badge.attributes("aria-label")).toBe("Install version 0.4.0");
    await badge.trigger("click");

    expect(wrapper.emitted("activate-update")).toHaveLength(1);
  });
});
