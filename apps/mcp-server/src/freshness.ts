/** Scan age copy shared by every tool that shows results, so staleness is never silent. */

export const STALE_SCAN_AFTER_DAYS = 7;
const DAY_MS = 24 * 60 * 60 * 1000;

export function describeScanAge(timestamp: string, nowMs: number): string {
  const scannedMs = Date.parse(timestamp);
  if (!Number.isFinite(scannedMs)) return `Scanned ${timestamp}`;
  const days = Math.max(0, Math.floor((nowMs - scannedMs) / DAY_MS));
  const date = new Date(scannedMs).toISOString().slice(0, 10);
  const age = days === 0 ? "today" : days === 1 ? "1 day ago" : `${days} days ago`;
  const stale =
    days >= STALE_SCAN_AFTER_DAYS
      ? `. These results are ${days} days old and may be stale; ask the user to rescan (see request_scan) before fixing`
      : "";
  return `Scanned ${date} (${age})${stale}`;
}
