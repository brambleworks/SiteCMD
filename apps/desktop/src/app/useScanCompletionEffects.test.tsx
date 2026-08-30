import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ScanJobContext } from "@/app/useScanShellStatus";
import type { MultiScanResult } from "@/hooks/useScan";
import type { ScanRunStep } from "@/lib/scan-run-status";
import type { CodeScanResult, ScanResult } from "@/lib/types";

const {
  handleWebScanCompletionMock,
  handleCodeScanCompletionMock,
  handleFullScanCompletionMock,
  handleFullMultiScanCompletionMock,
  handleMultiScanCompletionMock,
  completeJobMock,
  failJobMock,
} = vi.hoisted(() => ({
  handleWebScanCompletionMock: vi.fn(),
  handleCodeScanCompletionMock: vi.fn(),
  handleFullScanCompletionMock: vi.fn(),
  handleFullMultiScanCompletionMock: vi.fn(),
  handleMultiScanCompletionMock: vi.fn(),
  completeJobMock: vi.fn(),
  failJobMock: vi.fn(),
}));

vi.mock("@/lib/scan-completion-effects", () => ({
  handleWebScanCompletion: (...args: unknown[]) => handleWebScanCompletionMock(...args),
  handleCodeScanCompletion: (...args: unknown[]) => handleCodeScanCompletionMock(...args),
  handleFullScanCompletion: (...args: unknown[]) => handleFullScanCompletionMock(...args),
  handleFullMultiScanCompletion: (...args: unknown[]) => handleFullMultiScanCompletionMock(...args),
  handleMultiScanCompletion: (...args: unknown[]) => handleMultiScanCompletionMock(...args),
}));

vi.mock("@/lib/jobs", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/jobs")>();
  return {
    ...actual,
    completeJob: (...args: unknown[]) => completeJobMock(...args),
    failJob: (...args: unknown[]) => failJobMock(...args),
  };
});

import { useScanCompletionEffects } from "./useScanCompletionEffects";

const webResult: ScanResult = {
  url: "https://example.com",
  mode: "live",
  scanType: "health",
  overallScore: 82,
  categories: [],
  issues: [],
  detectedStack: null,
  durationMs: 900,
  timestamp: "2026-05-06T00:00:00Z",
};

const codeResult: CodeScanResult = {
  id: 42,
  projectId: 1,
  environmentUrl: "https://example.com",
  overallScore: 76,
  issueCount: 1,
  criticalCount: 0,
  highCount: 1,
  mediumCount: 0,
  lowCount: 0,
  durationMs: 1200,
  checkedAt: "2026-05-06T00:00:01Z",
  framework: "React",
  domainSummaries: [],
  issues: [
    {
      id: "unsafe-query:src/db.ts",
      checkId: "code_scan.unsafe-query",
      category: "security",
      domain: "security",
      severity: "high",
      title: "Unsafe query",
      description: "User input reaches raw SQL.",
      relativePath: "src/db.ts",
      absolutePath: "/tmp/project/src/db.ts",
      line: 12,
      sourceExcerpt: null,
      evidence: null,
      whyNow: null,
      likelyFix: null,
      confidence: "high",
      verifyHint: null,
    },
  ],
};

const multiResult: MultiScanResult = {
  sessionId: 7,
  totalPages: 3,
  completedPages: 3,
  overallScore: 80,
  durationMs: 4200,
  incompleteDetail: null,
  newIssueCount: null,
  resolvedIssueCount: null,
  pageResults: [
    {
      url: "https://example.com",
      score: 80,
      issuesCount: 2,
      issuesCritical: 0,
      issuesHigh: 1,
      issuesMedium: 1,
      issuesLow: 0,
      durationMs: 1400,
      scanId: 101,
    },
  ],
  siteIssues: [],
};

interface RenderCompletionHookOptions {
  scanRunStep: ScanRunStep | null;
  result?: ScanResult | null;
  codeResult?: CodeScanResult | null;
  multiResult?: MultiScanResult | null;
  state?: "complete" | "error";
  error?: string | null;
  executionIncompleteDetail?: string | null;
  codeResultFromBackground?: boolean;
  currentExecutionMode?: "full" | "web" | "code";
  scanJobContext?: ScanJobContext | null;
}

