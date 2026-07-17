import { describe, expect, it } from "vitest";
import { formatDuration, formatQuotaReset, formatRecentAge, formatRecentAgeValue } from "../lib/duration";
import { RECENT_AGE_WIDTH_SAMPLES } from "../lib/recentAgeWidth";

describe("formatDuration", () => {
  it("uses stable tabular-friendly minute and hour formats", () => {
    expect(formatDuration(0)).toBe("00:00");
    expect(formatDuration(59_000)).toBe("00:59");
    expect(formatDuration(60_000)).toBe("01:00");
    expect(formatDuration(3_661_000)).toBe("1:01:01");
  });
});

describe("formatRecentAge", () => {
  it("uses lowercase relative time copy", () => {
    expect(formatRecentAge(18_000)).toBe("18s ago");
    expect(formatRecentAge(120_000)).toBe("2m ago");
    expect(formatRecentAge(3_600_000)).toBe("1h ago");
    expect(formatRecentAge(86_400_000)).toBe("1d ago");
    expect(formatRecentAge(8_640_000_000)).toBe("99d+ ago");
    expect(formatRecentAgeValue(18_000)).toBe("18s");
    expect(formatRecentAgeValue(8_640_000_000)).toBe("99d+");
    expect(RECENT_AGE_WIDTH_SAMPLES).toEqual(["1s", "10s", "1m", "10m", "1h", "10h", "1d", "10d", "99d+"]);
  });
});

describe("formatQuotaReset", () => {
  it("uses compact day, hour, and minute countdown units", () => {
    expect(formatQuotaReset(2 * 86_400_000 + 4 * 3_600_000)).toBe("2d 4h");
    expect(formatQuotaReset(2 * 3_600_000 + 9 * 60_000)).toBe("2h 9m");
    expect(formatQuotaReset(30_000)).toBe("0m");
  });
});
