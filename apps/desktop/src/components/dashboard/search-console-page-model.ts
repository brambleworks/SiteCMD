import type { CategoryScore, CheckResult } from "@/lib/types";
import type { SearchConsoleData } from "@/lib/analytics-types";
import { formatCheckStatus } from "@/lib/issues";
import { severityRank } from "@/lib/severity";
import { scoreIssueImpact } from "@/lib/sitecmd-score";
import { inferSeoFocusFromText, matchesSeoFocusText } from "@/lib/seo-focus";

export type Period = "7d" | "28d" | "3mo" | "6mo" | "12mo" | "16mo";

export const PERIODS: { value: Period; label: string }[] = [
  { value: "7d", label: "7 days" },
  { value: "28d", label: "28 days" },
  { value: "3mo", label: "3 months" },
  { value: "6mo", label: "6 months" },
  { value: "12mo", label: "12 months" },
  { value: "16mo", label: "16 months" },
];

type SeoCoverageStatus = "covered" | "needs-work" | "not-checked";
type GscObservationTone = "critical" | "warning" | "info";

interface SeoCoverageGroup {
  id: string;
  label: string;
  description: string;
  focus: string | null;
  issues: CheckResult[];
  passed: CheckResult[];
  total: number;
  status: SeoCoverageStatus;
}

export interface GscObservation {
  id: string;
  label: string;
  detail: string;
  metric: string;
  tone: GscObservationTone;
}

export interface SearchTrendSummary {
  label: string;
  detail: string;
  tone: "up" | "down" | "flat" | "empty";
  deltaLabel: string;
}

const SEO_COVERAGE_DEFS = [
  {
    id: "crawl",
    label: "Crawl Access",
    description: "Robots, noindex, status codes, and anything that can stop crawlers.",
    focus: "seo.robots",
    terms: ["robots", "robots_txt", "noindex", "indexability", "crawl", "blocked", "status"],
  },
  {
    id: "discovery",
    label: "Discovery",
    description: "Sitemaps, canonicals, internal links, broken links, and page discovery.",
    focus: "seo.sitemap",
    terms: ["sitemap", "canonical", "internal", "broken", "link", "url_structure"],
  },
  {
    id: "metadata",
    label: "Metadata",
    description: "Title tags, descriptions, social previews, and duplicate metadata.",
    focus: "seo.titles",
    terms: [
      "title",
      "meta_description",
      "meta-description",
      "description",
      "open_graph",
      "og",
      "twitter",
      "duplicate_meta",
      "meta_conflicts",
      "meta_robots",
    ],
  },
  {
    id: "structured",
    label: "Structured Data",
    description: "Schema, JSON-LD, FAQ, organization, and rich-result eligibility.",
    focus: "seo.structured_data",
    terms: ["structured", "schema", "json_ld", "json-ld", "faq", "organization"],
  },
  {
    id: "content",
    label: "Content Signals",
    description: "Headings, thin content, image alt text, semantic HTML, and freshness.",
    focus: null,
    terms: [
      "heading",
      "h1",
      "thin_content",
      "image_alt",
      "semantic",
      "content_freshness",
      "source_citations",
    ],
  },
  {
    id: "ai-search",
    label: "AI Search Readiness",
    description: "LLMs.txt, AI crawler access, citation metadata, and JS-only content.",
    focus: null,
    terms: ["llms", "ai_crawler", "citation", "js_only", "javascript-only"],
  },
] as const;

export function matchesSeoFocus(issue: CheckResult, focus: string | null | undefined): boolean {
  if (!focus) return false;
  return matchesSeoFocusText(`${issue.checkId} ${issue.title}`, focus);
}

export function getSingleFocusedSeoIssueId(
  issues: CheckResult[],
  focus: string | null | undefined,
): string | null {
  if (!focus) return null;
  const matches = issues.filter((issue) => matchesSeoFocus(issue, focus));
  return matches.length === 1 ? (matches[0]?.checkId ?? null) : null;
}

