import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined)
}));

import App from "../App.vue";

describe("App", () => {
  it("renders the product name", () => {
    const wrapper = mount(App);
    expect(wrapper.text()).toContain("Codex Pulse");
    expect(wrapper.text()).toContain("Loading active Codex sessions");
  });
});
