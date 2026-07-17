import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import FooterStatus from "../components/FooterStatus.vue";
import { i18n } from "../i18n";

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
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.text()).toContain("Weekly quota");
    expect(wrapper.text()).toContain("Used 81% · Remaining 19%");
    expect(wrapper.text()).toContain("Resets in 2d 4h");
    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuenow")).toBe("81");
    expect(wrapper.get(".quota-footer__progress-fill").attributes("style")).toContain("81%");
    expect(wrapper.get(".quota-footer__remaining-value").text()).toBe("19%");
  });

  it("honestly renders unavailable when there is no local quota observation", () => {
    const wrapper = mount(FooterStatus, { props: { nowMs: 1_000_000, activeSessionCount: 0 }, global: { plugins: [i18n] } });

    expect(wrapper.text()).toContain("Weekly quota · unavailable");
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false);
  });

  it("becomes unavailable as soon as the weekly reset has passed", () => {
    const wrapper = mount(FooterStatus, {
      props: {
        nowMs: 1_000_000,
        activeSessionCount: 1,
        quota: { usedPercent: 81, remainingPercent: 19, resetsAtMs: 999_999 }
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.text()).toContain("Weekly quota · unavailable");
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
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.classes()).toContain("quota-footer--stale");
    expect(wrapper.text()).toContain("No active sessions; updates resume automatically when a task starts");
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false);
  });
});
