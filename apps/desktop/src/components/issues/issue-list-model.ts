import type { BatchFixItem } from "@/lib/fix-copilot-batch";
import {
  getCodeScanDomainFromFocus,
  getIssuesSeverityFromFocus,
  getIssuesSourceFromFocus,
  getIssuesWebCategoryFromFocus,
  type IssueSeverityFocus,
} from "@/lib/app-targets";
import {
  CODE_SCAN_DOMAIN_META,
  CODE_SCAN_DOMAIN_ORDER,
  getCodeIssueDomain,
} from "@/lib/code-scan-domains";
import type { UnifiedFixIssue } from "@/lib/issue-ranking";
import { CATEGORY_LABELS, CATEGORY_ORDER } from "@/lib/tokens";
import { severityRank } from "@/lib/severity";
import type { CodeScanDomain, ScanCategory } from "@/lib/types";

type IssueSource = "web" | "code";
export type IssueSourceFilter = "all" | IssueSource;
export type IssueFilter =
  | { kind: "web-category"; category: ScanCategory }
  | { kind: "code-domain"; domain: CodeScanDomain };
export type SeverityFilter = "all" | IssueSeverityFocus;
export type IssueStatusFilter = "active" | "ignored" | "blocked" | "resolved" | "all";
type ScanFixIssue = Extract<UnifiedFixIssue, { kind: "web" | "code" }>;

interface FilterOption {
  value: string;
  label: string;
}

export const ISSUE_SOURCE_LABELS: Record<IssueSourceFilter, string> = {
  all: "All",
  web: "Web Scan",
  code: "Code Scan",
};

export const SEVERITY_FILTER_LABELS: Record<SeverityFilter, string> = {
  all: "All severities",
  critical: "Critical",
  high: "High",
  medium: "Medium",
  low: "Low",
};

export const ISSUE_STATUS_LABELS: Record<IssueStatusFilter, string> = {
  active: "Active",
  ignored: "Ignored",
  blocked: "Blocked",
  resolved: "Resolved",
  all: "All",
};

export function parseIssueFilterFocus(focus?: string | null): IssueFilter | null {
  const webCategory = getIssuesWebCategoryFromFocus(focus);
  if (webCategory) {
    return { kind: "web-category", category: webCategory };
  }
  const codeDomain = getCodeScanDomainFromFocus(focus);
  if (codeDomain) {
    return { kind: "code-domain", domain: codeDomain };
  }
  return null;
}

export function parseSeverityFocus(focus?: string | null) {
  return getIssuesSeverityFromFocus(focus);
}

export function parseIssueSourceFocus(focus?: string | null): IssueSource | null {
  return getIssuesSourceFromFocus(focus);
}

export function filterScanIssues(
  ranked: UnifiedFixIssue[],
  activeSource: IssueSourceFilter,
  activeSeverity: SeverityFilter,
  activeFilter: IssueFilter | null,
): ScanFixIssue[] {
  return ranked.filter((item): item is ScanFixIssue => {
    if (item.kind === "alert") return false;
    if (activeSource !== "all" && item.kind !== activeSource) return false;
    if (activeSeverity !== "all" && item.issue.severity !== activeSeverity) return false;
    if (!activeFilter) return true;
    if (activeFilter.kind === "web-category") {
      return item.kind === "web" && item.issue.category === activeFilter.category;
    }
    return item.kind === "code" && getCodeIssueDomain(item.issue) === activeFilter.domain;
  });
}

export function sortScanItems(items: ScanFixIssue[]) {
  return [...items].sort((a, b) => {
    const severityDelta = severityRank(a.issue.severity) - severityRank(b.issue.severity);
    if (severityDelta !== 0) return severityDelta;
    if (b.impact !== a.impact) return b.impact - a.impact;
    return a.issue.title.localeCompare(b.issue.title);
  });
}

export function buildWebFilterCounts(ranked: UnifiedFixIssue[]) {
  const counts = new Map<ScanCategory, number>();
  for (const item of ranked) {
    if (item.kind !== "web") continue;
    counts.set(item.issue.category, (counts.get(item.issue.category) ?? 0) + 1);
  }
  return CATEGORY_ORDER.map((category) => ({
    category,
    count: counts.get(category) ?? 0,
  })).filter((entry) => entry.count > 0);
}

export function buildCodeFilterCounts(ranked: UnifiedFixIssue[]) {
  const counts = new Map<CodeScanDomain, number>();
  for (const item of ranked) {
    if (item.kind !== "code") continue;
    const domain = getCodeIssueDomain(item.issue);
    counts.set(domain, (counts.get(domain) ?? 0) + 1);
  }
  return CODE_SCAN_DOMAIN_ORDER.map((domain) => ({
    domain,
    count: counts.get(domain) ?? 0,
  })).filter((entry) => entry.count > 0);
}

export function buildSubfilterOptions({
  activeSource,
  webCount,
  codeCount,
  webFilterCounts,
  codeFilterCounts,
}: {
  activeSource: IssueSourceFilter;
  webCount: number;
  codeCount: number;
  webFilterCounts: ReturnType<typeof buildWebFilterCounts>;
  codeFilterCounts: ReturnType<typeof buildCodeFilterCounts>;
}): FilterOption[] {
  if (activeSource === "web") {
    return [
      { value: "all", label: `All web categories (${webCount})` },
      ...webFilterCounts.map((entry) => ({
        value: `web:${entry.category}`,
        label: `${CATEGORY_LABELS[entry.category] ?? entry.category} (${entry.count})`,
      })),
    ];
  }

  if (activeSource === "code") {
    return [
      { value: "all", label: `All code categories (${codeCount})` },
      ...codeFilterCounts.map((entry) => ({
        value: `code:${entry.domain}`,
        label: `${CODE_SCAN_DOMAIN_META[entry.domain]?.label ?? entry.domain} (${entry.count})`,
      })),
    ];
  }

  return [{ value: "all", label: "All subcategories" }];
}

export function getActiveSubfilterValue(activeFilter: IssueFilter | null) {
  if (!activeFilter) return "all";
  return activeFilter.kind === "web-category"
    ? `web:${activeFilter.category}`
    : `code:${activeFilter.domain}`;
}

export function buildBatchFixItems(scanItems: ScanFixIssue[]): BatchFixItem[] {
  return scanItems.map((item) => {
    if (item.kind === "web") {
      const issue = item.issue;
      return {
        kind: "web" as const,
        title: issue.title,
        severity: issue.severity,
        category: issue.category,
        description: issue.description,
        fixHint: issue.fixPrompt || issue.manualFix || null,
        filePath: null,
      };
    }
    const issue = item.issue;
    return {
      kind: "code" as const,
      title: issue.title,
      severity: issue.severity,
      category: issue.category,
      description: issue.description,
      fixHint: issue.likelyFix || null,
      filePath: issue.relativePath || null,
    };
  });
}
