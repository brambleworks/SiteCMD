import type {
  CodeIssue,
  CodeScanDomainSummary,
  CodeScanResult,
  CodeScanSummary,
} from "@/lib/types";
import {
  CODE_SCAN_DOMAIN_ORDER,
  getCodeIssueDomain,
  type CodeScanDomain,
} from "@/lib/code-scan-domains";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { createSeverityCounts, severityRank, type SeverityCounts } from "@/lib/severity";

export type CodeIssueCounts = SeverityCounts;

interface CodeScanChangeItem {
  issueId: string;
  title: string;
  domain: CodeScanDomain;
  category: string;
  before: CodeIssue | null;
  after: CodeIssue | null;
}

interface CodeScanDomainComparison {
  domain: CodeScanDomain;
  beforeCount: number;
  afterCount: number;
  beforeSeverities: CodeIssueCounts;
  afterSeverities: CodeIssueCounts;
  fixedCount: number;
  newCount: number;
  changedCount: number;
  trend: "improved" | "regressed" | "stable";
}

export interface CodeScanComparison {
  scoreDelta: number;
  issueDelta: number;
  criticalDelta: number;
  highDelta: number;
  mediumDelta: number;
  lowDelta: number;
  fixed: CodeScanChangeItem[];
  newIssues: CodeScanChangeItem[];
  changed: CodeScanChangeItem[];
  unchangedCount: number;
  domains: CodeScanDomainComparison[];
}

function domainSummaryToCounts(summary: CodeScanDomainSummary): CodeIssueCounts {
  return createSeverityCounts({
    critical: summary.criticalCount,
    high: summary.highCount,
    medium: summary.mediumCount,
    low: summary.lowCount,
  });
}

export function getCodeScanDomainSummaries(
  result: Pick<CodeScanResult, "issues" | "domainSummaries">,
): CodeScanDomainSummary[] {
  if (result.domainSummaries && result.domainSummaries.length > 0) {
    return result.domainSummaries;
  }

  const buckets = new Map<CodeScanDomain, CodeScanDomainSummary>();
  for (const issue of result.issues) {
    const domain = getCodeIssueDomain(issue);
    if (!buckets.has(domain)) {
      buckets.set(domain, {
        domain,
        issueCount: 0,
        criticalCount: 0,
        highCount: 0,
        mediumCount: 0,
        lowCount: 0,
      });
    }
    const bucket = buckets.get(domain)!;
    bucket.issueCount += 1;
    switch (issue.severity) {
      case "critical":
        bucket.criticalCount += 1;
        break;
      case "high":
        bucket.highCount += 1;
        break;
      case "medium":
        bucket.mediumCount += 1;
        break;
      case "low":
        bucket.lowCount += 1;
        break;
    }
  }

  return CODE_SCAN_DOMAIN_ORDER.map((domain) => buckets.get(domain)).filter(
    (summary): summary is CodeScanDomainSummary => Boolean(summary),
  );
}

export function buildSummaryOnlyCodeScanResult(result: CodeScanResult): CodeScanResult {
  return {
    ...result,
    domainSummaries: getCodeScanDomainSummaries(result),
    issues: [],
  };
}

export function summarizeCodeIssueCounts(issues: CodeIssue[]): CodeIssueCounts {
  const counts = createSeverityCounts();

  for (const issue of issues) {
    counts[issue.severity] += 1;
  }

  return counts;
}

export function sortCodeIssues(a: CodeIssue, b: CodeIssue) {
  const severityDelta = severityRank(a.severity) - severityRank(b.severity);
  if (severityDelta !== 0) return severityDelta;
  return a.title.localeCompare(b.title);
}

function normalizeCodeScanTargetUrl(url: string | null | undefined) {
  return normalizeAppUrlForKey(url);
}

export function getPreviousCodeScanSummary(current: CodeScanResult, history: CodeScanSummary[]) {
  if (!history.length) return null;

  const currentTarget = normalizeCodeScanTargetUrl(current.environmentUrl);
  const currentIndex = history.findIndex((entry) => entry.id === current.id);
  const pool =
    currentIndex >= 0
      ? history.slice(currentIndex + 1)
      : history.filter((entry) => entry.id !== current.id);

  const sameTarget = pool.find(
    (entry) => normalizeCodeScanTargetUrl(entry.environmentUrl) === currentTarget,
  );
  if (sameTarget) return sameTarget;

  return pool[0] ?? null;
}

function compareSeverityWeightFromCounts(counts: CodeIssueCounts) {
  return counts.critical * 8 + counts.high * 4 + counts.medium * 2 + counts.low;
}

