import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useProjectNavBadgeRefresh } from "@/app/useProjectNavBadgeRefresh";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { safeListen } from "@/lib/tauri-events";
import { setUpdatesBadge, useNavBadges } from "@/lib/nav-badges";
import {
  getProjectNavBadgeSnapshot,
  primeProjectUpdatesSnapshot,
} from "@/lib/project-summary-signals";
import type { ProjectNavBadgeSnapshot, ProjectSignalSnapshot } from "@/lib/project-summary-types";
import { getRecentPendingProjectUpdates, readUpdateSnapshot } from "@/lib/update-memory";
import type { PackageUpdate, UpdateReport } from "@/lib/types";

vi.mock("@/lib/project-summary-signals", () => ({
  getProjectNavBadgeSnapshot: vi.fn(),
  primeProjectUpdatesSnapshot: vi.fn(),
}));

vi.mock("@/lib/tauri-events", () => ({
  safeListen: vi.fn(async () => () => {}),
}));

vi.mock("@/lib/update-memory", () => ({
  getRecentPendingProjectUpdates: vi.fn(),
  readUpdateSnapshot: vi.fn(),
}));

function packageUpdate(name: string, isSecurity = false): PackageUpdate {
  return {
    name,
    currentVersion: "1.0.0",
    latestVersion: "1.1.0",
    ecosystem: "npm",
    updateType: "minor",
    isSecurity: isSecurity,
    advisorySeverity: isSecurity ? "high" : null,
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

function updateReport(updates: PackageUpdate[]): UpdateReport {
  return {
    packages: [],
    updates,
    ecosystemsDetected: ["npm"],
    scanDurationMs: 12,
  };
}

function signalSnapshot(updates: UpdateReport | null): ProjectSignalSnapshot {
  return {
    projectId: 7,
    environmentUrl: "https://sitecmd.com",
    firstScanBannerDismissed: false,
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
    updates,
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
  };
}

function navSnapshot(updates: UpdateReport | null): ProjectNavBadgeSnapshot {
  return {
    projectId: 7,
    environmentUrl: "https://sitecmd.com",
    aggregatedFailedIssues: [],
    inactiveCheckIds: [],
    signals: signalSnapshot(updates),
  };
}

describe("useProjectNavBadgeRefresh", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setUpdatesBadge(null);
    vi.mocked(readUpdateSnapshot).mockReturnValue(null);
    vi.mocked(getRecentPendingProjectUpdates).mockReturnValue([]);
    vi.mocked(getProjectNavBadgeSnapshot).mockResolvedValue(navSnapshot(null));
    vi.mocked(primeProjectUpdatesSnapshot).mockReset();
  });

  it("hydrates the updates badge from stored update memory when the backend cache is empty", async () => {
    vi.mocked(readUpdateSnapshot).mockReturnValue([
      packageUpdate("next", true),
      packageUpdate("vite"),
    ]);
    vi.mocked(getProjectNavBadgeSnapshot).mockResolvedValue(navSnapshot(null));

    const badges = renderHook(() => useNavBadges(7));
    renderHook(
      () =>
        useProjectNavBadgeRefresh({
          activeProjectId: 7,
          activeEnvUrl: "https://sitecmd.com",
          activeProjectPath: "/tmp/sitecmd",
          result: null,
          codeResult: null,
        }),
      { wrapper: withQueryClient(createTestQueryClient()) },
    );

    await waitFor(() => {
      expect(badges.result.current.updates).toEqual({ projectId: 7, total: 2, critical: 1 });
    });
    await waitFor(() => {
      expect(getProjectNavBadgeSnapshot).toHaveBeenCalledTimes(1);
    });
    expect(badges.result.current.updates).toEqual({ projectId: 7, total: 2, critical: 1 });
  });

  it("forces a nav-badge refetch when an integration is connected or removed", async () => {
    vi.mocked(getProjectNavBadgeSnapshot).mockResolvedValue(navSnapshot(null));

    renderHook(
      () =>
        useProjectNavBadgeRefresh({
          activeProjectId: 7,
          activeEnvUrl: "https://sitecmd.com",
          activeProjectPath: "/tmp/sitecmd",
          result: null,
          codeResult: null,
        }),
      { wrapper: withQueryClient(createTestQueryClient()) },
    );

    // Let the mount refresh (a non-forced, cache-allowed read) fully settle so
    // it does not swallow the event-driven refresh as an in-flight duplicate.
    await waitFor(() => expect(getProjectNavBadgeSnapshot).toHaveBeenCalledTimes(1));
    await act(async () => {});

    const registration = vi
      .mocked(safeListen)
      .mock.calls.find(([name]) => name === "project-signals-changed");
    expect(registration).toBeDefined();
    const listener = registration![1] as (event: { payload: unknown }) => void;
    await act(async () => {
      listener({ payload: { projectId: 7, url: null, source: "integration" } });
    });

    await waitFor(() => {
      expect(getProjectNavBadgeSnapshot).toHaveBeenCalledWith(
        expect.anything(),
        7,
        "https://sitecmd.com",
        { forceRefresh: true },
      );
    });
  });

  it("lets an explicit empty backend update report clear the stored badge", async () => {
    vi.mocked(readUpdateSnapshot).mockReturnValue([packageUpdate("next", true)]);
    vi.mocked(getProjectNavBadgeSnapshot).mockResolvedValue(navSnapshot(updateReport([])));

    const badges = renderHook(() => useNavBadges(7));
    renderHook(
      () =>
        useProjectNavBadgeRefresh({
          activeProjectId: 7,
          activeEnvUrl: "https://sitecmd.com",
          activeProjectPath: "/tmp/sitecmd",
          result: null,
          codeResult: null,
        }),
      { wrapper: withQueryClient(createTestQueryClient()) },
    );

    await waitFor(() => {
      expect(getProjectNavBadgeSnapshot).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(badges.result.current.updates).toBeNull();
    });
  });
});
