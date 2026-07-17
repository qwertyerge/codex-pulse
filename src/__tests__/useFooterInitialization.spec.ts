import { nextTick, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FOOTER_STATUS_THROTTLE_MS, useFooterInitialization } from "../composables/useFooterInitialization";
import type { InitializationSnapshot } from "../types";

function snapshot(runId: number, phase: InitializationSnapshot["phase"]): InitializationSnapshot {
  return {
    runId,
    phase,
    events: [{ runId, sequence: 1, occurredAtMs: runId, phase, summary: phase }]
  };
}

describe("useFooterInitialization", () => {
  afterEach(() => vi.useRealTimers());

  it("shows only a terminal event for a continuous background refresh", async () => {
    vi.useFakeTimers();
    const initialization = ref(snapshot(1, "complete"));
    const footer = useFooterInitialization(initialization);
    await nextTick();

    initialization.value = snapshot(2, "starting");
    await nextTick();
    await vi.advanceTimersByTimeAsync(300);
    initialization.value = snapshot(2, "readingQuota");
    await nextTick();
    await vi.advanceTimersByTimeAsync(300);

    expect(footer.visible.value).toBe(false);

    initialization.value = snapshot(2, "complete");
    await nextTick();

    expect(footer.visible.value).toBe(true);
    expect(footer.initialization.value?.phase).toBe("complete");
    footer.stop();
  });

  it("exposes a stalled progress event after 600 milliseconds", async () => {
    vi.useFakeTimers();
    const initialization = ref(snapshot(1, "complete"));
    const footer = useFooterInitialization(initialization);
    await nextTick();

    initialization.value = snapshot(2, "reconcilingSessions");
    await nextTick();
    await vi.advanceTimersByTimeAsync(FOOTER_STATUS_THROTTLE_MS - 1);
    expect(footer.visible.value).toBe(false);

    await vi.advanceTimersByTimeAsync(1);
    expect(footer.visible.value).toBe(true);
    expect(footer.initialization.value?.phase).toBe("reconcilingSessions");
    footer.stop();
  });
});
