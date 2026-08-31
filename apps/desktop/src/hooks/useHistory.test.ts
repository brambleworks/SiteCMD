import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { withQueryClient } from "@/test-utils/query-client";
import type {
  NormalizedRunDiagnostics,
  ScanExecutionSummary,
  ScanRunSummary,
} from "@/generated/ipc-bindings";

const commandMocks = vi.hoisted(() => ({
  getScanExecutions: vi.fn(),
}));

vi.mock("@/lib/commands", () => commandMocks);

import { useHistory } from "./useHistory";

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

function runSummary(id: number, source: "web_scan" | "code_scan"): ScanRunSummary {
  return {
    id,
    parentRunId: null,
    source,
    runKind: source === "code_scan" ? "code" : "single",
    status: "complete",
    timestamp: "2026-07-21T12:00:00Z",
    rawScore: source === "code_scan" ? 78 : 92,
    durationMs: 900,
    issuesTotal: 1,
    issuesCritical: 0,
    issuesHigh: 1,
    issuesMedium: 0,
    issuesLow: 0,
    diagnostics,
  };
}

function execution(
  runs: ScanRunSummary[],
  overrides: Partial<ScanExecutionSummary> = {},
): ScanExecutionSummary {
  return {
    id: 10,
    projectId: 8,
    environmentId: null,
    environmentUrl: null,
    requestedMode: runs.some((run) => run.source === "code_scan") ? "code" : "web",
    webFocus: null,
    trigger: "manual",
    status: "complete",
    startedAt: Date.parse("2026-07-21T12:00:00Z"),
    completedAt: Date.parse("2026-07-21T12:00:01Z"),
    score: 78,
    criticalCount: 0,
    highCount: 1,
    mediumCount: 0,
    lowCount: 0,
    webStatus: runs.some((run) => run.source === "web_scan") ? "complete" : null,
    webDetail: null,
    codeStatus: runs.some((run) => run.source === "code_scan") ? "complete" : null,
    codeDetail: null,
    webScanId: runs.find((run) => run.source === "web_scan")?.id ?? null,
    webSessionId: null,
    webPageCount: runs.filter((run) => run.source === "web_scan").length,
    codeScanId: runs.find((run) => run.source === "code_scan")?.id ?? null,
    runs,
    ...overrides,
  };
}

describe("useHistory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commandMocks.getScanExecutions.mockResolvedValue([]);
  });

  it("loads canonical Code history for a project that has no site URL", async () => {
    const codeExecution = execution([runSummary(24, "code_scan")]);
    commandMocks.getScanExecutions.mockResolvedValue([codeExecution]);
    const { result } = renderHook(() => useHistory(), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadHistory("", 8);
    });

    expect(commandMocks.getScanExecutions).toHaveBeenCalledWith({
      projectId: 8,
      environmentUrl: null,
      limit: 20,
    });
    expect(result.current.executions).toEqual([codeExecution]);
    expect(result.current.codeHistory).toMatchObject([{ id: 24, projectId: 8 }]);
    expect(result.current.historyError).toBeNull();
  });

  it("uses one execution query instead of source-specific history requests", async () => {
    const fullExecution = execution([runSummary(41, "web_scan"), runSummary(42, "code_scan")], {
      requestedMode: "full",
      environmentUrl: "https://example.com",
    });
    commandMocks.getScanExecutions.mockResolvedValue([fullExecution]);
    const { result } = renderHook(() => useHistory(), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadHistory("https://example.com", 8);
    });

    expect(commandMocks.getScanExecutions).toHaveBeenCalledTimes(1);
    expect(result.current.history).toHaveLength(1);
    expect(result.current.codeHistory).toHaveLength(1);
  });

  it("short-circuits when neither a URL nor a project is known", async () => {
    const { result } = renderHook(() => useHistory(), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadHistory("", undefined);
    });

    expect(commandMocks.getScanExecutions).not.toHaveBeenCalled();
    expect(result.current.loading).toBe(false);
  });

  it("does not present partial source history when the canonical read fails", async () => {
    commandMocks.getScanExecutions.mockRejectedValue(new Error("database unavailable"));
    const { result } = renderHook(() => useHistory(), { wrapper: withQueryClient() });

    await act(async () => {
      await result.current.loadHistory("https://example.com", 7);
    });

    expect(result.current.executions).toEqual([]);
    expect(result.current.history).toEqual([]);
    expect(result.current.sessions).toEqual([]);
    expect(result.current.codeHistory).toEqual([]);
    await waitFor(() => {
      expect(result.current.historyError).toBe("Scan history could not load.");
    });
    expect(result.current.loading).toBe(false);
  });
});
