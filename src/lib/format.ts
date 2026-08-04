/** "0s", "45s", "1m 05s", "1h 02m" — for idle counters and countdowns. */
export function formatSeconds(total: number): string {
  const s = Math.max(0, Math.floor(total));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const rest = s % 60;
  if (m < 60) return `${m}m ${String(rest).padStart(2, "0")}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${String(m % 60).padStart(2, "0")}m`;
}

/** Clock time ("14:32:05") from a unix-ms timestamp, or a dash when absent. */
export function formatClock(unixMs: number | null): string {
  if (unixMs === null) return "—";
  return new Date(unixMs).toLocaleTimeString();
}
