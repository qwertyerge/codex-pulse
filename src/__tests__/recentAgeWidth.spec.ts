import { describe, expect, it, vi } from "vitest";
import { measureRecentAgeWidth } from "../lib/recentAgeWidth";

describe("measureRecentAgeWidth", () => {
  it("uses the actual laid-out sample widths instead of an approximated canvas font", () => {
    const first = document.createElement("span");
    const second = document.createElement("span");
    const samples = document.createElement("span");
    samples.append(first, second);
    vi.spyOn(first, "getBoundingClientRect").mockReturnValue({ width: 31 } as DOMRect);
    vi.spyOn(second, "getBoundingClientRect").mockReturnValue({ width: 47 } as DOMRect);

    expect(measureRecentAgeWidth(samples)).toBe(47);
  });
});
