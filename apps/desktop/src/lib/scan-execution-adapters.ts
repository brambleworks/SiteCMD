import { getScanExecutionDetail, getScanExecutions } from "@/lib/commands";
import type {
  CategoryScore,
  CheckResult,
  CodeScanDomain,
  CodeScanDomainSummary,
  CodeScanResult,
  CodeScanSummary,
  NormalizedFinding,
  ScanExecutionDetail,
  ScanExecutionSummary,
  ScanResult,
  ScanRunDetail,
  ScanRunSummary,
  ScanSessionSummary,
  ScanSummary,
} from "@/generated/ipc-bindings";

function runToWebSummary(execution: ScanExecutionSummary, run: ScanRunSummary): ScanSummary {
  return {
    id: run.id,
    url: run.diagnostics.pageUrl ?? execution.environmentUrl ?? "",
    mode: run.diagnostics.mode ?? (run.runKind === "multi_parent" ? "multi_page" : "live"),
    scanType: (run.diagnostics.focus ?? execution.webFocus ?? "health") as ScanSummary["scanType"],
    overallScore: run.rawScore ?? 0,
    issuesTotal: run.issuesTotal,
    issuesCritical: run.issuesCritical,
    issuesHigh: run.issuesHigh,
    issuesMedium: run.issuesMedium,
    issuesLow: run.issuesLow,
    durationMs: run.durationMs,
    timestamp: run.timestamp,
    sessionId: run.parentRunId,
    pageUrl: run.diagnostics.pageUrl,
  };
}

export function deriveScanPresentationHistory(executions: ScanExecutionSummary[]) {
  const history: ScanSummary[] = [];
  const sessions: ScanSessionSummary[] = [];
  const codeHistory: CodeScanSummary[] = [];

  for (const execution of executions) {
    for (const run of execution.runs) {
      if (run.source === "web_scan" && run.runKind === "single") {
        history.push(runToWebSummary(execution, run));
      }
      if (run.source === "web_scan" && run.runKind === "multi_parent") {
        const pages = execution.runs
          .filter((candidate) => candidate.parentRunId === run.id && candidate.runKind === "page")
          .map((candidate) => runToWebSummary(execution, candidate));
        sessions.push({
          sessionId: run.id,
          totalPages: run.diagnostics.totalPages ?? pages.length,
          completedPages: run.diagnostics.completedPages ?? pages.length,
          status: run.status,
          startedAt: run.timestamp,
          overallScore: run.rawScore,
          durationMs: run.durationMs,
          pageScans: pages,
        });
      }
      if (
        run.source === "code_scan" &&
        run.runKind === "code" &&
        execution.projectId != null &&
        run.rawScore != null
      ) {
        codeHistory.push({
          id: run.id,
          projectId: execution.projectId,
          environmentUrl: execution.environmentUrl,
          overallScore: run.rawScore,
          issueCount: run.issuesTotal,
          groupedIssueCount: run.issuesTotal,
          criticalCount: run.issuesCritical,
          highCount: run.issuesHigh,
          durationMs: run.durationMs,
          checkedAt: run.timestamp,
          framework: run.diagnostics.framework,
          topDomain: null,
          topDomainCount: 0,
          domainSummaries: [],
        });
      }
    }
  }

  const newestFirst = <T>(items: T[], timestamp: (value: T) => string) =>
    items.sort((left, right) => Date.parse(timestamp(right)) - Date.parse(timestamp(left)));
  return {
    history: newestFirst(history, (scan) => scan.timestamp),
    sessions: newestFirst(sessions, (session) => session.startedAt),
    codeHistory: newestFirst(codeHistory, (scan) => scan.checkedAt),
  };
}

