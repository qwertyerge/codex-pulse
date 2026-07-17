import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";
import TopBar from "../components/TopBar.vue";
import { i18n } from "../i18n";

function mountTopBar(props: { activeCount: number; alwaysOnTop: boolean; theme: "system" | "light" | "dark"; locale: "system" | "zh-CN" | "en" | "fr" | "de" }) {
  return mount(TopBar, { props, global: { plugins: [i18n] } });
}

describe("TopBar", () => {
  beforeEach(() => { i18n.global.locale.value = "en"; });

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
});
