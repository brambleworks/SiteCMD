import {
  getPagespeedReport,
  pagespeedApiKeyIsSet as pagespeedApiKeyIsSetCmd,
  setPagespeedApiKey as setPagespeedApiKeyCmd,
} from "@/lib/commands";

export type PageSpeedStrategy = "mobile" | "desktop";

// PageSpeedReport is generated from the Rust struct (ts-rs); the wire is
// camelCase (performanceScore, lcpMs, fieldLcpMs,...).
import type { PageSpeedReport } from "@/generated/ipc-bindings";
export type { PageSpeedReport };

/** Fetch a fresh PageSpeed Insights report for a URL. */
export function fetchPageSpeedReport(
  url: string,
  strategy: PageSpeedStrategy,
): Promise<PageSpeedReport> {
  return getPagespeedReport({ url, strategy });
}

/** Store (or, with an empty string, clear) the optional PSI API key. */
export function setPageSpeedApiKey(key: string): Promise<void> {
  return setPagespeedApiKeyCmd({ key });
}

/** Whether a PageSpeed Insights API key is currently stored. */
export function pageSpeedApiKeyIsSet(): Promise<boolean> {
  return pagespeedApiKeyIsSetCmd();
}

/** Heuristic: does this PSI error look like a rate-limit (429) failure? */
export function isRateLimitError(message: string): boolean {
  return /\b429\b|rate.?limit|too many requests|exhausted|quota/i.test(message);
}

export type VitalRating = "good" | "needs-improvement" | "poor";

export type VitalMetric = "lcp" | "cls" | "inp" | "tbt" | "fcp" | "ttfb" | "si";

/** [good ceiling, needs-improvement ceiling]; above the second is "poor". */
const THRESHOLDS: Record<VitalMetric, [number, number]> = {
  lcp: [2500, 4000],
  cls: [0.1, 0.25],
  inp: [200, 500],
  tbt: [200, 600],
  fcp: [1800, 3000],
  ttfb: [800, 1800],
  si: [3400, 5800],
};

/** Rate a metric value, or null when the value is missing. */
export function rateVital(metric: VitalMetric, value: number | null): VitalRating | null {
  if (value === null) return null;
  const [good, needs] = THRESHOLDS[metric];
  if (value <= good) return "good";
  if (value <= needs) return "needs-improvement";
  return "poor";
}

/** Lighthouse performance score bands (0-100). */
const LIGHTHOUSE_GOOD_MIN = 90;
const LIGHTHOUSE_NEEDS_IMPROVEMENT_MIN = 50;
export function ratePerformanceScore(score: number): VitalRating {
  if (score >= LIGHTHOUSE_GOOD_MIN) return "good";
  if (score >= LIGHTHOUSE_NEEDS_IMPROVEMENT_MIN) return "needs-improvement";
  return "poor";
}

/** Map a rating to an app text-color token class. */
export function ratingColorClass(rating: VitalRating | null): string {
  switch (rating) {
    case "good":
      return "text-score-excellent";
    case "needs-improvement":
      return "text-severity-high";
    case "poor":
      return "text-severity-critical";
    default:
      return "text-muted-foreground";
  }
}
