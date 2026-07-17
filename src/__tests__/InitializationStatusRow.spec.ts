import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import InitializationStatusRow from "../components/InitializationStatusRow.vue";
import { i18n } from "../i18n";

describe("InitializationStatusRow", () => {
  it("renders only the latest background reconciliation event in one footer row", () => {
    const wrapper = mount(InitializationStatusRow, {
      props: {
        initialization: {
          runId: 2,
          phase: "reconcilingSessions",
          events: [
            { runId: 2, sequence: 1, occurredAtMs: 1, phase: "starting", summary: "Starting" },
            { runId: 2, sequence: 2, occurredAtMs: 2, phase: "reconcilingSessions", summary: "Reconciling active sessions" }
          ]
        }
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.findAll(".initialization-status-row")).toHaveLength(1);
    expect(wrapper.text()).toContain("Reconciling active Codex sessions");
    expect(wrapper.text()).not.toContain("Starting");
  });

  it("keeps an unknown failed summary verbatim", () => {
    const wrapper = mount(InitializationStatusRow, {
      props: {
        initialization: {
          runId: 2,
          phase: "failed",
          events: [{ runId: 2, sequence: 1, occurredAtMs: 1, phase: "failed", summary: "Reconciliation failed: disk unavailable" }]
        }
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.text()).toContain("Reconciliation failed: disk unavailable");
  });
});
