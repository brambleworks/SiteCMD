import {
  act,
  renderHook as rtlRenderHook,
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
import type { PackageUpdate } from "@/lib/types";

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

function packageUpdate(name: string): PackageUpdate {
  return {
    name,
    currentVersion: "1.0.0",
    latestVersion: "1.1.0",
    ecosystem: "npm",
    updateType: "minor",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "package-lock.json",
    isDev: false,
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    workspaceMembers: [],
  };
}

function dashboardSnapshot(updates: PackageUpdate[] | null): DashboardSnapshot {
  return {
    projectId: 7,
    environmentUrl: "https://example.com",
    trend: [],
    codeTrend: [],
    latestScanId: null,
    latestDetail: null,
    previousDetail: null,
    aggregatedCheckCounts: { passed: 0, total: 0, failed: 0 },
    aggregatedFailedIssues: [],
    commitsSinceLastScan: [],
    issueLinks: [],
    inactiveCheckIds: [],
    workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
    signals: {
      projectId: 7,
      environmentUrl: "https://example.com",
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
      updates: updates
        ? { packages: [], updates, ecosystemsDetected: ["npm"], scanDurationMs: 1 }
        : null,
      updatesRefreshedAt: updates ? "2026-05-19T12:00:00Z" : null,
      targets: {
        securityIssueId: null,
        securityFocus: null,
      },
      workSummary: {
        unresolvedCount: 0,
        newCount: 0,
        workingCount: 0,
        regressedCount: 0,
        ignoredCount: 0,
        blockedCount: 0,
        launchBlockerCount: 0,
        maintenanceCount: 0,
        primaryAction: null,
        regressedAction: null,
        workingAction: null,
        blockedAction: null,
        ignoredAction: null,
        launchBlockerAction: null,
        weeklySummary: null,
      },
    },
  };
}

describe("useDashboardData", () => {
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

  it("hydrates dashboard updates from stored update memory when the snapshot has no update payload", async () => {
    readUpdateSnapshotMock.mockReturnValue([packageUpdate("next")]);
    getDashboardSnapshotMock.mockResolvedValue(dashboardSnapshot(null));

    const { useDashboardData } = await import("./useDashboardData");

    const { result } = renderHook(() =>
      useDashboardData({
        url: "https://example.com",
        projectId: 7,
        projectPath: "/tmp/sitecmd",
        latestResult: null,
        latestCodeResult: null,
        includeReferenceSignals: false,
      }),
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.allUpdates).toHaveLength(1);
    expect(result.current.allUpdates[0]?.name).toBe("next");
  });

  it("runs the bootstrap cache reads once per mount, not on every render", async () => {
    getDashboardSnapshotMock.mockResolvedValue(dashboardSnapshot(null));

    const { useDashboardData } = await import("./useDashboardData");

    const hookProps = {
      url: "https://example.com",
      projectId: 7,
      projectPath: "/tmp/sitecmd",
      latestResult: null,
      latestCodeResult: null,
      includeReferenceSignals: false,
    };
    const { rerender } = renderHook((props: typeof hookProps) => useDashboardData(props), {
      initialProps: hookProps,
    });

    await act(async () => {
      await Promise.resolve();
    });

    const snapshotPeeks = peekDashboardSnapshotMock.mock.calls.length;
    const referencePeeks = peekDashboardReferenceSignalsMock.mock.calls.length;
    const storedUpdateReads = readUpdateSnapshotMock.mock.calls.length;

    rerender({ ...hookProps });
    rerender({ ...hookProps });

    expect(peekDashboardSnapshotMock.mock.calls.length).toBe(snapshotPeeks);
    expect(peekDashboardReferenceSignalsMock.mock.calls.length).toBe(referencePeeks);
    expect(readUpdateSnapshotMock.mock.calls.length).toBe(storedUpdateReads);
  });

  it("bypasses the frontend snapshot cache after project update signals", async () => {
    let projectSignalHandler: ((event: { payload: unknown }) => Promise<void>) | null = null;
    safeListenMock.mockImplementation((...args: unknown[]) => {
      const [eventName, handler] = args as [string, typeof projectSignalHandler];
      if (eventName === "project-signals-changed") {
        projectSignalHandler = handler;
      }
      return Promise.resolve(() => {});
    });
    getDashboardSnapshotMock
      .mockResolvedValueOnce(dashboardSnapshot([]))
      .mockResolvedValueOnce(dashboardSnapshot([packageUpdate("react")]));

    const { useDashboardData } = await import("./useDashboardData");

    const { result } = renderHook(() =>
      useDashboardData({
        url: "https://example.com",
        projectId: 7,
        projectPath: "/tmp/sitecmd",
        latestResult: null,
        latestCodeResult: null,
        includeReferenceSignals: false,
      }),
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.allUpdates).toHaveLength(0);
    expect(projectSignalHandler).not.toBeNull();

    await act(async () => {
      await projectSignalHandler?.({
        payload: {
          projectId: 7,
          url: "https://example.com",
          source: "updates",
          updates: { updates: [packageUpdate("react")] },
        },
      });
    });

    // The snapshot load carries no tier-driven includeCodeScanDetail option
    // any more: code-scan detail is part of every snapshot.
    expect(getDashboardSnapshotMock).toHaveBeenLastCalledWith(
      expect.anything(),
      7,
      "https://example.com",
      {
        bypassCache: true,
        forceRefresh: undefined,
      },
    );
    expect(result.current.allUpdates).toHaveLength(1);
    expect(result.current.allUpdates[0]?.name).toBe("react");
  });

  it("bypasses cached reference signals after project signal changes", async () => {
    let projectSignalHandler: ((event: { payload: unknown }) => Promise<void>) | null = null;
    safeListenMock.mockImplementation((...args: unknown[]) => {
      const [eventName, handler] = args as [string, typeof projectSignalHandler];
      if (eventName === "project-signals-changed") {
        projectSignalHandler = handler;
      }
      return Promise.resolve(() => {});
    });
    getDashboardSnapshotMock.mockResolvedValue(dashboardSnapshot([]));
    getDashboardReferenceSignalsMock.mockResolvedValue({
      integrations: [],
      lastCiRun: null,
      psiReport: null,
    });

    const { useDashboardData } = await import("./useDashboardData");

    renderHook(() =>
      useDashboardData({
        url: "https://example.com",
        projectId: 7,
        projectPath: "/tmp/sitecmd",
        latestResult: null,
        latestCodeResult: null,
        includeReferenceSignals: true,
      }),
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(projectSignalHandler).not.toBeNull();

    await act(async () => {
      await projectSignalHandler?.({
        payload: {
          projectId: 7,
          url: "https://example.com",
          source: "updates",
        },
      });
    });

    expect(getDashboardReferenceSignalsMock).toHaveBeenCalledWith(
      expect.anything(),
      7,
      "https://example.com",
      {
        includePsi: false,
        bypassCache: true,
      },
    );
  });

  it("refreshes a mounted dashboard when the project score changes", async () => {
    let scoreHandler:
      ((event: { payload: { projectId?: number } }) => void | Promise<void>) | null = null;
    safeListenMock.mockImplementation((...args: unknown[]) => {
      const [eventName, handler] = args as [string, typeof scoreHandler];
      if (eventName === "site-score-changed") scoreHandler = handler;
      return Promise.resolve(() => {});
    });
    getDashboardSnapshotMock
      .mockResolvedValueOnce(dashboardSnapshot([]))
      .mockResolvedValueOnce(dashboardSnapshot([packageUpdate("fresh-score-state")]));

    const { useDashboardData } = await import("./useDashboardData");
    const { result } = renderHook(() =>
      useDashboardData({
        url: "https://example.com",
        projectId: 7,
        projectPath: "/tmp/sitecmd",
        latestResult: null,
        latestCodeResult: null,
        includeReferenceSignals: false,
      }),
    );

    await act(async () => {
      await Promise.resolve();
    });
    expect(scoreHandler).not.toBeNull();

    await act(async () => {
      await scoreHandler?.({ payload: { projectId: 99 } });
    });
    expect(getDashboardSnapshotMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      await scoreHandler?.({ payload: { projectId: 7 } });
    });

    expect(getDashboardSnapshotMock).toHaveBeenLastCalledWith(
      expect.anything(),
      7,
      "https://example.com",
      {
        bypassCache: true,
        forceRefresh: undefined,
      },
    );
    expect(result.current.allUpdates[0]?.name).toBe("fresh-score-state");
  });

  it("keeps cached dashboard content visible while a remount refresh is still pending", async () => {
    const cachedSnapshot = {
      projectId: 7,
      environmentUrl: "https://example.com",
      trend: [
        {
          overall: 81,
          security: 78,
          performance: 74,
          seo: 82,
          accessibility: 85,
          compliance: 88,
          config: 79,
          polish: 80,
          timestamp: "2026-04-20T16:08:00Z",
          issues: 3,
          scanType: "health",
        },
      ],
      codeTrend: [],
      latestScanId: 42,
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-20T16:08:00Z",
      },
      previousDetail: null,
      aggregatedCheckCounts: { passed: 9, total: 12, failed: 3 },
      aggregatedFailedIssues: [],
      commitsSinceLastScan: [],
      issueLinks: [],
      inactiveCheckIds: [],
      workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
      signals: {
        firstScanBannerDismissed: true,
        codeScanSummary: null,
        previousCodeScanSummary: null,
        codeScanDetail: null,
        monitoring: {
          enabledIntegrations: ["plausible"],
          integrationFailureCount: 0,
          staleIntegrationCount: 0,
          searchRegression: null,
        },
        monitoringRefreshedAt: null,
        updates: { updates: [], summary: null, checkedAt: null },
        updatesRefreshedAt: null,
        targets: {
          securityIssueId: null,
          securityFocus: null,
        },
        workSummary: {
          unresolvedCount: 3,
          newCount: 1,
          workingCount: 0,
          regressedCount: 0,
          ignoredCount: 0,
          blockedCount: 0,
          launchBlockerCount: 1,
          maintenanceCount: 0,
          primaryAction: null,
          regressedAction: null,
          workingAction: null,
          blockedAction: null,
          ignoredAction: null,
          launchBlockerAction: null,
          weeklySummary: null,
        },
      },
    };

    peekDashboardSnapshotMock.mockReturnValue(cachedSnapshot);
    getDashboardSnapshotMock.mockImplementation(() => new Promise(() => {}));

    const { useDashboardData } = await import("./useDashboardData");

    const { result } = renderHook(() =>
      useDashboardData({
        url: "https://example.com",
        projectId: 7,
        projectPath: null,
        latestResult: null,
        latestCodeResult: null,
        includeReferenceSignals: false,
      }),
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.dashboardReady).toBe(true);
    expect(result.current.latestDetail?.overallScore).toBe(81);
    expect(result.current.workSummary.unresolvedCount).toBe(3);
    expect(result.current.aggregatedCheckCounts.failed).toBe(3);
    expect(getDashboardSnapshotMock).toHaveBeenCalledWith(
      expect.anything(),
      7,
      "https://example.com",
      {
        bypassCache: undefined,
        forceRefresh: undefined,
      },
    );
  });

  it("keeps cached dashboard reference cards visible while a remount refresh is still pending", async () => {
    const cachedSnapshot = {
      projectId: 7,
      environmentUrl: "https://example.com",
      trend: [],
      codeTrend: [],
      latestScanId: null,
      latestDetail: null,
      previousDetail: null,
      aggregatedCheckCounts: { passed: 0, total: 0, failed: 0 },
      aggregatedFailedIssues: [],
      commitsSinceLastScan: [],
      issueLinks: [],
      inactiveCheckIds: [],
      workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
      signals: {
        firstScanBannerDismissed: true,
        codeScanSummary: null,
        previousCodeScanSummary: null,
        codeScanDetail: null,
        monitoring: {
          enabledIntegrations: ["plausible", "cloudflare"],
          integrationFailureCount: 0,
          staleIntegrationCount: 0,
          searchRegression: null,
        },
        monitoringRefreshedAt: null,
        updates: { updates: [], summary: null, checkedAt: null },
        updatesRefreshedAt: null,
        targets: {
          securityIssueId: null,
          securityFocus: null,
        },
        workSummary: {
          unresolvedCount: 0,
          newCount: 0,
          workingCount: 0,
          regressedCount: 0,
          ignoredCount: 0,
          blockedCount: 0,
          launchBlockerCount: 0,
          maintenanceCount: 0,
          primaryAction: null,
          regressedAction: null,
          workingAction: null,
          blockedAction: null,
          ignoredAction: null,
          launchBlockerAction: null,
          weeklySummary: null,
        },
      },
    };
    const cachedReferenceSignals = {
      integrations: [
        {
          integrationType: "plausible",
          data: { visitors: 123, pageviews: 456 },
          fetchedAt: "2026-04-20T16:08:00Z",
          error: null,
        },
        {
          integrationType: "cloudflare",
          data: { requests_total: 1000, cache_hit_rate: 87 },
          fetchedAt: "2026-04-20T16:08:00Z",
          error: null,
        },
      ],
      lastCiRun: {
        name: "Deploy",
        conclusion: "success",
        status: "completed",
        htmlUrl: "https://example.com/actions/1",
        updatedAt: "2026-04-20T16:08:00Z",
      },
      psiReport: null,
    };

    peekDashboardSnapshotMock.mockReturnValue(cachedSnapshot);
    peekDashboardReferenceSignalsMock.mockReturnValue(cachedReferenceSignals);
    getDashboardSnapshotMock.mockImplementation(() => new Promise(() => {}));

    const { useDashboardData } = await import("./useDashboardData");

    const { result } = renderHook(() =>
      useDashboardData({
        url: "https://example.com",
        projectId: 7,
        projectPath: null,
        latestResult: null,
        latestCodeResult: null,
        includeReferenceSignals: true,
      }),
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.integrations).toEqual(cachedReferenceSignals.integrations);
    expect(result.current.lastCIRun).toEqual(cachedReferenceSignals.lastCiRun);
    expect(result.current.referenceSignalsLoading).toBe(false);
    expect(getDashboardReferenceSignalsMock).not.toHaveBeenCalled();
  });
});
