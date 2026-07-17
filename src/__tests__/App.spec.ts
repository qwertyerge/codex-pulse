import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined)
}));

import App from "../App.vue";
import { i18n } from "../i18n";

describe("App", () => {
  it("renders the product name", () => {
    const wrapper = mount(App, { global: { plugins: [i18n] } });
    expect(wrapper.text()).toContain("Codex Pulse");
    expect(wrapper.text()).toContain("Loading active Codex sessions");
    const footer = wrapper.get(".quota-footer");
    expect(footer.element.parentElement?.classList.contains("session-list")).toBe(false);
    expect(footer.element.parentElement?.classList.contains("footer-stack")).toBe(true);
  });
});
