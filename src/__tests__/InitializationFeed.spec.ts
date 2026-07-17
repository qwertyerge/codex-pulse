import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import InitializationFeed from "../components/InitializationFeed.vue";
import { i18n } from "../i18n";

describe("InitializationFeed", () => {
  afterEach(() => vi.useRealTimers());

  it("plays a snapshot of initialization events in sequence", async () => {
    vi.useFakeTimers();
    const wrapper = mount(InitializationFeed, {
      props: {
        initialization: {
          runId: 4,
          phase: "complete",
          events: [
            { runId: 4, sequence: 1, occurredAtMs: 1, phase: "starting", summary: "Starting" },
            { runId: 4, sequence: 2, occurredAtMs: 2, phase: "readingQuota", summary: "Reading quota" }
          ]
        }
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.findAll("li")).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(160);
    expect(wrapper.findAll(".initialization-feed__summary").map((event) => event.text())).toEqual(["Starting refresh", "Reading weekly quota"]);
  });

  it("marks the latest in-flight line as a live log with looping dots", async () => {
    vi.useFakeTimers();
    const wrapper = mount(InitializationFeed, {
      props: {
        initialization: {
          runId: 4,
          phase: "readingQuota",
          events: [
            { runId: 4, sequence: 1, occurredAtMs: 1, phase: "readingQuota", summary: "Reading quota" }
          ]
        }
      },
      global: { plugins: [i18n] }
    });

    await vi.advanceTimersByTimeAsync(1);
    expect(wrapper.attributes("role")).toBe("log");
    expect(wrapper.find(".initialization-feed__cursor").exists()).toBe(false);
    expect(wrapper.get(".initialization-feed__ellipsis").text()).toBe("......");
  });
});
