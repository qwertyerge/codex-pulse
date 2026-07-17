import { nextTick, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  FOOTER_STATUS_HIDE_MS,
  FOOTER_STATUS_THROTTLE_MS,
  useFooterInitialization
} from "../composables/useFooterInitialization";
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

  it("waits two seconds from the latest hidden-state update before showing it", async () => {
    vi.useFakeTimers();
    const initialization = ref(snapshot(1, "complete"));
    const footer = useFooterInitialization(initialization);
    await nextTick();

    initialization.value = snapshot(2, "starting");
    await nextTick();
    await vi.advanceTimersByTimeAsync(1_000);
    initialization.value = snapshot(2, "readingQuota");
    await nextTick();
    await vi.advanceTimersByTimeAsync(1_999);

    expect(footer.visible.value).toBe(false);

    await vi.advanceTimersByTimeAsync(1);
    expect(footer.visible.value).toBe(true);
    expect(footer.initialization.value?.phase).toBe("readingQuota");
    expect(FOOTER_STATUS_THROTTLE_MS).toBe(2_000);
    expect(FOOTER_STATUS_HIDE_MS).toBe(2_000);
    footer.stop();
  });

  it("updates the mounted row in place and restarts the terminal leave delay", async () => {
    vi.useFakeTimers();
    const initialization = ref(snapshot(1, "complete"));
    const footer = useFooterInitialization(initialization);
    await nextTick();

    initialization.value = snapshot(2, "complete");
    await nextTick();
    await vi.advanceTimersByTimeAsync(FOOTER_STATUS_THROTTLE_MS);
    expect(footer.visible.value).toBe(true);

    initialization.value = snapshot(3, "starting");
    await nextTick();
    expect(footer.visible.value).toBe(true);
    expect(footer.initialization.value?.runId).toBe(3);
    expect(footer.initialization.value?.phase).toBe("starting");

    await vi.advanceTimersByTimeAsync(FOOTER_STATUS_HIDE_MS);
    expect(footer.visible.value).toBe(true);

    initialization.value = snapshot(3, "complete");
    await nextTick();
    await vi.advanceTimersByTimeAsync(FOOTER_STATUS_HIDE_MS - 1);
    expect(footer.visible.value).toBe(true);

    await vi.advanceTimersByTimeAsync(1);
    expect(footer.visible.value).toBe(false);
    footer.stop();
  });

  it("restarts the terminal leave delay when another terminal snapshot arrives", async () => {
    vi.useFakeTimers();
    const initialization = ref(snapshot(1, "complete"));
    const footer = useFooterInitialization(initialization);
    await nextTick();

    initialization.value = snapshot(2, "complete");
    await nextTick();
    await vi.advanceTimersByTimeAsync(FOOTER_STATUS_THROTTLE_MS);
    expect(footer.visible.value).toBe(true);

    await vi.advanceTimersByTimeAsync(FOOTER_STATUS_HIDE_MS - 1);
    initialization.value = snapshot(3, "complete");
    await nextTick();
    expect(footer.visible.value).toBe(true);
    expect(footer.initialization.value?.runId).toBe(3);

    await vi.advanceTimersByTimeAsync(FOOTER_STATUS_HIDE_MS - 1);
    expect(footer.visible.value).toBe(true);
    await vi.advanceTimersByTimeAsync(1);
    expect(footer.visible.value).toBe(false);
    footer.stop();
  });
});
