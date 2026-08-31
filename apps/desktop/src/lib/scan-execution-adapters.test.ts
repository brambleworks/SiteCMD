import { beforeEach, describe, expect, it, vi } from "vitest";

const { getScanExecutionsMock } = vi.hoisted(() => ({
  getScanExecutionsMock: vi.fn(),
}));

vi.mock("@/lib/commands", () => ({
  getScanExecutionDetail: vi.fn(),
  getScanExecutions: getScanExecutionsMock,
}));

import type {
  NormalizedFinding,
  NormalizedRunDiagnostics,
  ScanExecutionDetail,
  ScanExecutionSummary,
  ScanRunDetail,
  ScanRunSummary,
} from "@/generated/ipc-bindings";
import {
  codeResultFromExecutionRun,
  deriveScanPresentationHistory,
  getScanHistory,
  getSessionHistory,
  webResultFromExecutionRun,
} from "./scan-execution-adapters";

beforeEach(() => {
  getScanExecutionsMock.mockReset();
  getScanExecutionsMock.mockResolvedValue([]);
});

const diagnostics: NormalizedRunDiagnostics = {
  mode: null,
  focus: null,
  securityScore: null,
  performanceScore: null,
  seoScore: null,
  accessibilityScore: null,
  complianceScore: null,
  configScore: null,
  polishScore: null,
  detectedStack: null,
  pageUrl: null,
  projectPath: null,
  framework: null,
  codeCommitSha: null,
  codeTreeClean: null,
  totalPages: null,
  completedPages: null,
  axeEnabled: null,
  browserRan: null,
  axeRan: null,
  browserBuild: null,
};

function runSummary(overrides: Partial<ScanRunSummary>): ScanRunSummary {
  return {
    id: 10,
    parentRunId: null,
    source: "web_scan",
    runKind: "single",
    status: "complete",
    timestamp: "2026-07-21T14:00:00Z",
    rawScore: 91,
    durationMs: 120,
    issuesTotal: 1,
    issuesCritical: 0,
    issuesHigh: 1,
    issuesMedium: 0,
    issuesLow: 0,
    diagnostics,
    ...overrides,
  };
}

function executionSummary(overrides: Partial<ScanExecutionSummary> = {}): ScanExecutionSummary {
  return {
    id: 1,
    projectId: 42,
    environmentId: 7,
    environmentUrl: "https://example.com",
    requestedMode: "full",
    webFocus: "health",
    trigger: "manual",
    status: "complete",
    startedAt: 1_753_107_200_000,
    completedAt: 1_753_107_201_000,
    score: 88,
    criticalCount: 0,
    highCount: 2,
    mediumCount: 0,
    lowCount: 0,
    webStatus: "complete",
    webDetail: null,
    codeStatus: "complete",
    codeDetail: null,
    webScanId: 10,
    webSessionId: null,
    webPageCount: 1,
    codeScanId: 11,
    runs: [
      runSummary({ id: 10 }),
      runSummary({
        id: 11,
        source: "code_scan",
        runKind: "code",
        rawScore: 84,
        diagnostics: { ...diagnostics, framework: "Astro" },
      }),
    ],
    ...overrides,
  };
}

function finding(overrides: Partial<NormalizedFinding> = {}): NormalizedFinding {
  return {
    occurrenceId: "occurrence-1",
    source: "code_scan",
    canonicalCheckId: "code_scan.public-endpoint-rate-limit",
    producerCheckId: "public-endpoint-rate-limit",
    producerCategory: "security",
    category: "security",
    domain: "security",
    verdict: "fail",
    severity: "high",
    confidence: "confirmed",
    confidenceReason: null,
    title: "Public endpoint has no rate limit",
    description: "The route is externally reachable without an explicit limit.",
    fixPrompt: null,
    producerFixPrompt: "Add a bounded rate limiter.",
    manualFix: null,
    whyItMatters: "The endpoint can be abused.",
    verificationHint: "Re-run this rule for the target file.",
    rawData: null,
    detailJson: JSON.stringify({
      id: "public-endpoint-rate-limit:src/api.ts",
      absolutePath: "/workspace/src/api.ts",
      sourceExcerpt: "export async function POST() {}",
      evidence: "No limiter was found.",
    }),
    locationKind: "file",
    pageUrl: null,
    relativePath: "src/api.ts",
    line: 12,
    ...overrides,
  };
}

function runDetail(overrides: Partial<ScanRunDetail> = {}): ScanRunDetail {
  return {
    ...runSummary({}),
    startedAt: 1_753_107_200_000,
    completedAt: 1_753_107_201_000,
    coverage: {
      kind: "project",
      successful: true,
      pageUrls: [],
      checks: [],
      exceptions: [],
    },
    statusDetail: null,
    detailState: "full",
    findings: [],
    ...overrides,
  };
}

