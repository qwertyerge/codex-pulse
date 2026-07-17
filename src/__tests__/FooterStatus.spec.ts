import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import FooterStatus from "../components/FooterStatus.vue";

describe("FooterStatus", () => {
  it("shows used, remaining, reset countdown, and progress semantics", () => {
    const wrapper = mount(FooterStatus, {
      props: {
        nowMs: 1_000_000,
        activeSessionCount: 1,
        quota: {
          usedPercent: 81,
          remainingPercent: 19,
          resetsAtMs: 1_000_000 + 2 * 86_400_000 + 4 * 3_600_000
        }
      }
    });

    expect(wrapper.text()).toContain("周额度");
    expect(wrapper.text()).toContain("已用 81% · 剩余 19%");
    expect(wrapper.text()).toContain("2d 4h 后重置");
    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuenow")).toBe("81");
    expect(wrapper.get(".quota-footer__progress-fill").attributes("style")).toContain("81%");
    expect(wrapper.get(".quota-footer__remaining-value").text()).toBe("19%");
  });

  it("honestly renders unavailable when there is no local quota observation", () => {
    const wrapper = mount(FooterStatus, { props: { nowMs: 1_000_000, activeSessionCount: 0 } });

    expect(wrapper.text()).toContain("周额度 · 暂不可用");
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false);
  });

  it("becomes unavailable as soon as the weekly reset has passed", () => {
    const wrapper = mount(FooterStatus, {
      props: {
        nowMs: 1_000_000,
        activeSessionCount: 1,
        quota: { usedPercent: 81, remainingPercent: 19, resetsAtMs: 999_999 }
      }
    });

    expect(wrapper.text()).toContain("周额度 · 暂不可用");
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false);
  });

  it("downgrades a cached quota while there are no active sessions", () => {
    const wrapper = mount(FooterStatus, {
      props: {
        nowMs: 1_000_000,
        activeSessionCount: 0,
        quota: {
          usedPercent: 81,
          remainingPercent: 19,
          resetsAtMs: 1_000_000 + 2 * 86_400_000
        }
      }
    });

    expect(wrapper.classes()).toContain("quota-footer--stale");
    expect(wrapper.text()).toContain("暂无活跃会话；新任务开始后将自动恢复更新");
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false);
  });
});
