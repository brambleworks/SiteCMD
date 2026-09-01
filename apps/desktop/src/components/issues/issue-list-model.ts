import {
  getCodeScanDomainFromFocus,
  getIssuesSeverityFromFocus,
  getIssuesWebCategoryFromFocus,
  type IssueSeverityFocus,
} from "@/lib/app-targets";
import { getCodeIssueDomain } from "@/lib/code-scan-domains";
import {
  ISSUE_CATEGORY_ORDER,
  issueCategoryLabel,
  type IssueCategoryKey,
} from "@/lib/issue-categories";
import type { UnifiedFixIssue } from "@/lib/issue-ranking";
import { severityRank } from "@/lib/severity";

export type SeverityFilter = "all" | IssueSeverityFocus;
export type IssueStatusFilter = "active" | "ignored" | "blocked" | "resolved" | "all";
type ScanFixIssue = Extract<UnifiedFixIssue, { kind: "web" | "code" }>;

interface FilterOption {
  value: string;
  label: string;
}

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

/** Category focus targets arrive as either a web category or a code domain. */
export function parseIssueCategoryFocus(focus?: string | null): IssueCategoryKey | null {
  return getIssuesWebCategoryFromFocus(focus) ?? getCodeScanDomainFromFocus(focus) ?? null;
}

export function parseSeverityFocus(focus?: string | null) {
  return getIssuesSeverityFromFocus(focus);
}

/** The category a row belongs to, whichever scanner reported it. */
function issueCategoryOf(item: ScanFixIssue): IssueCategoryKey {
  return item.kind === "web" ? item.issue.category : getCodeIssueDomain(item.issue);
}

export function filterScanIssues(
  ranked: UnifiedFixIssue[],
  activeSeverity: SeverityFilter,
  activeCategory: IssueCategoryKey | null,
): ScanFixIssue[] {
  return ranked.filter((item): item is ScanFixIssue => {
    if (item.kind === "alert") return false;
    if (activeSeverity !== "all" && item.issue.severity !== activeSeverity) return false;
    if (!activeCategory) return true;
    return issueCategoryOf(item) === activeCategory;
  });
}

/** Plain substring match on the titles the list already shows. */
export function filterIssuesByTitle(items: ScanFixIssue[], query: string): ScanFixIssue[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return items;
  return items.filter((item) => item.issue.title.toLowerCase().includes(needle));
}

export function sortScanItems(items: ScanFixIssue[]) {
  return [...items].sort((a, b) => {
    const severityDelta = severityRank(a.issue.severity) - severityRank(b.issue.severity);
    if (severityDelta !== 0) return severityDelta;
    if (b.impact !== a.impact) return b.impact - a.impact;
    return a.issue.title.localeCompare(b.issue.title);
  });
}

export function buildCategoryFilterCounts(ranked: UnifiedFixIssue[]) {
  const counts = new Map<IssueCategoryKey, number>();
  for (const item of ranked) {
    if (item.kind === "alert") continue;
    const key = issueCategoryOf(item);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return ISSUE_CATEGORY_ORDER.map((key) => ({ key, count: counts.get(key) ?? 0 })).filter(
    (entry) => entry.count > 0,
  );
}

export function buildCategoryOptions(
  categoryCounts: ReturnType<typeof buildCategoryFilterCounts>,
): FilterOption[] {
  const total = categoryCounts.reduce((sum, entry) => sum + entry.count, 0);
  return [
    { value: "all", label: `All categories (${total})` },
    ...categoryCounts.map((entry) => ({
      value: entry.key,
      label: `${issueCategoryLabel(entry.key)} (${entry.count})`,
    })),
  ];
}
