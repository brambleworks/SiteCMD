import type { CodeScanDomainSummary, CodeScanResult, CodeScanSummary } from "@/lib/types";
import { getCodeScanDomainSummaries } from "@/lib/code-scan-comparison";
import { countGroupedCodeIssues } from "@/lib/issue-ranking";
import {
  CODE_SCAN_DOMAIN_META,
  CODE_SCAN_DOMAIN_ORDER,
  type CodeScanDomain,
} from "@/lib/code-scan-domains";

type CodeScanDomainTrendTone = "improved" | "regressed" | "stable" | "shifted";

interface CodeScanDomainTrendSummary {
  label: string | null;
  tone: CodeScanDomainTrendTone | null;
}

interface CodeScanDomainRowSummary {
  domain: CodeScanDomain;
  label: string;
  score: number;
  count: number;
  criticalCount: number;
  highCount: number;
  delta: number | null;
  tone: "improved" | "regressed" | "stable";
}

function rankDomain(a: CodeScanDomain, b: CodeScanDomain) {
  return CODE_SCAN_DOMAIN_ORDER.indexOf(a) - CODE_SCAN_DOMAIN_ORDER.indexOf(b);
}

function compareDomainSummaries(a: CodeScanDomainSummary, b: CodeScanDomainSummary) {
  if (b.criticalCount !== a.criticalCount) return b.criticalCount - a.criticalCount;
  if (b.highCount !== a.highCount) return b.highCount - a.highCount;
  if (b.issueCount !== a.issueCount) return b.issueCount - a.issueCount;
  return rankDomain(a.domain, b.domain);
}

function getDomainCount(summaries: CodeScanDomainSummary[], domain: CodeScanDomain) {
  return summaries.find((summary) => summary.domain === domain)?.issueCount ?? 0;
}

function scoreDomainSummary(
  summary: Pick<CodeScanDomainSummary, "criticalCount" | "highCount" | "mediumCount" | "lowCount">,
) {
  const critical = summary.criticalCount;
  const high = summary.highCount;
  const medium = summary.mediumCount;
  const low = summary.lowCount;
  const penalty =
    25 * Math.sqrt(critical) + 12 * Math.sqrt(high) + 5 * Math.sqrt(medium) + Math.sqrt(low);
  const cap = critical > 0 ? 49 : high > 0 ? 79 : 100;
  return Math.round(Math.max(0, Math.min(cap, 100 - penalty)));
}

export function buildCodeScanSummaryFromResult(result: CodeScanResult): CodeScanSummary {
  const domainSummaries = getCodeScanDomainSummaries(result);
  const topDomain = [...domainSummaries].sort(compareDomainSummaries)[0] ?? null;

  return {
    id: result.id,
    projectId: result.projectId,
    environmentUrl: result.environmentUrl,
    overallScore: result.overallScore,
    issueCount: result.issueCount,
    groupedIssueCount:
      result.issues.length > 0 ? countGroupedCodeIssues(result.issues) : result.issueCount,
    criticalCount: result.criticalCount,
    highCount: result.highCount,
    durationMs: result.durationMs,
    checkedAt: result.checkedAt,
    framework: result.framework,
    topDomain: topDomain?.domain ?? null,
    topDomainCount: topDomain?.issueCount ?? 0,
    domainSummaries,
  };
}

export function describeCodeScanDomainTrend(
  currentSummary: CodeScanSummary | null,
  previousSummary: CodeScanSummary | null,
): CodeScanDomainTrendSummary {
  if (!currentSummary || !previousSummary) {
    return { label: null, tone: null };
  }

  const currentDomainSummaries = currentSummary.domainSummaries ?? [];
  const previousDomainSummaries = previousSummary.domainSummaries ?? [];
  if (currentDomainSummaries.length === 0 && previousDomainSummaries.length === 0) {
    return { label: null, tone: null };
  }

  let strongestDomain: CodeScanDomain | null = null;
  let strongestDelta = 0;

  for (const domain of CODE_SCAN_DOMAIN_ORDER) {
    const delta =
      getDomainCount(currentDomainSummaries, domain) -
      getDomainCount(previousDomainSummaries, domain);
    if (Math.abs(delta) > Math.abs(strongestDelta)) {
      strongestDomain = domain;
      strongestDelta = delta;
    }
  }

  if (strongestDomain && strongestDelta !== 0) {
    const label = CODE_SCAN_DOMAIN_META[strongestDomain].label;
    return strongestDelta > 0
      ? {
          label: `${label} grew by ${strongestDelta}`,
          tone: "regressed",
        }
      : {
          label: `${label} eased by ${Math.abs(strongestDelta)}`,
          tone: "improved",
        };
  }

  if (
    currentSummary.topDomain &&
    previousSummary.topDomain &&
    currentSummary.topDomain !== previousSummary.topDomain
  ) {
    return {
      label: `${CODE_SCAN_DOMAIN_META[currentSummary.topDomain].label} is now leading`,
      tone: "shifted",
    };
  }

  return {
    label: "Domain mix stable",
    tone: "stable",
  };
}

export function buildCodeScanDomainRows(
  currentSummary: CodeScanSummary | null,
  previousSummary: CodeScanSummary | null,
  limit = CODE_SCAN_DOMAIN_ORDER.length,
): CodeScanDomainRowSummary[] {
  if (!currentSummary) {
    return [];
  }

  const currentByDomain = new Map(
    (currentSummary.domainSummaries ?? []).map((summary) => [summary.domain, summary]),
  );
  const previousByDomain = new Map(
    (previousSummary?.domainSummaries ?? []).map((summary) => [summary.domain, summary]),
  );

  return CODE_SCAN_DOMAIN_ORDER.slice(0, limit).map((domain) => {
    const currentDomainSummary = currentByDomain.get(domain) ?? {
      domain,
      issueCount: 0,
      criticalCount: 0,
      highCount: 0,
      mediumCount: 0,
      lowCount: 0,
    };
    const previousDomainSummary = previousByDomain.get(domain);
    const currentScore = scoreDomainSummary(currentDomainSummary);
    const previousScore = previousDomainSummary ? scoreDomainSummary(previousDomainSummary) : null;
    const delta = previousScore == null ? null : currentScore - previousScore;

    return {
      domain,
      label: CODE_SCAN_DOMAIN_META[domain].shortLabel,
      score: currentScore,
      count: currentDomainSummary.issueCount,
      criticalCount: currentDomainSummary.criticalCount,
      highCount: currentDomainSummary.highCount,
      delta,
      tone: delta == null || delta === 0 ? "stable" : delta > 0 ? "improved" : "regressed",
    };
  });
}
