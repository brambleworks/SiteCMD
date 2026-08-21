import type { MultiScanResult } from "@/hooks/useScan";
import type { ScanSummary, ScanSessionSummary } from "@/hooks/useHistory";
import { normalizeAppUrlForOptionalKey } from "@/lib/app-targets";
import { addSeverityCounts, createSeverityCounts, type SeverityCounts } from "@/lib/severity";
import type { CodeScanResult, CodeScanSummary, ScanResult } from "@/lib/types";
import { formatUrlHost } from "@/lib/utils";
import { countActionableCheckResults, isActionableCheckResult } from "@/lib/issues";
import { buildProjectIssueSummary, type ProjectIssueSummary } from "@/lib/project-issue-summary";

type ScanSummaryKind = "web" | "code" | "multi";

type ScanSummarySeverityCounts = SeverityCounts;

interface ScanSummarySectionModel {
  kind: ScanSummaryKind;
  label: string;
  score: number | null;
  issueCount: number;
  severityCounts: ScanSummarySeverityCounts;
  previousScore: number | null;
  scoreDelta: number | null;
  previousIssueCount: number | null;
  issueDelta: number | null;
  resolvedCount: number | null;
  detail: string;
  // Code-scan scopes the walker skipped.
  note: string | null;
}

export interface ScanSummaryModel {
  id: string;
  title: string;
  scopeLabel: string;
  // Unified SiteCMD Score; null without project scoring context.
  siteCmdScore: number | null;
  totalIssues: number;
  severityCounts: ScanSummarySeverityCounts;
  estimatedNewIssues: number | null;
  resolvedIssues: number | null;
  regressionCount: number;
  // Code-scan scopes the walker skipped.
  note: string | null;
}

interface BuildScanSummaryModelInput {
  result: ScanResult | null;
  codeResult: CodeScanResult | null;
  multiResult: MultiScanResult | null;
  sitecmdScore: number | null;
  history: ScanSummary[];
  codeHistory: CodeScanSummary[];
  sessions: ScanSessionSummary[];
  scopeLabel: string;
  // Lifecycle-excluded check ids used to align the headline with active issues.
  inactiveCheckIds?: ReadonlySet<string>;
  // Persisted active-issue counts override the pre-hydration scan fallback.
  persistedSummary?: ProjectIssueSummary | null;
}

const EMPTY_INACTIVE_CHECK_IDS: ReadonlySet<string> = new Set();

function normalizeUrl(value: string | null | undefined): string | null {
  return normalizeAppUrlForOptionalKey(value);
}

function countWebSeverities(result: ScanResult): ScanSummarySeverityCounts {
  const counts = createSeverityCounts();
  for (const issue of result.issues) {
    if (!isActionableCheckResult(issue)) continue;
    counts[issue.severity] += 1;
  }
  return counts;
}

function webIssueCount(result: ScanResult): number {
  return countActionableCheckResults(result.issues);
}

/** Build a summary note for scopes skipped during the current code scan. */
export function buildSkippedScopeNote(skipped: CodeScanResult["skippedScopes"]): string | null {
  if (!skipped) return null;
  const { nestedRepositories, gitignoredDirectories, sampleNames } = skipped;
  if (nestedRepositories + gitignoredDirectories === 0) return null;

  const parts: string[] = [];
  if (nestedRepositories > 0) {
    parts.push(
      `${nestedRepositories} nested ${nestedRepositories === 1 ? "repository" : "repositories"}`,
    );
  }
  if (gitignoredDirectories > 0) {
    parts.push(
      `${gitignoredDirectories} gitignored ${gitignoredDirectories === 1 ? "directory" : "directories"}`,
    );
  }
  const names = sampleNames.length > 0 ? ` (${sampleNames.join(", ")})` : "";
  return `Skipped ${parts.join(" and ")}${names}. Nested repositories and gitignored folders are not scanned as this project's code, so they add no findings here.`;
}

function countCodeSeverities(result: CodeScanResult): ScanSummarySeverityCounts {
  return createSeverityCounts({
    critical: result.criticalCount,
    high: result.highCount,
    medium: result.mediumCount,
    low: result.lowCount,
  });
}

function findPreviousWebSummary(result: ScanResult, history: ScanSummary[]): ScanSummary | null {
  const resultUrl = normalizeUrl(result.url);
  return (
    history.find((entry) => {
      if (normalizeUrl(entry.url) !== resultUrl) return false;
      if (entry.timestamp === result.timestamp) return false;
      return true;
    }) ?? null
  );
}

