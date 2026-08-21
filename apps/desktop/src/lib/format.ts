/** Milliseconds in one minute. Use instead of inlining `60_000` / `60 * 1000`. */
export const MS_PER_MINUTE = 60_000;
/** Milliseconds in one hour. Use instead of inlining `3_600_000` / `60 * 60 * 1000`. */
export const MS_PER_HOUR = 60 * MS_PER_MINUTE;
/** Milliseconds in one day. Use instead of inlining `86_400_000` / `24 * 60 * 60 * 1000`. */
export const MS_PER_DAY = 24 * MS_PER_HOUR;

/** Format Date, ISO string, or millisecond epochs against a caller-provided clock. */
type RelativeTimeStyle = "compact" | "verbose";

export function formatRelativeTime(
  value: Date | string | number,
  nowMs: number,
  style: RelativeTimeStyle = "compact",
): string {
  const t =
    value instanceof Date ? value.getTime() : typeof value === "string" ? Date.parse(value) : value;
  if (!Number.isFinite(t)) return "unknown";
  const diff = Math.max(0, nowMs - t);

  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;

  const days = Math.floor(diff / 86_400_000);
  if (days < 30) {
    if (style === "verbose" && days === 1) return "yesterday";
    return `${days}d ago`;
  }

  const months = Math.floor(days / 30);
  if (months < 12) {
    if (style === "verbose") return months === 1 ? "1 month ago" : `${months} months ago`;
    return `${months}mo ago`;
  }

  const years = Math.floor(months / 12);
  if (style === "verbose") return years === 1 ? "1 year ago" : `${years} years ago`;
  return `${years}y ago`;
}