describe("deriveScanPresentationHistory", () => {
  it("projects Full children for comparisons while preserving one execution record", () => {
    const execution = executionSummary();
    const presentation = deriveScanPresentationHistory([execution]);

    expect(presentation.history).toHaveLength(1);
    expect(presentation.history[0]?.id).toBe(10);
    expect(presentation.codeHistory).toHaveLength(1);
    expect(presentation.codeHistory[0]).toMatchObject({
      id: 11,
      projectId: 42,
      environmentUrl: "https://example.com",
      framework: "Astro",
    });
    expect(execution.requestedMode).toBe("full");
  });

  it("builds one multi-page session from its parent and page children", () => {
    const parent = runSummary({
      id: 20,
      runKind: "multi_parent",
      rawScore: 80,
      diagnostics: { ...diagnostics, totalPages: 2, completedPages: 2 },
    });
    const pages = [
      runSummary({ id: 21, parentRunId: 20, runKind: "page" }),
      runSummary({ id: 22, parentRunId: 20, runKind: "page" }),
    ];
    const presentation = deriveScanPresentationHistory([
      executionSummary({ requestedMode: "web", codeScanId: null, runs: [parent, ...pages] }),
    ]);

    expect(presentation.sessions).toEqual([
      expect.objectContaining({ sessionId: 20, totalPages: 2, completedPages: 2 }),
    ]);
    expect(presentation.sessions[0]?.pageScans.map((page) => page.id)).toEqual([21, 22]);
  });
});

describe("execution detail adapters", () => {
  it("keeps Code producer identity, canonical identity, and location separate", () => {
    const run = runDetail({
      id: 11,
      source: "code_scan",
      runKind: "code",
      rawScore: 84,
      diagnostics: { ...diagnostics, framework: "Astro" },
      findings: [finding()],
    });
    const detail: ScanExecutionDetail = {
      summary: executionSummary(),
      runs: [run],
    };

    const result = codeResultFromExecutionRun(detail, run);

    expect(result?.issues[0]).toMatchObject({
      id: "public-endpoint-rate-limit:src/api.ts",
      producerRuleId: "public-endpoint-rate-limit",
      checkId: "code_scan.public-endpoint-rate-limit",
      relativePath: "src/api.ts",
      absolutePath: "/workspace/src/api.ts",
      line: 12,
    });
    expect(result?.issues[0]?.checkId).not.toContain(":");
  });

  it("preserves distinct Code occurrence IDs for sibling files sharing one rule", () => {
    const run = runDetail({
      id: 11,
      source: "code_scan",
      runKind: "code",
      findings: [
        finding(),
        finding({
          occurrenceId: "occurrence-2",
          relativePath: "src/worker.ts",
          line: 24,
          detailJson: JSON.stringify({
            id: "public-endpoint-rate-limit:src/worker.ts",
            absolutePath: "/workspace/src/worker.ts",
          }),
        }),
      ],
    });
    const detail: ScanExecutionDetail = {
      summary: executionSummary(),
      runs: [run],
    };

    const result = codeResultFromExecutionRun(detail, run);

    expect(result?.issues.map((issue) => issue.id)).toEqual([
      "public-endpoint-rate-limit:src/api.ts",
      "public-endpoint-rate-limit:src/worker.ts",
    ]);
    expect(new Set(result?.issues.map((issue) => issue.id)).size).toBe(2);
  });

  it("reconstructs Web categories and source-native check IDs from normalized findings", () => {
    const webFinding = finding({
      source: "web_scan",
      canonicalCheckId: "security.hsts",
      producerCheckId: "hsts",
      producerCategory: "security",
      category: "security",
      domain: null,
      rawData: JSON.stringify({ header: "strict-transport-security" }),
      detailJson: null,
      locationKind: "page",
      pageUrl: "https://example.com",
      relativePath: null,
      line: null,
    });
    const run = runDetail({
      source: "web_scan",
      runKind: "single",
      diagnostics: { ...diagnostics, securityScore: 72, pageUrl: "https://example.com" },
      findings: [webFinding],
    });
    const detail: ScanExecutionDetail = {
      summary: executionSummary(),
      runs: [run],
    };

    const result = webResultFromExecutionRun(detail, run);

    expect(result.categories).toEqual([
      expect.objectContaining({ category: "security", score: 72, issuesHigh: 1 }),
    ]);
    expect(result.issues[0]).toMatchObject({
      checkId: "hsts",
      rawData: { header: "strict-transport-security" },
    });
  });
});

describe("execution history adapters", () => {
  it("scopes Web history to the selected project as well as its environment URL", async () => {
    await getScanHistory({ projectId: 42, url: "https://example.com", limit: 7 });

    expect(getScanExecutionsMock).toHaveBeenCalledWith({
      projectId: 42,
      environmentUrl: "https://example.com",
      runKind: "single",
      limit: 7,
    });
  });

  it("scopes multi-page history to the selected project as well as its environment URL", async () => {
    await getSessionHistory({ projectId: 42, url: "https://example.com", limit: 5 });

    expect(getScanExecutionsMock).toHaveBeenCalledWith({
      projectId: 42,
      environmentUrl: "https://example.com",
      runKind: "multi_parent",
      limit: 5,
    });
  });
});