function parseJson(value: string | null): unknown {
  if (value == null) return null;
  try {
    return JSON.parse(value) as unknown;
  } catch {
    return null;
  }
}

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function webCategoryScores(run: ScanRunDetail): CategoryScore[] {
  const diagnostics = run.diagnostics;
  const values = [
    ["security", diagnostics.securityScore],
    ["performance", diagnostics.performanceScore],
    ["seo", diagnostics.seoScore],
    ["accessibility", diagnostics.accessibilityScore],
    ["compliance", diagnostics.complianceScore],
    ["config", diagnostics.configScore],
    ["polish", diagnostics.polishScore],
  ] as const;

  return values.flatMap(([category, score]) => {
    if (score == null) return [];
    const findings = run.findings.filter(
      (finding) => finding.producerCategory === category || finding.category === category,
    );
    const actionable = findings.filter(
      (finding) => finding.verdict === "fail" || finding.verdict === "warn",
    );
    const count = (severity: string) =>
      actionable.filter((finding) => finding.severity === severity).length;
    return [
      {
        category,
        score,
        issuesTotal: actionable.length,
        issuesCritical: count("critical"),
        issuesHigh: count("high"),
        issuesMedium: count("medium"),
        issuesLow: count("low"),
        issuesPassed: findings.filter((finding) => finding.verdict === "pass").length,
      },
    ];
  });
}

function findingToCheckResult(finding: NormalizedFinding): CheckResult {
  return {
    checkId: finding.producerCheckId,
    category: finding.producerCategory as CheckResult["category"],
    title: finding.title,
    description: finding.description,
    status: finding.verdict,
    severity: finding.severity,
    fixPrompt: finding.fixPrompt,
    manualFix: finding.manualFix,
    rawData: parseJson(finding.rawData),
    confidence: finding.confidence,
    confidenceReason: finding.confidenceReason ?? undefined,
    whyItMatters: finding.whyItMatters ?? undefined,
  };
}

export function webResultFromExecutionRun(
  detail: ScanExecutionDetail,
  run: ScanRunDetail,
): ScanResult {
  return {
    url: run.diagnostics.pageUrl ?? detail.summary.environmentUrl ?? "",
    mode: run.diagnostics.mode ?? (run.runKind === "multi_parent" ? "multi_page" : "live"),
    scanType: (run.diagnostics.focus ??
      detail.summary.webFocus ??
      "health") as ScanResult["scanType"],
    overallScore: run.rawScore ?? 0,
    categories: webCategoryScores(run),
    issues: run.findings.map(findingToCheckResult),
    detectedStack: parseJson(run.diagnostics.detectedStack),
    durationMs: run.durationMs,
    timestamp: run.timestamp,
  };
}

const CODE_DOMAINS = new Set<CodeScanDomain>([
  "database",
  "ai-safety",
  "security",
  "architecture",
  "operations",
  "supply-chain",
  "ai-scaffolding",
]);

function codeDomain(finding: NormalizedFinding): CodeScanDomain {
  return finding.domain && CODE_DOMAINS.has(finding.domain as CodeScanDomain)
    ? (finding.domain as CodeScanDomain)
    : "architecture";
}

function codeIssueId(finding: NormalizedFinding, native: Record<string, unknown>): string {
  if (typeof native.id === "string" && native.id.trim()) return native.id;
  if (finding.relativePath) return `${finding.producerCheckId}:${finding.relativePath}`;
  return finding.occurrenceId;
}

function codeDomainSummaries(findings: NormalizedFinding[]): CodeScanDomainSummary[] {
  const grouped = new Map<CodeScanDomain, NormalizedFinding[]>();
  for (const finding of findings) {
    const domain = codeDomain(finding);
    grouped.set(domain, [...(grouped.get(domain) ?? []), finding]);
  }
  return [...grouped.entries()].map(([domain, rows]) => ({
    domain,
    issueCount: rows.length,
    criticalCount: rows.filter((row) => row.severity === "critical").length,
    highCount: rows.filter((row) => row.severity === "high").length,
    mediumCount: rows.filter((row) => row.severity === "medium").length,
    lowCount: rows.filter((row) => row.severity === "low").length,
  }));
}