function sortSeoIssuesForAction(issues: CheckResult[]): CheckResult[] {
  return [...issues].sort((a, b) => {
    const severityDelta = severityRank(a.severity) - severityRank(b.severity);
    if (severityDelta !== 0) return severityDelta;
    return a.title.localeCompare(b.title);
  });
}

export function buildSeoCoverageGroups(
  issues: CheckResult[],
  passedChecks: CheckResult[],
): SeoCoverageGroup[] {
  const allChecks = [...issues, ...passedChecks];

  return SEO_COVERAGE_DEFS.map((group) => {
    const groupChecks = allChecks.filter((check) => matchesCoverageTerms(check, group.terms));
    const groupIssues = sortSeoIssuesForAction(
      issues.filter((check) => matchesCoverageTerms(check, group.terms)),
    );
    const groupPassed = passedChecks.filter((check) => matchesCoverageTerms(check, group.terms));

    return {
      id: group.id,
      label: group.label,
      description: group.description,
      focus: group.focus,
      issues: groupIssues,
      passed: groupPassed,
      total: groupChecks.length,
      status:
        groupChecks.length === 0
          ? "not-checked"
          : groupIssues.length > 0
            ? "needs-work"
            : "covered",
    };
  });
}

export function buildSearchTrendSummary(
  data: SearchConsoleData | null | undefined,
): SearchTrendSummary {
  if (!data?.daily?.length || data.daily.length < 4) {
    return {
      label: "Connect search data",
      detail: "Connect Search Console or collect more daily data to see direction.",
      tone: "empty",
      deltaLabel: "No baseline",
    };
  }

  const midpoint = Math.floor(data.daily.length / 2);
  const previousClicks = sumNumbers(data.daily.slice(0, midpoint).map((point) => point.clicks));
  const currentClicks = sumNumbers(data.daily.slice(midpoint).map((point) => point.clicks));
  const delta = currentClicks - previousClicks;
  const percent = previousClicks > 0 ? Math.round((delta / previousClicks) * 100) : null;

  if (delta === 0) {
    return {
      label: "Search traffic is flat",
      detail: "Clicks are holding steady across the selected period.",
      tone: "flat",
      deltaLabel: "No change",
    };
  }

  const direction = delta > 0 ? "up" : "down";
  return {
    label: delta > 0 ? "Search traffic is improving" : "Search traffic is slipping",
    detail:
      percent == null
        ? `${Math.abs(delta)} click${Math.abs(delta) === 1 ? "" : "s"} ${direction} in the latest half of the period.`
        : `${Math.abs(percent)}% ${direction} in the latest half of the selected period.`,
    tone: delta > 0 ? "up" : "down",
    deltaLabel: `${delta > 0 ? "+" : ""}${delta} clicks`,
  };
}

/** Derive passive observations for the Google Search Visibility card. */
export function buildGscObservations(
  gscData: SearchConsoleData | null | undefined,
): GscObservation[] {
  if (!gscData) return [];
  const observations: GscObservation[] = [];

  const lowCtrQuery = [...(gscData.top_queries ?? [])]
    .filter((query) => query.impressions >= 100 && query.ctr < 0.03)
    .sort((a, b) => b.impressions - a.impressions)[0];
  if (lowCtrQuery) {
    observations.push({
      id: `query-ctr:${lowCtrQuery.query}`,
      label: `Snippet for ${formatQueryForDisplay(lowCtrQuery.query)} is underclicking`,
      detail: "Rewrite the title and meta description to explain the page more clearly.",
      metric: `${formatPercent(lowCtrQuery.ctr)} CTR · ${formatCompactNumber(lowCtrQuery.impressions)} impressions`,
      tone: "warning",
    });
  }

  const strikingQuery = [...(gscData.top_queries ?? [])]
    .filter((query) => query.position > 3 && query.position <= 20 && query.impressions >= 50)
    .sort((a, b) => a.position - b.position || b.impressions - a.impressions)[0];
  if (strikingQuery) {
    observations.push({
      id: `query-position:${strikingQuery.query}`,
      label: `${formatQueryForDisplay(strikingQuery.query)} is close to ranking well`,
      detail:
        "Strengthen the matching page with clearer headings, internal links, and content that answers the query.",
      metric: `Position ${strikingQuery.position.toFixed(1)}`,
      tone: "info",
    });
  }

  const lowCtrPage = [...(gscData.top_pages ?? [])]
    .filter((page) => page.impressions >= 100 && page.ctr < 0.025)
    .sort((a, b) => b.impressions - a.impressions)[0];
  if (lowCtrPage) {
    observations.push({
      id: `page-ctr:${lowCtrPage.page}`,
      label: `${lowCtrPage.page} is visible but underclicking`,
      detail: "The page surfaces in search results often but rarely earns the click.",
      metric: `${formatPercent(lowCtrPage.ctr)} CTR`,
      tone: "warning",
    });
  }

  return observations.slice(0, 4);
}

