import {
  act,
  renderHook as rtlRenderHook,
  waitFor,
  type RenderHookOptions,
  type RenderHookResult,
} from "@testing-library/react";
import { withQueryClient } from "@/test-utils/query-client";

function renderHook<Result, Props>(
  cb: (props: Props) => Result,
  options?: RenderHookOptions<Props>,
): RenderHookResult<Result, Props> {
  return rtlRenderHook(cb, { wrapper: withQueryClient(), ...options });
}
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DashboardSnapshot } from "@/lib/project-summary-signals";
import { EMPTY_PROJECT_WORK_SUMMARY } from "@/lib/project-work-summary";
import type { CheckResult, ScanResult } from "@/lib/types";

const {
  getDashboardSnapshotMock,
  getDashboardReferenceSignalsMock,
  invalidateLatestCodeScanSnapshotMock,
  invalidateProjectSignalSnapshotMock,
  peekDashboardReferenceSignalsMock,
  peekDashboardSnapshotMock,
  readUpdateSnapshotMock,
  getRecentPendingProjectUpdatesMock,
  safeListenMock,
  invokeMock,
} = vi.hoisted(() => ({
  getDashboardSnapshotMock: vi.fn(),
  getDashboardReferenceSignalsMock: vi.fn(),
  invalidateLatestCodeScanSnapshotMock: vi.fn(),
  invalidateProjectSignalSnapshotMock: vi.fn(),
  peekDashboardReferenceSignalsMock: vi.fn(),
  peekDashboardSnapshotMock: vi.fn(),
  readUpdateSnapshotMock: vi.fn(),
  getRecentPendingProjectUpdatesMock: vi.fn(),
  safeListenMock: vi.fn(async (..._args: unknown[]) => () => {}),
  invokeMock: vi.fn(),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: () => false,
  }),
}));

vi.mock("@/lib/project-summary-signals", () => ({
  getDashboardReferenceSignals: getDashboardReferenceSignalsMock,
  getDashboardSnapshot: getDashboardSnapshotMock,
  invalidateLatestCodeScanSnapshot: invalidateLatestCodeScanSnapshotMock,
  invalidateProjectSignalSnapshot: invalidateProjectSignalSnapshotMock,
  peekDashboardReferenceSignals: peekDashboardReferenceSignalsMock,
  peekDashboardSnapshot: peekDashboardSnapshotMock,
}));

vi.mock("@/lib/tauri-events", () => ({
  safeListen: safeListenMock,
}));

vi.mock("@/lib/update-memory", () => ({
  readUpdateSnapshot: readUpdateSnapshotMock,
  getRecentPendingProjectUpdates: getRecentPendingProjectUpdatesMock,
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: invokeMock,
}));

import { useIssuesPageSnapshot } from "./useIssuesPageSnapshot";

const PROJECT_ID = 7;
const URL = "https://example.com";

function failedIssue(checkId: string): CheckResult {
  return {
    checkId: checkId,
    category: "security",
    title: `Issue ${checkId}`,
    description: "Broken",
    status: "fail",
    severity: "high",
    fixPrompt: null,
    manualFix: null,
    rawData: null,
    confidence: "high",
  };
}

function dashboardSnapshot(options?: {
  issues?: CheckResult[];
  inactiveCheckIds?: string[];
}): DashboardSnapshot {
  const issues = options?.issues ?? [];
  return {
    projectId: PROJECT_ID,
    environmentUrl: URL,
    trend: [],
    codeTrend: [],
    latestScanId: null,
    latestDetail: null,
    previousDetail: null,
    aggregatedCheckCounts: { passed: 0, total: issues.length, failed: issues.length },
    aggregatedFailedIssues: issues,
    commitsSinceLastScan: [],
    issueLinks: [],
    inactiveCheckIds: options?.inactiveCheckIds ?? [],
    workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
    signals: {
      projectId: PROJECT_ID,
      environmentUrl: URL,
      firstScanBannerDismissed: true,
      codeScanSummary: null,
      previousCodeScanSummary: null,
      codeScanDetail: null,
      monitoring: {
        enabledIntegrations: [],
        integrationFailureCount: 0,
        staleIntegrationCount: 0,
        searchRegression: null,
      },
      monitoringRefreshedAt: null,
      updates: null,
      updatesRefreshedAt: null,
      targets: {
        securityIssueId: null,
        securityFocus: null,
      },
      workSummary: EMPTY_PROJECT_WORK_SUMMARY,
    },
  };
}

function scanResult(): ScanResult {
  return {
    url: URL,
    mode: "live",
    scanType: "health",
    overallScore: 100,
    categories: [],
    issues: [],
    detectedStack: null,
    durationMs: 900,
    timestamp: "2026-07-05T12:00:00Z",
  };
}