function findPreviousCodeSummary(
  result: CodeScanResult,
  codeHistory: CodeScanSummary[],
): CodeScanSummary | null {
  return (
    codeHistory.find((entry) => {
      if (entry.projectId !== result.projectId) return false;
      if (entry.id === result.id) return false;
      return true;
    }) ?? null
  );
}

function findPreviousSessionSummary(
  result: MultiScanResult,
  sessions: ScanSessionSummary[],
): ScanSessionSummary | null {
  return sessions.find((entry) => entry.sessionId !== result.sessionId) ?? null;
}

function buildDelta(
  currentIssueCount: number,
  currentScore: number | null,
  previousIssueCount: number | null,
  previousScore: number | null,
) {
  const issueDelta = previousIssueCount == null ? null : currentIssueCount - previousIssueCount;
  return {
    issueDelta,
    resolvedCount: issueDelta == null ? null : Math.max(0, -issueDelta),
    scoreDelta: previousScore == null || currentScore == null ? null : currentScore - previousScore,
  };
}

function buildWebSection(result: ScanResult, history: ScanSummary[]): ScanSummarySectionModel {
  const previous = findPreviousWebSummary(result, history);
  const issueCount = webIssueCount(result);
  const previousIssueCount = previous?.issuesTotal ?? null;
  const previousScore = previous?.overallScore ?? null;
  const delta = buildDelta(issueCount, result.overallScore, previousIssueCount, previousScore);

  return {
    kind: "web",
    label: "Web Scan",
    score: result.overallScore,
    issueCount,
    severityCounts: countWebSeverities(result),
    previousIssueCount,
    previousScore,
    ...delta,
    detail: previous
      ? "Compared with the previous web scan for this site."
      : "First web scan saved for this site.",
    note: null,
  };
}

function buildCodeSection(
  result: CodeScanResult,
  codeHistory: CodeScanSummary[],
): ScanSummarySectionModel {
  const previous = findPreviousCodeSummary(result, codeHistory);
  const previousIssueCount = previous?.issueCount ?? null;
  const previousScore = previous?.overallScore ?? null;
  const delta = buildDelta(
    result.issueCount,
    result.overallScore,
    previousIssueCount,
    previousScore,
  );

  return {
    kind: "code",
    label: "Code Scan",
    score: result.overallScore,
    issueCount: result.issueCount,
    severityCounts: countCodeSeverities(result),
    previousIssueCount,
    previousScore,
    ...delta,
    detail: previous
      ? "Compared with the previous code scan for this project."
      : "First code scan saved for this project.",
    note: buildSkippedScopeNote(result.skippedScopes),
  };
}

function countMultiSeverities(result: MultiScanResult): ScanSummarySeverityCounts {
  return result.pageResults.reduce(
    (counts, page) =>
      addSeverityCounts(
        counts,
        createSeverityCounts({
          critical: page.issuesCritical,
          high: page.issuesHigh,
          medium: page.issuesMedium,
          low: page.issuesLow,
        }),
      ),
    createSeverityCounts(),
  );
}

function buildMultiSection(
  result: MultiScanResult,
  sessions: ScanSessionSummary[],
): ScanSummarySectionModel {
  const issueCount = result.pageResults.reduce((sum, page) => sum + page.issuesCount, 0);
  const previous = findPreviousSessionSummary(result, sessions);
  const previousIssueCount = previous
    ? previous.pageScans.reduce((sum, page) => sum + page.issuesTotal, 0)
    : null;
  const previousScore = previous?.overallScore ?? null;
  const scoreDelta = previousScore == null ? null : result.overallScore - previousScore;

  const siteFindings = countActionableCheckResults(result.siteIssues ?? []);

  return {
    kind: "multi",
    label: "Page Scan",
    score: result.overallScore,
    issueCount,
    severityCounts: countMultiSeverities(result),
    previousIssueCount,
    previousScore,
    // Use backend group deltas, never page-occurrence deltas.
    issueDelta: result.newIssueCount ?? null,
    resolvedCount: result.resolvedIssueCount ?? null,
    scoreDelta,
    detail:
      siteFindings > 0
        ? `${result.completedPages} of ${result.totalPages} pages completed. ${siteFindings} site-wide finding${siteFindings === 1 ? "" : "s"} across pages (shown in the session results and your issue list).`
        : `${result.completedPages} of ${result.totalPages} pages completed.`,
    note: null,
  };
}

