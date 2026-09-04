import type { ScanCategory } from "./types";
import { CATEGORY_META } from "./category-meta";

export const CATEGORY_ORDER: ScanCategory[] = [
  "security",
  "performance",
  "seo",
  "accessibility",
  "compliance",
  "config",
  "polish",
];

export const CATEGORY_LABELS: Record<ScanCategory, string> = {
  security: CATEGORY_META.security.label,
  performance: CATEGORY_META.performance.label,
  seo: CATEGORY_META.seo.label,
  accessibility: CATEGORY_META.accessibility.label,
  compliance: CATEGORY_META.compliance.label,
  config: CATEGORY_META.config.label,
  polish: CATEGORY_META.polish.label,
};

/**
 * Compact category names for dense surfaces (stage strips, action buttons)
 * where the full label would crowd its neighbours.
 */
export const CATEGORY_SHORT_LABELS: Record<ScanCategory, string> = {
  security: CATEGORY_META.security.shortLabel,
  performance: CATEGORY_META.performance.shortLabel,
  seo: CATEGORY_META.seo.shortLabel,
  accessibility: CATEGORY_META.accessibility.shortLabel,
  compliance: CATEGORY_META.compliance.shortLabel,
  config: CATEGORY_META.config.shortLabel,
  polish: CATEGORY_META.polish.shortLabel,
};

/** Category text-token classes. */
export const CATEGORY_TEXT: Partial<Record<ScanCategory, string>> = {
  security: "text-cat-security",
  performance: "text-cat-performance",
  seo: "text-cat-seo",
  accessibility: "text-cat-accessibility",
  compliance: "text-cat-compliance",
  config: "text-cat-config",
  polish: "text-cat-polish",
};

/** CSS var reference for SVGs and inline styles */
export const CATEGORY_CSS_VAR: Partial<Record<ScanCategory, string>> = {
  security: "var(--cat-security)",
  performance: "var(--cat-performance)",
  seo: "var(--cat-seo)",
  accessibility: "var(--cat-accessibility)",
  compliance: "var(--cat-compliance)",
  config: "var(--cat-config)",
  polish: "var(--cat-polish)",
};

// Rust owns scoring; this module retains presentation ordering only.

// Severity ordering helpers live in `./severity`. Import from there:
//   import { compareSeverity, severityRank } from "@/lib/severity";

// Score-band helpers live in `./score` - the single source of truth.
// Re-exported here so existing `@/lib/tokens` import sites keep working.
export { SCORE_CSS_VAR, getScoreCssVar } from "./score";

export function formatNum(n: number | null | undefined): string {
  if (n == null) return "0";
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "K";
  return n.toString();
}

export function formatBytes(bytes: number | null | undefined): string {
  if (!bytes) return "0 B";
  if (bytes >= 1_073_741_824) return (bytes / 1_073_741_824).toFixed(1) + " GB";
  if (bytes >= 1_048_576) return (bytes / 1_048_576).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
  return bytes + " B";
}

export { formatRelativeTime } from "./format";

export function formatDuration(seconds: number | null | undefined): string {
  if (!seconds) return "0s";
  if (seconds >= 3600) {
    const h = Math.floor(seconds / 3600);
    const m = Math.round((seconds % 3600) / 60);
    return m > 0 ? `${h}h ${m}m` : `${h}h`;
  }
  if (seconds >= 60) {
    const m = Math.floor(seconds / 60);
    const s = Math.round(seconds % 60);
    return s > 0 ? `${m}m ${s}s` : `${m}m`;
  }
  return `${Math.round(seconds)}s`;
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  return (
    d.toLocaleDateString(undefined, { month: "short", day: "numeric" }) +
    " at " +
    d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
  );
}

export function formatCheckName(id: string): string {
  return id
    .replace(/\./g, " ")
    .replace(/_/g, " ")
    .replace(/-/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .replace(/Ssl/g, "SSL")
    .replace(/Https/g, "HTTPS")
    .replace(/Seo/g, "SEO")
    .replace(/Hsts/g, "HSTS")
    .replace(/Csp/g, "CSP")
    .replace(/Dns/g, "DNS")
    .replace(/Http /g, "HTTP ")
    .replace(/Wcag/g, "WCAG");
}