function renderCompletionHook({
  scanRunStep,
  result = webResult,
  codeResult: scanCodeResult = null,
  multiResult: scanMultiResult = null,
  state = "complete",
  error = null,
  executionIncompleteDetail = null,
  codeResultFromBackground = false,
  currentExecutionMode = scanRunStep?.mode ?? "web",
  scanJobContext = null,
}: RenderCompletionHookOptions) {
  const loadHistory = vi.fn();
  const toast = {
    success: vi.fn(),
    error: vi.fn(),
  };
  const rendered = renderHook(() =>
    useScanCompletionEffects({
      state,
      currentScanType: "health",
      currentExecutionMode,
      result,
      codeResult: scanCodeResult,
      multiResult: scanMultiResult,
      error,
      executionIncompleteDetail,
      codeResultFromBackground,
      activeEnvUrl: "https://example.com",
      activeProjectId: 1,
      activeProjectName: "Example",
      activeScanScope: "Example",
      history: [],
      codeHistory: [],
      scanBackgroundedRef: { current: false },
      scanJobContextRef: { current: scanJobContext },
      desktopNotificationsEnabled: false,
      loadHistory,
      openAppTarget: vi.fn(),
      refreshProjects: vi.fn(),
      setScanFollowUpBanner: vi.fn(),
      toast,
    }),
  );
  return { ...rendered, loadHistory, toast };
}