type SnapshotHookProps = Parameters<typeof useIssuesPageSnapshot>[0];

function hookProps(overrides?: Partial<SnapshotHookProps>): SnapshotHookProps {
  return {
    latestCodeResult: null,
    latestResult: null,
    projectId: PROJECT_ID,
    projectPath: null,
    url: URL,
    ...overrides,
  };
}

describe("useIssuesPageSnapshot", () => {
  beforeEach(() => {
    getDashboardSnapshotMock.mockReset();
    getDashboardReferenceSignalsMock.mockReset();
    invalidateLatestCodeScanSnapshotMock.mockReset();
    invalidateProjectSignalSnapshotMock.mockReset();
    peekDashboardReferenceSignalsMock.mockReset();
    peekDashboardSnapshotMock.mockReset();
    readUpdateSnapshotMock.mockReset();
    getRecentPendingProjectUpdatesMock.mockReset();
    safeListenMock.mockReset();
    invokeMock.mockReset();
    safeListenMock.mockReturnValue(Promise.resolve(() => {}));
    peekDashboardReferenceSignalsMock.mockReturnValue(null);
    peekDashboardSnapshotMock.mockReturnValue(null);
    readUpdateSnapshotMock.mockReturnValue(null);
    getRecentPendingProjectUpdatesMock.mockReturnValue([]);
    invokeMock.mockResolvedValue([]);
  });

  it("renders cached issues instantly while the live snapshot is still loading", async () => {
    peekDashboardSnapshotMock.mockReturnValue(
      dashboardSnapshot({
        issues: [failedIssue("security.csp"), failedIssue("seo.title"), failedIssue("perf.lcp")],
      }),
    );
    getDashboardSnapshotMock.mockImplementation(() => new Promise(() => {}));

    const { result } = renderHook(() => useIssuesPageSnapshot(hookProps()));

    expect(result.current.issuesSnapshotReady).toBe(true);
    expect(result.current.effectiveAggregatedFailedIssues.map((issue) => issue.checkId)).toEqual([
      "security.csp",
      "seo.title",
      "perf.lcp",
    ]);

    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.effectiveAggregatedFailedIssues).toHaveLength(3);
  });

  it("returns live empty data once a fresh snapshot lands, not the stale cached issues", async () => {
    peekDashboardSnapshotMock.mockReturnValue(
      dashboardSnapshot({
        issues: [failedIssue("security.csp"), failedIssue("seo.title"), failedIssue("perf.lcp")],
        inactiveCheckIds: ["polish.favicon"],
      }),
    );
    let resolveLiveSnapshot: ((snapshot: DashboardSnapshot) => void) | null = null;
    getDashboardSnapshotMock.mockImplementation(
      () =>
        new Promise<DashboardSnapshot>((resolve) => {
          resolveLiveSnapshot = resolve;
        }),
    );

    const { result } = renderHook(() => useIssuesPageSnapshot(hookProps()));

    expect(result.current.effectiveAggregatedFailedIssues).toHaveLength(3);
    expect(result.current.effectiveDismissedIds.has("polish.favicon")).toBe(true);

    await act(async () => {
      resolveLiveSnapshot?.(dashboardSnapshot({ issues: [], inactiveCheckIds: [] }));
      await Promise.resolve();
    });

    await waitFor(() => expect(result.current.effectiveAggregatedFailedIssues).toHaveLength(0));
    // A user with zero local dismissals must not inherit cached inactive ids.
    expect(result.current.effectiveDismissedIds.size).toBe(0);
    expect(result.current.dashboardReady).toBe(true);
  });

  it("drops already-fixed issues when a scan completes while the page is mounted", async () => {
    const cachedSnapshot = dashboardSnapshot({
      issues: [failedIssue("security.csp"), failedIssue("seo.title"), failedIssue("perf.lcp")],
    });
    peekDashboardSnapshotMock.mockReturnValue(cachedSnapshot);
    getDashboardSnapshotMock
      .mockResolvedValueOnce(cachedSnapshot)
      .mockResolvedValueOnce(dashboardSnapshot({ issues: [] }));

    const { result, rerender } = renderHook(
      (props: SnapshotHookProps) => useIssuesPageSnapshot(props),
      { initialProps: hookProps() },
    );

    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.effectiveAggregatedFailedIssues).toHaveLength(3);

    rerender(hookProps({ latestResult: scanResult() }));

    await waitFor(() => expect(result.current.effectiveAggregatedFailedIssues).toHaveLength(0));
    expect(result.current.dashboardReady).toBe(true);
  });
});