function formatQueryForDisplay(query: string): string {
  // Strip ASCII quotes before wrapping query text in guillemets.
  return `«${query.replace(/["«»]/g, "").trim()}»`;
}

export function buildSeoCategoryScore(checks: CheckResult[]): CategoryScore | null {
  const seoChecks = checks.filter((check) => check.category === "seo");
  if (seoChecks.length === 0) return null;

  let score = 100;
  let issuesCritical = 0;
  let issuesHigh = 0;
  let issuesMedium = 0;
  let issuesLow = 0;
  let issuesPassed = 0;

  for (const check of seoChecks) {
    switch (check.status) {
      case "pass":
        issuesPassed += 1;
        break;
      case "fail":
        score -= scoreIssueImpact(check.severity, check.confidence ?? "high", "fail", 1);
        switch (check.severity) {
          case "critical":
            issuesCritical += 1;
            break;
          case "high":
            issuesHigh += 1;
            break;
          case "medium":
            issuesMedium += 1;
            break;
          case "low":
            issuesLow += 1;
            break;
        }
        break;
      case "warn":
        score -= scoreIssueImpact(check.severity, check.confidence ?? "high", "warn", 1);
        switch (check.severity) {
          case "critical":
            issuesCritical += 1;
            break;
          case "high":
            issuesHigh += 1;
            break;
          case "medium":
            issuesMedium += 1;
            break;
          case "low":
            issuesLow += 1;
            break;
        }
        break;
      case "skipped":
        break;
    }
  }

  return {
    category: "seo",
    score: Math.trunc(Math.min(100, Math.max(0, score))),
    issuesTotal: issuesCritical + issuesHigh + issuesMedium + issuesLow,
    issuesCritical: issuesCritical,
    issuesHigh: issuesHigh,
    issuesMedium: issuesMedium,
    issuesLow: issuesLow,
    issuesPassed: issuesPassed,
  };
}

export { formatCheckStatus };

export function resolveVerifiedIssue(issue: CheckResult, results: CheckResult[]): CheckResult {
  const exact = results.find((candidate) => candidate.checkId === issue.checkId);
  if (exact) return exact;

  if (results.length > 0 && results.every((candidate) => candidate.status === "pass")) {
    return {
      ...issue,
      status: "pass",
      description: "Verified just now. This issue no longer reproduced in the targeted check.",
      fixPrompt: null,
      manualFix: null,
      rawData: null,
    };
  }

  return issue;
}

export function inferSeoFocus(issue: CheckResult): string | null {
  return inferSeoFocusFromText(`${issue.checkId} ${issue.title}`);
}

function matchesCoverageTerms(check: CheckResult, terms: readonly string[]) {
  const haystack = `${check.checkId} ${check.title} ${check.description}`.toLowerCase();
  return terms.some((term) => haystack.includes(term.toLowerCase()));
}

function sumNumbers(values: number[]): number {
  return values.reduce((sum, value) => sum + value, 0);
}

function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

function formatCompactNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(value);
}
