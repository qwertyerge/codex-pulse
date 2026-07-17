export function formatDuration(milliseconds: number): string {
  const seconds = Math.floor(Math.max(0, milliseconds) / 1_000);
  const hours = Math.floor(seconds / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  const remainder = seconds % 60;
  const twoDigits = (value: number) => String(value).padStart(2, "0");

  return hours > 0
    ? `${hours}:${twoDigits(minutes)}:${twoDigits(remainder)}`
    : `${twoDigits(minutes)}:${twoDigits(remainder)}`;
}

export function formatRecentAge(milliseconds: number): string {
  return `${formatRecentAgeValue(milliseconds)} ago`;
}

export function formatRecentAgeValue(milliseconds: number): string {
  const seconds = Math.floor(Math.max(0, milliseconds) / 1_000);
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3_600)}h`;
  const days = Math.floor(seconds / 86_400);
  return `${Math.min(99, days)}${days >= 99 ? "d+" : "d"}`;
}

export function formatQuotaReset(milliseconds: number): string {
  const minutes = Math.floor(Math.max(0, milliseconds) / 60_000);
  const days = Math.floor(minutes / 1_440);
  const hours = Math.floor((minutes % 1_440) / 60);

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  return `${minutes}m`;
}
