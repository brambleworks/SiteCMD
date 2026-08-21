/** Canonical severity ordering, labels, comparisons, and tones. */

export const SEVERITIES = ["critical", "high", "medium", "low"] as const;

export type Severity = (typeof SEVERITIES)[number];
export type SeverityCounts = Record<Severity, number>;

const RANK: Record<Severity, number> = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
};

export function isSeverity(value?: string | null): value is Severity {
  return (SEVERITIES as readonly string[]).includes(value ?? "");
}

export function createSeverityCounts(overrides: Partial<SeverityCounts> = {}): SeverityCounts {
  return {
    critical: overrides.critical ?? 0,
    high: overrides.high ?? 0,
    medium: overrides.medium ?? 0,
    low: overrides.low ?? 0,
  };
}

export function addSeverityCounts(left: SeverityCounts, right: SeverityCounts): SeverityCounts {
  return {
    critical: left.critical + right.critical,
    high: left.high + right.high,
    medium: left.medium + right.medium,
    low: left.low + right.low,
  };
}

export function severityCountTotal(
  counts: SeverityCounts,
  severities: readonly Severity[] = SEVERITIES,
): number {
  return severities.reduce((sum, severity) => sum + counts[severity], 0);
}

/** Numeric rank where lower values are more severe. */
export function severityRank(s: Severity): number {
  return RANK[s];
}

/** Drop-in `(a, b) => number` comparator that sorts critical first, low last. */
export function compareSeverity(a: Severity, b: Severity): number {
  return RANK[a] - RANK[b];
}

/** Human-friendly label, e.g. "Critical", "High". */
export function severityLabel(s: Severity): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export function formatSeverityLabel(value: string): string {
  return isSeverity(value) ? severityLabel(value) : value;
}

/** Canonical token-backed text class for an issue severity. */
export function severityToneClass(s: Severity): string {
  switch (s) {
    case "critical":
      return "text-severity-critical";
    case "high":
      return "text-severity-high";
    case "medium":
      return "text-severity-medium";
    case "low":
      return "text-severity-low";
  }
}

/** Severity tone with a muted fallback for unknown wire values. */
export function formatSeverityToneClass(value: string): string {
  return isSeverity(value) ? severityToneClass(value) : "text-muted-foreground";
}

/** Return the severity color token for SVG fills and border accents. */
export function severityCssVar(s: Severity): string {
  switch (s) {
    case "critical":
      return "var(--severity-critical)";
    case "high":
      return "var(--severity-high)";
    case "medium":
      return "var(--severity-medium)";
    case "low":
      return "var(--severity-low)";
  }
}
