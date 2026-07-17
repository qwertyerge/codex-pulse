export const RECENT_AGE_WIDTH_SAMPLES = ["1s", "10s", "1m", "10m", "1h", "10h", "1d", "10d", "99d+"] as const;

export function measureRecentAgeWidth(element: HTMLElement): number {
  const widths = Array.from(element.children, (sample) => sample.getBoundingClientRect().width);
  return widths.length ? Math.ceil(Math.max(...widths)) : 0;
}
