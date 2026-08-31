import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import type { MultiScanResult } from "@/hooks/useScan";
import type { ScanResult } from "@/lib/types";

const {
  loadCurrentScoreSnapshotMock,
  fetchInactiveKeysMock,
  getProjectNavBadgeSnapshotMock,
  buildProjectIssueSummaryFromSnapshotMock,
  buildScanSummaryModelMock,
} = vi.hoisted(() => ({
  loadCurrentScoreSnapshotMock: vi.fn(),
  fetchInactiveKeysMock: vi.fn(),
  getProjectNavBadgeSnapshotMock: vi.fn(),
  buildProjectIssueSummaryFromSnapshotMock: vi.fn(),
  buildScanSummaryModelMock: vi.fn(),
}));

vi.mock("@/lib/current-score", () => ({
  loadCurrentScoreSnapshot: (...args: unknown[]) => loadCurrentScoreSnapshotMock(...args),
}));

vi.mock("@/pages/issues/useInactiveIssueKeys", () => ({
  fetchInactiveKeys: (...args: unknown[]) => fetchInactiveKeysMock(...args),
}));

vi.mock("@/lib/project-summary-signals", () => ({
  getProjectNavBadgeSnapshot: (...args: unknown[]) => getProjectNavBadgeSnapshotMock(...args),
}));

vi.mock("@/lib/project-nav-badges", () => ({
  buildProjectIssueSummaryFromSnapshot: (...args: unknown[]) =>
    buildProjectIssueSummaryFromSnapshotMock(...args),
}));

vi.mock("@/components/scan/scan-summary-model", () => ({
  buildScanSummaryModel: (...args: unknown[]) => buildScanSummaryModelMock(...args),
}));

import { usePostScanSummary } from "./usePostScanSummary";

type PostScanSummaryParams = Parameters<typeof usePostScanSummary>[0];

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

const multiResult: MultiScanResult = {
  sessionId: 5,
  totalPages: 3,
  completedPages: 3,
  overallScore: 80,
  durationMs: 5_000,
  incompleteDetail: null,
  newIssueCount: null,
  resolvedIssueCount: null,
  pageResults: [],
  siteIssues: [],
};

function buildParams(overrides: Partial<PostScanSummaryParams> = {}): PostScanSummaryParams {
  return {
    state: "complete",
    currentExecutionMode: "web",
    result: webResult,
    codeResult: null,
    multiResult: null,
    executionIncompleteDetail: null,
    activeProjectId: 1,
    activeEnvUrl: "https://example.com",
    activeScanScope: "Example • example.com",
    fullScanStillRunning: false,
    scanBackgrounded: false,
    codeResultFromBackground: false,
    history: [],
    codeHistory: [],
    sessions: [],
    showScanConfig: false,
    ...overrides,
  };
}

function renderSummaryHook(overrides: Partial<PostScanSummaryParams> = {}) {
  const queryClient = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return renderHook((props: PostScanSummaryParams) => usePostScanSummary(props), {
    wrapper,
    initialProps: buildParams(overrides),
  });
}

function lastSummaryModelInput() {
  const call = buildScanSummaryModelMock.mock.calls.at(-1);
  expect(call).toBeDefined();
  return call?.[0] as Record<string, unknown>;
}