describe("useScanCompletionEffects", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("completes a unified full scan even when its presentation step is stale", () => {
    handleWebScanCompletionMock.mockReset();
    handleCodeScanCompletionMock.mockReset();
    handleFullScanCompletionMock.mockReset();
    handleMultiScanCompletionMock.mockReset();

    renderCompletionHook({
      result: webResult,
      codeResult,
      scanRunStep: {
        mode: "full",
        stepIndex: 1,
        stepCount: 2,
        label: "Web Scan",
      },
    });

    expect(handleFullScanCompletionMock).toHaveBeenCalledTimes(1);
    expect(handleWebScanCompletionMock).not.toHaveBeenCalled();
    expect(handleCodeScanCompletionMock).not.toHaveBeenCalled();
    expect(handleMultiScanCompletionMock).not.toHaveBeenCalled();
  });

  it("uses full scan completion when both scan phases have completed", () => {
    handleWebScanCompletionMock.mockReset();
    handleCodeScanCompletionMock.mockReset();
    handleFullScanCompletionMock.mockReset();
    handleMultiScanCompletionMock.mockReset();

    renderCompletionHook({
      result: webResult,
      codeResult,
      scanRunStep: {
        mode: "full",
        stepIndex: 2,
        stepCount: 2,
        label: "Code Scan",
      },
    });

    expect(handleFullScanCompletionMock).toHaveBeenCalledTimes(1);
    expect(handleWebScanCompletionMock).not.toHaveBeenCalled();
    expect(handleCodeScanCompletionMock).not.toHaveBeenCalled();
    expect(handleMultiScanCompletionMock).not.toHaveBeenCalled();
  });

  it("reports a Full Scan (not a Code Scan) when the web portion was multi-page", () => {
    handleWebScanCompletionMock.mockReset();
    handleCodeScanCompletionMock.mockReset();
    handleFullScanCompletionMock.mockReset();
    handleFullMultiScanCompletionMock.mockReset();
    handleMultiScanCompletionMock.mockReset();

    renderCompletionHook({
      result: null,
      multiResult,
      codeResult,
      scanRunStep: {
        mode: "full",
        stepIndex: 2,
        stepCount: 2,
        label: "Code Scan",
      },
    });

    expect(handleFullMultiScanCompletionMock).toHaveBeenCalledTimes(1);
    expect(handleCodeScanCompletionMock).not.toHaveBeenCalled();
    expect(handleFullScanCompletionMock).not.toHaveBeenCalled();
    expect(handleMultiScanCompletionMock).not.toHaveBeenCalled();
    expect(handleWebScanCompletionMock).not.toHaveBeenCalled();
  });

  it("keeps plain multi-page completion when there is no code result", () => {
    handleFullMultiScanCompletionMock.mockReset();
    handleMultiScanCompletionMock.mockReset();

    renderCompletionHook({
      result: null,
      multiResult,
      codeResult: null,
      scanRunStep: null,
    });

    expect(handleMultiScanCompletionMock).toHaveBeenCalledTimes(1);
    expect(handleFullMultiScanCompletionMock).not.toHaveBeenCalled();
  });

  it("keeps code-only completion for standalone code scans", () => {
    handleWebScanCompletionMock.mockReset();
    handleCodeScanCompletionMock.mockReset();
    handleFullScanCompletionMock.mockReset();
    handleMultiScanCompletionMock.mockReset();

    renderCompletionHook({
      result: null,
      codeResult,
      scanRunStep: {
        mode: "code",
        stepIndex: 1,
        stepCount: 1,
        label: "Code Scan",
      },
    });

    expect(handleCodeScanCompletionMock).toHaveBeenCalledTimes(1);
    expect(handleFullScanCompletionMock).not.toHaveBeenCalled();
    expect(handleWebScanCompletionMock).not.toHaveBeenCalled();
    expect(handleMultiScanCompletionMock).not.toHaveBeenCalled();
  });

  it("runs web completion with the active scan context", () => {
    renderCompletionHook({
      scanRunStep: null,
      scanJobContext: {
        projectId: 1,
        url: "https://example.com",
        scopeLabel: "Example",
      },
    });

    expect(handleWebScanCompletionMock).toHaveBeenCalledTimes(1);
  });

  it("keeps the app-level error toast and jobs-tray failure for normal scan errors", () => {
    const { toast } = renderCompletionHook({
      scanRunStep: null,
      state: "error",
      error: "DNS lookup failed",
      result: null,
    });

    expect(toast.error).toHaveBeenCalledTimes(1);
    expect(failJobMock).toHaveBeenCalledTimes(1);
  });

  it("reports incomplete results without running a successful completion path", () => {
    const { toast } = renderCompletionHook({
      scanRunStep: null,
      executionIncompleteDetail: "Web Scan: Browser analysis failed: unavailable",
    });

    expect(handleWebScanCompletionMock).not.toHaveBeenCalled();
    expect(handleCodeScanCompletionMock).not.toHaveBeenCalled();
    expect(handleFullScanCompletionMock).not.toHaveBeenCalled();
    expect(handleFullMultiScanCompletionMock).not.toHaveBeenCalled();
    expect(handleMultiScanCompletionMock).not.toHaveBeenCalled();
    expect(failJobMock).toHaveBeenCalledWith(
      "scan",
      expect.objectContaining({ detail: "Web Scan: Browser analysis failed: unavailable" }),
    );
    expect(toast.error).toHaveBeenCalledWith(
      "Web Scan completed partially",
      "Web Scan: Browser analysis failed: unavailable",
    );
  });

  it("keeps a partial Full Scan labeled as a Full Scan when one result is missing", () => {
    const { toast } = renderCompletionHook({
      scanRunStep: null,
      currentExecutionMode: "full",
      result: null,
      codeResult,
      executionIncompleteDetail: "Web Scan: Network error: Failed to fetch",
    });

    expect(toast.error).toHaveBeenCalledWith(
      "Full Scan completed partially",
      "Web Scan: Network error: Failed to fetch",
    );
  });

  it("does not replay foreground completion effects for a background code refresh", () => {
    const { toast } = renderCompletionHook({
      scanRunStep: null,
      currentExecutionMode: "full",
      result: webResult,
      codeResult,
      codeResultFromBackground: true,
      executionIncompleteDetail: "Web Scan: Browser analysis failed: unavailable",
    });

    expect(handleWebScanCompletionMock).not.toHaveBeenCalled();
    expect(handleCodeScanCompletionMock).not.toHaveBeenCalled();
    expect(handleFullScanCompletionMock).not.toHaveBeenCalled();
    expect(handleFullMultiScanCompletionMock).not.toHaveBeenCalled();
    expect(handleMultiScanCompletionMock).not.toHaveBeenCalled();
    expect(failJobMock).not.toHaveBeenCalled();
    expect(completeJobMock).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
    expect(toast.success).not.toHaveBeenCalled();
  });
});