function buildSummaryId(input: BuildScanSummaryModelInput): string {
  const parts = [
    input.result ? `web:${input.result.timestamp}:${input.result.overallScore}` : null,
    input.codeResult
      ? `code:${input.codeResult.id}:${input.codeResult.overallScore}:${input.codeResult.issueCount}`
      : null,
    input.multiResult
      ? `multi:${input.multiResult.sessionId}:${input.multiResult.overallScore}:${input.multiResult.completedPages}`
      : null,
  ].filter(Boolean);
  return parts.join("|");
}

function buildTitle(sections: ScanSummarySectionModel[]): string {
  const hasWeb = sections.some((section) => section.kind === "web" || section.kind === "multi");
  const hasCode = sections.some((section) => section.kind === "code");
  if (hasWeb && hasCode) return "Full scan complete";
  if (hasCode) return "Code scan complete";
  if (sections.some((section) => section.kind === "multi")) return "Page scan complete";
  return "Web scan complete";
}

function primarySiteCmdScore(input: BuildScanSummaryModelInput): number | null {
  if (input.sitecmdScore != null && Number.isFinite(input.sitecmdScore)) {
    return Math.round(input.sitecmdScore);
  }
  return null;
}

export function buildScanSummaryModel(input: BuildScanSummaryModelInput): ScanSummaryModel | null {
  const sections: ScanSummarySectionModel[] = [];
  if (input.result) sections.push(buildWebSection(input.result, input.history));
  if (input.multiResult) sections.push(buildMultiSection(input.multiResult, input.sessions));
  if (input.codeResult) sections.push(buildCodeSection(input.codeResult, input.codeHistory));

  if (sections.length === 0) return null;

  // Canonical grouped counts keep the overview, score, and Issues list aligned.
  const inactiveCheckIds = input.inactiveCheckIds ?? EMPTY_INACTIVE_CHECK_IDS;
  const webIssues = input.result
    ? input.result.issues
        .filter(isActionableCheckResult)
        .filter((issue) => !inactiveCheckIds.has(issue.checkId))
    : [];
  const codeIssues = (input.codeResult?.issues ?? []).filter(
    (issue) => !inactiveCheckIds.has(issue.checkId),
  );
  // Prefer persisted canonical counts; raw results are a no-projection fallback.
  const summary =
    input.persistedSummary ??
    buildProjectIssueSummary({ webIssues, codeIssues, codeSummaryFallback: null });

  // Multi-page occurrences never add to the canonical grouped total.
  const multiSection = sections.find((section) => section.kind === "multi");
  const hasPersistedSummary = input.persistedSummary != null;
  const totalIssues = hasPersistedSummary
    ? summary.totalCount
    : summary.totalCount + (multiSection?.issueCount ?? 0);
  const severityCounts = hasPersistedSummary
    ? summary.severityCounts
    : addSeverityCounts(
        summary.severityCounts,
        multiSection?.severityCounts ?? createSeverityCounts(),
      );

  // Restate the web/code section counts with the deduped numbers so the
  // breakdown sums to the headline total.
  const reconciledSections = sections.map((section) =>
    section.kind === "web"
      ? { ...section, issueCount: summary.webCount }
      : section.kind === "code"
        ? { ...section, issueCount: summary.codeCount }
        : section,
  );

  const estimatedNewValues = reconciledSections
    .map((section) => section.issueDelta)
    .filter((value): value is number => value != null)
    .map((value) => Math.max(0, value));
  const resolvedValues = reconciledSections
    .map((section) => section.resolvedCount)
    .filter((value): value is number => value != null);
  const regressionCount = reconciledSections.filter(
    (section) => (section.scoreDelta ?? 0) < 0,
  ).length;

  // Hoist the skipped-scope note (attached to the code section only) to the
  // top level so the overlay can render it without exposing per-source rows.
  const note = reconciledSections.find((section) => section.kind === "code")?.note ?? null;

  return {
    id: buildSummaryId(input),
    title: buildTitle(reconciledSections),
    scopeLabel:
      formatUrlHost(input.result?.url) ||
      formatUrlHost(input.codeResult?.environmentUrl) ||
      formatUrlHost(input.multiResult?.pageResults[0]?.url) ||
      input.scopeLabel,
    siteCmdScore: primarySiteCmdScore(input),
    totalIssues,
    severityCounts,
    estimatedNewIssues:
      estimatedNewValues.length > 0
        ? estimatedNewValues.reduce((sum, value) => sum + value, 0)
        : null,
    resolvedIssues:
      resolvedValues.length > 0 ? resolvedValues.reduce((sum, value) => sum + value, 0) : null,
    regressionCount,
    note,
  };
}
