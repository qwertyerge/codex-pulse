import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import TopBar from "../components/TopBar.vue";

describe("TopBar", () => {
  it("shows active count and emits a Pin to Top request", async () => {
    const wrapper = mount(TopBar, { props: { activeCount: 2, alwaysOnTop: false } });

    expect(wrapper.text()).toContain("2 active");
    const button = wrapper.get("button");
    expect(button.text()).toContain("Pin to Top");
    await button.trigger("click");
    expect(wrapper.emitted("toggle-pin")).toHaveLength(1);
  });

  it("uses Unpin when the window is already pinned", () => {
    const wrapper = mount(TopBar, { props: { activeCount: 1, alwaysOnTop: true } });
    expect(wrapper.get("button").text()).toContain("Unpin");
  });
});
