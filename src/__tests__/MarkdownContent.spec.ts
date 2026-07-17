import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import MarkdownContent from "../components/MarkdownContent.vue";

describe("MarkdownContent", () => {
  beforeEach(() => invoke.mockReset());

  it("renders Markdown but removes unsafe HTML", () => {
    const wrapper = mount(MarkdownContent, {
      props: {
        source: "**safe**\n\n<script>alert(1)</script><img src=\"https://tracker.example/pixel\"><span>raw</span>\n\n[link](https://example.com)\n\n[bad](javascript:alert(1))"
      }
    });

    expect(wrapper.html()).toContain("<strong>safe</strong>");
    expect(wrapper.html()).toContain('href="https://example.com"');
    expect(wrapper.html()).not.toContain("<script");
    expect(wrapper.html()).not.toContain("<img");
    expect(wrapper.html()).not.toContain("<span>raw</span>");
    expect(wrapper.html()).not.toContain("javascript:");
  });

  it("hands normal links and image placeholders to the operating system", async () => {
    const wrapper = mount(MarkdownContent, {
      props: { source: "[reference](https://example.com/docs) and ![Architecture](https://example.com/diagram.png \"System diagram\")" }
    });

    expect(wrapper.find("img").exists()).toBe(false);
    const image = wrapper.get(".markdown-image-placeholder");
    expect(image.text()).toContain("System diagram");
    await wrapper.get('a[href="https://example.com/docs"]').trigger("click");
    await image.trigger("click");

    expect(invoke).toHaveBeenNthCalledWith(1, "open_external_url", { url: "https://example.com/docs" });
    expect(invoke).toHaveBeenNthCalledWith(2, "open_external_url", { url: "https://example.com/diagram.png" });
  });
});