describe("usePostScanSummary", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loadCurrentScoreSnapshotMock.mockResolvedValue({ overall: 88 });
    fetchInactiveKeysMock.mockResolvedValue(new Set(["inactive-check"]));
    getProjectNavBadgeSnapshotMock.mockResolvedValue({ activeTotal: 3 });
    buildProjectIssueSummaryFromSnapshotMock.mockReturnValue({ totalCount: 3 });
    buildScanSummaryModelMock.mockReturnValue({ id: "summary-1" });
  });

  it("does not load a score or build a summary while the scan is still running", () => {
    const { result } = renderSummaryHook({ state: "scanning" });

    expect(loadCurrentScoreSnapshotMock).not.toHaveBeenCalled();
    expect(result.current.scanSummary).toBeNull();
    expect(result.current.showScanSummary).toBe(false);
  });

  it("loads the persisted score and builds the summary when a scan completes", async () => {
    const { result } = renderSummaryHook();

    await waitFor(() => expect(result.current.scanSummary).not.toBeNull());

    expect(loadCurrentScoreSnapshotMock).toHaveBeenCalledWith(1, "https://example.com");
    const input = lastSummaryModelInput();
    expect(input.sitecmdScore).toBe(88);
    expect(input.inactiveCheckIds).toEqual(new Set(["inactive-check"]));
    expect(input.persistedSummary).toEqual({ totalCount: 3 });
    expect(input.scopeLabel).toBe("Example • example.com");
    expect(result.current.showScanSummary).toBe(true);
  });

  it("passes incomplete execution context into the summary model", async () => {
    renderSummaryHook({
      executionIncompleteDetail: "Web Scan: Browser analysis failed: unavailable",
    });

    await waitFor(() => expect(buildScanSummaryModelMock).toHaveBeenCalled());
    expect(lastSummaryModelInput().incompleteDetail).toBe(
      "Web Scan: Browser analysis failed: unavailable",
    );
  });

  it("still builds the summary without a score when the persisted-score load fails", async () => {
    loadCurrentScoreSnapshotMock.mockRejectedValue(new Error("no snapshot"));

    const { result } = renderSummaryHook();

    await waitFor(() => expect(result.current.scanSummary).not.toBeNull());
    const input = lastSummaryModelInput();
    expect(input.sitecmdScore).toBeNull();
    expect(input.persistedSummary).toBeNull();
    expect(result.current.showScanSummary).toBe(true);
  });

  it("holds the summary back until the persisted score matches the current scan", async () => {
    loadCurrentScoreSnapshotMock.mockReturnValue(new Promise(() => {}));

    const { result } = renderSummaryHook();

    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.scanSummary).toBeNull();
    expect(result.current.showScanSummary).toBe(false);
  });

  it("ignores a stale score resolution after the scan key changed", async () => {
    let resolveFirst: (value: { overall: number }) => void = () => {};
    loadCurrentScoreSnapshotMock
      .mockImplementationOnce(() => new Promise((resolve) => (resolveFirst = resolve)))
      .mockResolvedValueOnce({ overall: 42 });

    const { result, rerender } = renderSummaryHook();
    rerender(buildParams({ result: { ...webResult, timestamp: "2026-05-07T00:00:00Z" } }));

    await waitFor(() => expect(result.current.scanSummary).not.toBeNull());
    await act(async () => {
      resolveFirst({ overall: 99 });
      await Promise.resolve();
    });

    // A stale resolution cannot replace or clear the current summary.
    expect(lastSummaryModelInput().sitecmdScore).toBe(42);
    expect(result.current.scanSummary).not.toBeNull();
  });

  it("dismisses the overlay via closeScanSummary without discarding the model", async () => {
    const { result } = renderSummaryHook();
    await waitFor(() => expect(result.current.showScanSummary).toBe(true));

    act(() => result.current.closeScanSummary());

    expect(result.current.showScanSummary).toBe(false);
    expect(result.current.scanSummary).not.toBeNull();
  });

  it("skips the score load entirely for backgrounded scans", () => {
    const { result } = renderSummaryHook({ scanBackgrounded: true });

    expect(loadCurrentScoreSnapshotMock).not.toHaveBeenCalled();
    expect(result.current.scanSummary).toBeNull();
  });

  it("keeps the overlay hidden while the scan config overlay is open", async () => {
    const { result } = renderSummaryHook({ showScanConfig: true });

    await waitFor(() => expect(result.current.scanSummary).not.toBeNull());
    expect(result.current.showScanSummary).toBe(false);
  });

  it("loads the canonical persisted totals before showing a multi-page summary", async () => {
    const { result } = renderSummaryHook({ result: null, multiResult });

    await waitFor(() => expect(result.current.scanSummary).not.toBeNull());

    expect(loadCurrentScoreSnapshotMock).toHaveBeenCalledWith(1, "https://example.com");
    expect(lastSummaryModelInput().sitecmdScore).toBe(88);
    expect(lastSummaryModelInput().persistedSummary).toEqual({ totalCount: 3 });
    expect(result.current.showScanSummary).toBe(true);
  });

  it("does not re-announce the overlay when a background execution refreshes the code report", async () => {
    const codeResult = {
      id: 49,
      projectId: 1,
      environmentUrl: "https://example.com",
      overallScore: 90,
      issueCount: 0,
    } as unknown as PostScanSummaryParams["codeResult"];

    const { result } = renderSummaryHook({
      result: null,
      codeResult,
      codeResultFromBackground: true,
    });

    await waitFor(() => expect(result.current.scanSummary).not.toBeNull());
    expect(result.current.showScanSummary).toBe(false);
  });

  it("still announces the overlay for a code report the user scanned themselves", async () => {
    const codeResult = {
      id: 49,
      projectId: 1,
      environmentUrl: "https://example.com",
      overallScore: 90,
      issueCount: 0,
    } as unknown as PostScanSummaryParams["codeResult"];

    const { result } = renderSummaryHook({
      result: null,
      codeResult,
      codeResultFromBackground: false,
    });

    await waitFor(() => expect(result.current.scanSummary).not.toBeNull());
    expect(result.current.showScanSummary).toBe(true);
  });

  it("does not substitute per-page occurrences when the persisted multi-page total fails", async () => {
    getProjectNavBadgeSnapshotMock.mockRejectedValue(new Error("snapshot unavailable"));

    const { result } = renderSummaryHook({ result: null, multiResult });

    await waitFor(() => expect(getProjectNavBadgeSnapshotMock).toHaveBeenCalled());
    await act(async () => {
      await Promise.resolve();
    });

    expect(buildScanSummaryModelMock).not.toHaveBeenCalled();
    expect(result.current.scanSummary).toBeNull();
    expect(result.current.showScanSummary).toBe(false);
  });
});
