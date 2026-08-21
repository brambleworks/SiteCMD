import { CODE_SCAN_DOMAIN_ORDER, getCodeIssueDomain } from "@/lib/code-scan-domains";
import { countGroupedCodeIssues } from "@/lib/issue-ranking";
import type { CodeIssue, CodeScanDomain, CodeScanResult, CodeScanSummary } from "@/lib/types";
import type { LatestCodeScanSnapshot, ProjectSignalSnapshot } from "@/lib/project-summary-types";

const primedCodeScanSnapshots = new Map<number, LatestCodeScanSnapshot>();

function summarizeTopDomain(issues: CodeIssue[]): {
  topDomain: CodeScanDomain | null;
  topDomainCount: number;
} {
  if (issues.length === 0) {
    return { topDomain: null, topDomainCount: 0 };
  }

  const counts = new Map<CodeScanDomain, number>();
  for (const issue of issues) {
    const domain = getCodeIssueDomain(issue);
    counts.set(domain, (counts.get(domain) ?? 0) + 1);
  }

  const ranked = Array.from(counts.entries()).sort((a, b) => {
    if (b[1] !== a[1]) return b[1] - a[1];
    return CODE_SCAN_DOMAIN_ORDER.indexOf(a[0]) - CODE_SCAN_DOMAIN_ORDER.indexOf(b[0]);
  });

  const [topDomain, topDomainCount] = ranked[0] ?? [null, 0];
  return {
    topDomain,
    topDomainCount,
  };
}

function summarizeTopDomainFromResult(result: CodeScanResult): {
  topDomain: CodeScanDomain | null;
  topDomainCount: number;
} {
  if (result.issues.length > 0) {
    return summarizeTopDomain(result.issues);
  }

  if (!result.domainSummaries || result.domainSummaries.length === 0) {
    return { topDomain: null, topDomainCount: 0 };
  }

  const ranked = [...result.domainSummaries].sort((a, b) => {
    if (b.issueCount !== a.issueCount) return b.issueCount - a.issueCount;
    return CODE_SCAN_DOMAIN_ORDER.indexOf(a.domain) - CODE_SCAN_DOMAIN_ORDER.indexOf(b.domain);
  });

  const top = ranked[0];
  return top
    ? { topDomain: top.domain, topDomainCount: top.issueCount }
    : { topDomain: null, topDomainCount: 0 };
}

function buildCodeScanSummary(result: CodeScanResult): CodeScanSummary {
  const { topDomain, topDomainCount } = summarizeTopDomainFromResult(result);
  const groupedIssueCount =
    result.issues.length > 0 ? countGroupedCodeIssues(result.issues) : result.issueCount;
  return {
    id: result.id,
    projectId: result.projectId,
    environmentUrl: result.environmentUrl,
    overallScore: result.overallScore,
    issueCount: result.issueCount,
    groupedIssueCount,
    criticalCount: result.criticalCount,
    highCount: result.highCount,
    durationMs: result.durationMs,
    checkedAt: result.checkedAt,
    framework: result.framework,
    topDomain,
    topDomainCount,
    domainSummaries: result.domainSummaries ?? [],
  };
}

function mergeSummaryWithPersistedCounts(
  primedSummary: CodeScanSummary | null,
  persistedSummary: CodeScanSummary | null,
): CodeScanSummary | null {
  if (!primedSummary || !persistedSummary || primedSummary.id !== persistedSummary.id) {
    return primedSummary;
  }

  // Keep active grouped totals and severity counts from the same persisted source.
  const usePersistedActiveCounts = persistedSummary.groupedIssueCount > 0;

  return {
    ...primedSummary,
    groupedIssueCount: usePersistedActiveCounts
      ? persistedSummary.groupedIssueCount
      : primedSummary.groupedIssueCount,
    criticalCount: usePersistedActiveCounts
      ? persistedSummary.criticalCount
      : primedSummary.criticalCount,
    highCount: usePersistedActiveCounts ? persistedSummary.highCount : primedSummary.highCount,
    topDomain: persistedSummary.topDomain ?? primedSummary.topDomain,
    topDomainCount:
      persistedSummary.topDomainCount > 0
        ? persistedSummary.topDomainCount
        : primedSummary.topDomainCount,
    domainSummaries:
      persistedSummary.domainSummaries.length > 0
        ? persistedSummary.domainSummaries
        : primedSummary.domainSummaries,
  };
}

/** Test seam for selecting the freshest summary. */
export function shouldPreferPrimed(
  primed: LatestCodeScanSnapshot | undefined,
  snapshot: ProjectSignalSnapshot,
): primed is LatestCodeScanSnapshot {
  const primedHead = primed?.summary ?? primed?.result;
  if (!primedHead) return false;
  // Summary metadata remains available when detail is absent, so compare freshness there.
  const snapshotHead = snapshot.codeScanSummary ?? snapshot.codeScanDetail;
  if (!snapshotHead) return true;
  const primedTime = Date.parse(primedHead.checkedAt);
  const snapshotTime = Date.parse(snapshotHead.checkedAt);
  if (Number.isNaN(primedTime) || Number.isNaN(snapshotTime)) {
    return primedHead.id >= snapshotHead.id;
  }
  return primedTime >= snapshotTime;
}

function mergePrimedCodeScan(snapshot: ProjectSignalSnapshot): ProjectSignalSnapshot {
  const primed = primedCodeScanSnapshots.get(snapshot.projectId);
  if (!shouldPreferPrimed(primed, snapshot)) {
    return snapshot;
  }
  const mergedPrimedSummary = mergeSummaryWithPersistedCounts(
    primed.summary,
    snapshot.codeScanSummary,
  );
  const previousCodeScanSummary =
    snapshot.codeScanSummary &&
    mergedPrimedSummary &&
    snapshot.codeScanSummary.id !== mergedPrimedSummary.id
      ? snapshot.codeScanSummary
      : snapshot.previousCodeScanSummary;
  return {
    ...snapshot,
    codeScanSummary: mergedPrimedSummary,
    previousCodeScanSummary,
    codeScanDetail:
      primed.result ??
      (snapshot.codeScanDetail?.id === mergedPrimedSummary?.id ? snapshot.codeScanDetail : null),
  };
}

export function mergePrimedCodeScanForAccess(
  snapshot: ProjectSignalSnapshot,
  includeCodeScanDetail: boolean,
): ProjectSignalSnapshot {
  const merged = mergePrimedCodeScan(snapshot);
  if (includeCodeScanDetail) return merged;
  return {
    ...merged,
    codeScanDetail: null,
  };
}

export function primeLatestCodeScanSnapshot(result: CodeScanResult) {
  const hasDetailedIssuePayload = result.issues.length > 0 || result.issueCount === 0;
  primedCodeScanSnapshots.set(result.projectId, {
    summary: buildCodeScanSummary(result),
    result: hasDetailedIssuePayload ? result : null,
  });
}

export function invalidateLatestCodeScanSnapshot(projectId: number) {
  primedCodeScanSnapshots.delete(projectId);
}
