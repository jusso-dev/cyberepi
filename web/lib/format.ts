export function formatCount(value: number): string {
  return Math.round(value).toLocaleString("en-US");
}

export function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

/** Rt at or above 1 means the simulated outbreak is still expanding. */
export function reproductionPhase(rt: number): "expanding" | "contracting" {
  return rt >= 1 ? "expanding" : "contracting";
}

export function formatHours(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "n/a";
  const total = Math.round(seconds);
  if (total > 0 && total % 86400 === 0) {
    const days = total / 86400;
    return days === 1 ? "1 day" : `${days} days`;
  }
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}
