import {
  countGroupedCodeIssues,
  countGroupedWebIssues,
  getGroupedSeverityCounts,
} from "@/lib/issue-ranking";
import { summarizeIssueSeverities } from "@/lib/issues";
import { getProjectWorkSummaryIssueTotal } from "@/lib/project-work-summary";
import { addSeverityCounts, createSeverityCounts, type SeverityCounts } from "@/lib/severity";
import type { IssueGroup } from "@/lib/types";
import type { ProjectWorkSummary } from "@/lib/project-summary-types";

interface SeverityEntry {
  severity: string;
  checkId?: string;
  category?: string;
  title?: string;
}

interface CodeIssueSummaryFallback {
  issueCount: number;
  criticalCount: number;
  highCount: number;
  topDomainLabel?: string | null;
  topDomainCount?: number;
  mode?: "locked" | "summary";
}

export interface ProjectIssueSummary {
  webCount: number;
  codeCount: number;
  totalCount: number;
  criticalCount: number;
  /** Shared severity counts; their sum equals `totalCount`. */
  severityCounts: SeverityCounts;
}

export const getProjectIssueTotalFromWorkSummary = getProjectWorkSummaryIssueTotal;

/** Adapt canonical issue groups for count-only UI surfaces. */
export function buildProjectIssueSummaryFromWorkSummary(
  summary: ProjectWorkSummary,
): ProjectIssueSummary {
  const severityCounts = createSeverityCounts({
    critical: summary.issueCriticalCount ?? 0,
    high: summary.issueHighCount ?? 0,
    medium: summary.issueMediumCount ?? 0,
    low: summary.issueLowCount ?? 0,
  });
  return {
    webCount: summary.issueWebCount ?? 0,
    codeCount: summary.issueCodeCount ?? 0,
    totalCount: summary.issueCount ?? getProjectWorkSummaryIssueTotal(summary),
    criticalCount: severityCounts.critical,
    severityCounts,
  };
}

/** Count groups once and attribute cross-source groups to Web in two-way filters. */
export function buildIssueGroupSummary(groups: readonly IssueGroup[]): ProjectIssueSummary {
  const active = groups.filter((group) => group.status === "new" || group.status === "regressed");
  const severityCounts = createSeverityCounts();
  let webCount = 0;
  let codeCount = 0;
  for (const group of active) {
    severityCounts[group.severity] += 1;
    const codeOnly = group.instances.every((instance) => instance.source === "code_scan");
    if (codeOnly) codeCount += 1;
    else webCount += 1;
  }
  return {
    webCount,
    codeCount,
    totalCount: active.length,
    criticalCount: severityCounts.critical,
    severityCounts,
  };
}

function getCodeIssueCount(
  codeIssues: readonly SeverityEntry[],
  codeSummaryFallback?: Pick<CodeIssueSummaryFallback, "issueCount"> | null,
): number {
  if (codeIssues.length === 0) return codeSummaryFallback?.issueCount ?? 0;
  const groupedCount = codeIssues.every(
    (issue): issue is SeverityEntry & { checkId: string } =>
      typeof (issue as { checkId?: unknown }).checkId === "string",
  )
    ? countGroupedCodeIssues(codeIssues)
    : codeIssues.length;
  return groupedCount;
}

interface BuildProjectIssueSummaryArgs {
  webIssues: readonly SeverityEntry[];
  codeIssues: readonly SeverityEntry[];
  codeSummaryFallback?: CodeIssueSummaryFallback | null;
}

/** Adapt immutable scan artifacts for historical comparison views. */
export function buildProjectIssueSummary({
  webIssues,
  codeIssues,
  codeSummaryFallback = null,
}: BuildProjectIssueSummaryArgs): ProjectIssueSummary {
  const webHasCheckIds = webIssues.every(
    (issue): issue is SeverityEntry & { checkId: string } =>
      typeof (issue as { checkId?: unknown }).checkId === "string",
  );
  const webCount = webHasCheckIds ? countGroupedWebIssues(webIssues) : webIssues.length;
  const codeCount = getCodeIssueCount(codeIssues, codeSummaryFallback);
  const totalCount = webCount + codeCount;

  const codeHasGroupingFields =
    codeIssues.length > 0 &&
    codeIssues.every(
      (issue): issue is SeverityEntry & { checkId: string } =>
        typeof (issue as { checkId?: unknown }).checkId === "string",
    );

  // Independent source slices preserve grouped web counts when code detail is absent.
  const webSlice = webHasCheckIds
    ? getGroupedSeverityCounts(webIssues, [])
    : rawSeverityCounts(webIssues);
  const codeSlice = codeHasGroupingFields
    ? getGroupedSeverityCounts([], codeIssues)
    : fallbackCodeSeverity(codeIssues, codeSummaryFallback);

  const severityCounts = addSeverityCounts(webSlice, codeSlice);

  const criticalCount = severityCounts.critical;

  return {
    webCount,
    codeCount,
    totalCount,
    criticalCount,
    severityCounts,
  };
}

function rawSeverityCounts(issues: readonly SeverityEntry[]): SeverityCounts {
  return summarizeIssueSeverities(issues);
}

function fallbackCodeSeverity(
  codeIssues: readonly SeverityEntry[],
  codeSummaryFallback: CodeIssueSummaryFallback | null,
): SeverityCounts {
  if (codeIssues.length > 0) return rawSeverityCounts(codeIssues);
  if (!codeSummaryFallback) return createSeverityCounts();
  // Clamp the severity breakdown so it cannot exceed the active grouped total.
  const budget = Math.max(0, codeSummaryFallback.issueCount);
  const critical = Math.min(codeSummaryFallback.criticalCount, budget);
  const high = Math.min(codeSummaryFallback.highCount, budget - critical);
  const medium = budget - critical - high;
  return createSeverityCounts({ critical, high, medium });
}
