import { afterEach, describe, expect, it, vi } from "vitest";
import { useMonotonicClock } from "../composables/useMonotonicClock";

describe("useMonotonicClock", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("catches up to wall time on the first tick after sleep", async () => {
    vi.useFakeTimers();
    const startedAt = Date.parse("2026-07-22T00:00:00.000Z");
    const wakeAt = startedAt + 6 * 60 * 60 * 1_000;
    vi.setSystemTime(startedAt);
    vi.spyOn(performance, "now").mockReturnValue(1_000);
    const clock = useMonotonicClock();

    clock.start();
    vi.setSystemTime(wakeAt);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(clock.nowMs.value).toBe(wakeAt + 1_000);
    clock.stop();
  });

  it("does not move backward when wall time is adjusted backward", async () => {
    vi.useFakeTimers();
    const startedAt = Date.parse("2026-07-22T06:00:00.000Z");
    vi.setSystemTime(startedAt);
    vi.spyOn(performance, "now").mockReturnValue(1_000);
    const clock = useMonotonicClock();

    clock.start();
    vi.setSystemTime(startedAt - 60 * 60 * 1_000);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(clock.nowMs.value).toBe(startedAt);
    clock.stop();
  });
});
