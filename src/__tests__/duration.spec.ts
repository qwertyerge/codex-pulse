import { describe, expect, it } from "vitest";
import { formatDuration, formatRecentAge } from "../lib/duration";

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
  });
});
