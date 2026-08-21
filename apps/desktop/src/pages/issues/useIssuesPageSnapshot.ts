import { useMemo } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { useDashboardData } from "@/components/dashboard/useDashboardData";
import { peekDashboardSnapshot } from "@/lib/project-summary-signals";
import { EMPTY_PROJECT_WORK_SUMMARY } from "@/lib/project-work-summary";
import type { CodeScanResult, ScanResult } from "@/lib/types";
import { buildUpdateQueueSummary } from "@/lib/update-summary";

interface UseIssuesPageSnapshotArgs {
  latestCodeResult: CodeScanResult | null;
  latestResult: ScanResult | null;
  projectId: number;
  projectPath: string | null;
  url: string;
}

export function useIssuesPageSnapshot({
  latestCodeResult,
  latestResult,
  projectId,
  projectPath,
  url,
}: UseIssuesPageSnapshotArgs) {
  const dashboardData = useDashboardData({
    url,
    projectId,
    projectPath,
    latestResult,
    latestCodeResult,
    includeReferenceSignals: false,
  });
  const queryClient = useQueryClient();
  const cachedDashboardSnapshot = useMemo(
    () => peekDashboardSnapshot(queryClient, projectId, url),
    [projectId, queryClient, url],
  );
  // Cached data only bridges the initial gap; a loaded empty snapshot is authoritative.
  const useCachedSnapshot = !dashboardData.snapshotHydrated && cachedDashboardSnapshot != null;
  const cachedSignals = useCachedSnapshot ? cachedDashboardSnapshot.signals : undefined;

  const effectiveLatestDetail = useCachedSnapshot
    ? (cachedDashboardSnapshot?.latestDetail ?? null)
    : (dashboardData.latestDetail ?? null);
  const effectiveLatestScanId = useCachedSnapshot
    ? (cachedDashboardSnapshot?.latestScanId ?? null)
    : (dashboardData.latestScanId ?? null);
  const effectiveTrend = useCachedSnapshot
    ? (cachedDashboardSnapshot?.trend ?? [])
    : (dashboardData.trend ?? []);
  const effectiveCodeTrend = useCachedSnapshot
    ? (cachedDashboardSnapshot?.codeTrend ?? [])
    : (dashboardData.codeTrend ?? []);
  const effectiveAggregatedFailedIssues = useMemo(
    () =>
      useCachedSnapshot
        ? (cachedDashboardSnapshot?.aggregatedFailedIssues ?? [])
        : dashboardData.aggregatedFailedIssues,
    [useCachedSnapshot, cachedDashboardSnapshot, dashboardData.aggregatedFailedIssues],
  );
  const effectiveAllUpdates = useMemo(
    () => (useCachedSnapshot ? (cachedSignals?.updates?.updates ?? []) : dashboardData.allUpdates),
    [useCachedSnapshot, cachedSignals, dashboardData.allUpdates],
  );
  const effectiveSecurityUpdates = useMemo(() => {
    if (!useCachedSnapshot) return dashboardData.securityUpdates;
    return buildUpdateQueueSummary(effectiveAllUpdates).securityUpdates;
  }, [useCachedSnapshot, dashboardData.securityUpdates, effectiveAllUpdates]);
  const effectiveLatestCodeScanSummary = useCachedSnapshot
    ? (cachedSignals?.codeScanSummary ?? null)
    : dashboardData.latestCodeScanSummary;
  const effectiveLatestCodeScanDetail = useCachedSnapshot
    ? (cachedSignals?.codeScanDetail ?? null)
    : dashboardData.latestCodeScanDetail;
  const effectiveIssueLinks = useCachedSnapshot
    ? (cachedDashboardSnapshot?.issueLinks ?? [])
    : dashboardData.issueLinks;
  const effectiveWorkQueue = useCachedSnapshot
    ? (cachedDashboardSnapshot?.workQueue ?? dashboardData.workQueue)
    : dashboardData.workQueue;
  const liveWorkSummary = dashboardData.workSummary ?? EMPTY_PROJECT_WORK_SUMMARY;
  const effectiveWorkSummary = useCachedSnapshot
    ? (cachedSignals?.workSummary ?? liveWorkSummary)
    : liveWorkSummary;
  const effectiveDismissedProjectId = useCachedSnapshot
    ? projectId
    : dashboardData.dismissedProjectId;
  const effectiveDismissedIds = useMemo(
    () =>
      useCachedSnapshot
        ? new Set(cachedDashboardSnapshot?.inactiveCheckIds ?? [])
        : dashboardData.dismissedIds,
    [useCachedSnapshot, cachedDashboardSnapshot, dashboardData.dismissedIds],
  );

  return {
    dashboardLoadError: dashboardData.dashboardLoadError,
    dashboardReady: dashboardData.dashboardReady,
    effectiveAggregatedFailedIssues,
    effectiveAllUpdates,
    effectiveDismissedIds,
    effectiveDismissedProjectId,
    effectiveIssueLinks,
    effectiveLatestCodeScanDetail,
    effectiveLatestCodeScanSummary,
    effectiveLatestDetail,
    effectiveLatestScanId,
    effectiveCodeTrend,
    effectiveTrend,
    effectiveSecurityUpdates,
    effectiveWorkQueue,
    effectiveWorkSummary,
    issuesSnapshotReady: dashboardData.dashboardReady || cachedDashboardSnapshot != null,
    lastCIRun: dashboardData.lastCIRun,
    refreshDashboard: dashboardData.refreshDashboard,
  };
}