function computeDomainTrendFromCounts(
  beforeCounts: CodeIssueCounts,
  afterCounts: CodeIssueCounts,
  beforeCount: number,
  afterCount: number,
) {
  const beforeWeight = compareSeverityWeightFromCounts(beforeCounts);
  const afterWeight = compareSeverityWeightFromCounts(afterCounts);
  if (afterWeight < beforeWeight) return "improved" as const;
  if (afterWeight > beforeWeight) return "regressed" as const;
  if (afterCount < beforeCount) return "improved" as const;
  if (afterCount > beforeCount) return "regressed" as const;
  return "stable" as const;
}

export function computeCodeScanComparison(
  before: CodeScanResult,
  after: CodeScanResult,
): CodeScanComparison {
  const hasDetailedIssues = before.issues.length > 0 && after.issues.length > 0;
  const beforeById = new Map(before.issues.map((issue) => [issue.id, issue]));
  const afterById = new Map(after.issues.map((issue) => [issue.id, issue]));

  const fixed: CodeScanChangeItem[] = [];
  const newIssues: CodeScanChangeItem[] = [];
  const changed: CodeScanChangeItem[] = [];
  let unchangedCount = 0;

  if (hasDetailedIssues) {
    for (const issue of before.issues) {
      const nextIssue = afterById.get(issue.id);
      if (!nextIssue) {
        fixed.push({
          issueId: issue.id,
          title: issue.title,
          domain: getCodeIssueDomain(issue),
          category: issue.category,
          before: issue,
          after: null,
        });
        continue;
      }

      if (nextIssue.severity !== issue.severity || nextIssue.title !== issue.title) {
        changed.push({
          issueId: issue.id,
          title: nextIssue.title,
          domain: getCodeIssueDomain(nextIssue),
          category: nextIssue.category,
          before: issue,
          after: nextIssue,
        });
      } else {
        unchangedCount += 1;
      }
    }

    for (const issue of after.issues) {
      if (!beforeById.has(issue.id)) {
        newIssues.push({
          issueId: issue.id,
          title: issue.title,
          domain: getCodeIssueDomain(issue),
          category: issue.category,
          before: null,
          after: issue,
        });
      }
    }
  }

  const beforeDomainSummaries = getCodeScanDomainSummaries(before);
  const afterDomainSummaries = getCodeScanDomainSummaries(after);

  const domains = CODE_SCAN_DOMAIN_ORDER.map((domain) => {
    const beforeSummary = beforeDomainSummaries.find((summary) => summary.domain === domain);
    const afterSummary = afterDomainSummaries.find((summary) => summary.domain === domain);
    const beforeIssues = hasDetailedIssues
      ? before.issues.filter((issue) => getCodeIssueDomain(issue) === domain)
      : [];
    const afterIssues = hasDetailedIssues
      ? after.issues.filter((issue) => getCodeIssueDomain(issue) === domain)
      : [];
    const beforeCount = beforeSummary?.issueCount ?? beforeIssues.length;
    const afterCount = afterSummary?.issueCount ?? afterIssues.length;
    const beforeSeverities = beforeSummary
      ? domainSummaryToCounts(beforeSummary)
      : summarizeCodeIssueCounts(beforeIssues);
    const afterSeverities = afterSummary
      ? domainSummaryToCounts(afterSummary)
      : summarizeCodeIssueCounts(afterIssues);
    return {
      domain,
      beforeCount,
      afterCount,
      beforeSeverities,
      afterSeverities,
      fixedCount: hasDetailedIssues ? fixed.filter((item) => item.domain === domain).length : 0,
      newCount: hasDetailedIssues ? newIssues.filter((item) => item.domain === domain).length : 0,
      changedCount: hasDetailedIssues ? changed.filter((item) => item.domain === domain).length : 0,
      trend: computeDomainTrendFromCounts(
        beforeSeverities,
        afterSeverities,
        beforeCount,
        afterCount,
      ),
    };
  }).filter((domain) => domain.beforeCount > 0 || domain.afterCount > 0);

  return {
    scoreDelta: after.overallScore - before.overallScore,
    issueDelta: after.issueCount - before.issueCount,
    criticalDelta: after.criticalCount - before.criticalCount,
    highDelta: after.highCount - before.highCount,
    mediumDelta: after.mediumCount - before.mediumCount,
    lowDelta: after.lowCount - before.lowCount,
    fixed: fixed.sort((a, b) => a.title.localeCompare(b.title)),
    newIssues: newIssues.sort((a, b) => sortCodeIssues(a.after!, b.after!)),
    changed: changed.sort((a, b) => sortCodeIssues(a.after ?? a.before!, b.after ?? b.before!)),
    unchangedCount,
    domains,
  };
}