export function codeResultFromExecutionRun(
  detail: ScanExecutionDetail,
  run: ScanRunDetail,
): CodeScanResult | null {
  const projectId = detail.summary.projectId;
  if (projectId == null) return null;
  const issues = run.findings.map((finding) => {
    const native = asRecord(parseJson(finding.detailJson));
    return {
      id: codeIssueId(finding, native),
      producerRuleId: finding.producerCheckId,
      checkId: finding.canonicalCheckId,
      category: finding.producerCategory,
      domain: codeDomain(finding),
      severity: finding.severity,
      title: finding.title,
      description: finding.description,
      relativePath: finding.relativePath ?? "",
      absolutePath:
        typeof native.absolutePath === "string"
          ? native.absolutePath
          : (finding.relativePath ?? ""),
      line: finding.line,
      sourceExcerpt: typeof native.sourceExcerpt === "string" ? native.sourceExcerpt : null,
      evidence: typeof native.evidence === "string" ? native.evidence : null,
      whyNow: finding.whyItMatters,
      likelyFix: finding.producerFixPrompt,
      confidence: finding.confidence,
      confidenceReason: finding.confidenceReason ?? undefined,
      verifyHint: finding.verificationHint,
    };
  });
  return {
    id: run.id,
    projectId,
    environmentUrl: detail.summary.environmentUrl,
    overallScore: run.rawScore ?? 0,
    issueCount: issues.length,
    criticalCount: issues.filter((issue) => issue.severity === "critical").length,
    highCount: issues.filter((issue) => issue.severity === "high").length,
    mediumCount: issues.filter((issue) => issue.severity === "medium").length,
    lowCount: issues.filter((issue) => issue.severity === "low").length,
    durationMs: run.durationMs,
    checkedAt: run.timestamp,
    framework: run.diagnostics.framework,
    domainSummaries: codeDomainSummaries(run.findings),
    issues,
  };
}

async function loadWebScanHistory(
  projectId: number | null,
  url: string,
  limit = 20,
): Promise<ScanSummary[]> {
  const executions = await getScanExecutions({
    projectId,
    environmentUrl: url,
    runKind: "single",
    limit,
  });
  return deriveScanPresentationHistory(executions).history;
}

async function loadSessionHistory(
  projectId: number | null,
  url: string,
  limit = 20,
): Promise<ScanSessionSummary[]> {
  const executions = await getScanExecutions({
    projectId,
    environmentUrl: url,
    runKind: "multi_parent",
    limit,
  });
  return deriveScanPresentationHistory(executions).sessions;
}

// `runId` narrows the execution in SQL, so the response carries that run and
// no other. Picking it back out here would mean the sibling runs were queried,
// serialized, and parsed for nothing.
async function loadWebScanDetail(runId: number): Promise<ScanResult | null> {
  const detail = await getScanExecutionDetail({ runId });
  const [run] = detail?.runs ?? [];
  return detail && run?.source === "web_scan" ? webResultFromExecutionRun(detail, run) : null;
}

async function loadCodeScanDetail(runId: number): Promise<CodeScanResult | null> {
  const detail = await getScanExecutionDetail({ runId });
  const [run] = detail?.runs ?? [];
  return detail && run?.source === "code_scan" ? codeResultFromExecutionRun(detail, run) : null;
}

// View-model-shaped entry points used by source-specific charts and compare
// panels. The transport remains the two canonical execution commands above.
export const getScanHistory = ({
  projectId,
  url,
  limit,
}: {
  projectId: number | null;
  url: string;
  limit?: number;
}) => loadWebScanHistory(projectId, url, limit);
export const getSessionHistory = ({
  projectId,
  url,
  limit,
}: {
  projectId: number | null;
  url: string;
  limit?: number;
}) => loadSessionHistory(projectId, url, limit);
export const getScanDetail = ({ scanId }: { scanId: number }) => loadWebScanDetail(scanId);
export const getCodeScanDetail = ({ scanId }: { scanId: number }) => loadCodeScanDetail(scanId);
