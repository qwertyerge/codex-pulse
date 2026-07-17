import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import TopBar from "../components/TopBar.vue";

describe("TopBar", () => {
  it("shows active count and emits a Pin to Top request", async () => {
    const wrapper = mount(TopBar, { props: { activeCount: 2, alwaysOnTop: false, theme: "system" } });

    expect(wrapper.text()).toContain("2 active");
    const button = wrapper.get(".top-bar__pin");
    expect(button.attributes("aria-label")).toBe("Pin window to top");
    expect(button.attributes("title")).toBe("Pin window to top");
    await button.trigger("click");
    expect(wrapper.emitted("toggle-pin")).toHaveLength(1);
  });

  it("uses Unpin when the window is already pinned", () => {
    const wrapper = mount(TopBar, { props: { activeCount: 1, alwaysOnTop: true, theme: "dark" } });
    expect(wrapper.get(".top-bar__pin").attributes("aria-label")).toBe("Unpin window");
  });

  it("emits the selected appearance mode from icon controls", async () => {
    const wrapper = mount(TopBar, { props: { activeCount: 1, alwaysOnTop: false, theme: "system" } });

    await wrapper.get('[aria-label="Use dark appearance"]').trigger("click");

    expect(wrapper.emitted("set-theme")?.[0]).toEqual(["dark"]);
  });
});
