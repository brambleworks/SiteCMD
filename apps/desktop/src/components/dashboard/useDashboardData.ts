import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { dismissFirstScanBanner as dismissFirstScanBannerCmd } from "@/lib/commands";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import type { ScanResult, CodeScanResult } from "@/lib/types";
import {
  getDashboardReferenceSignals,
  getDashboardSnapshot,
  invalidateLatestCodeScanSnapshot,
  invalidateProjectSignalSnapshot,
  peekDashboardReferenceSignals,
  peekDashboardSnapshot,
} from "@/lib/project-summary-signals";
import {
  matchesProjectSignalsChangedEvent,
  PROJECT_SIGNALS_CHANGED_EVENT,
} from "@/lib/project-signal-events";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { getRecentPendingProjectUpdates, readUpdateSnapshot } from "@/lib/update-memory";
import { createDashboardDataStateFromSnapshot, dashboardDataReducer } from "./dashboard-data-state";
import { useDashboardCodeScanDetail } from "./useDashboardCodeScanDetail";
import { useDashboardRecentEvents } from "./useDashboardRecentEvents";
import { useDashboardDerivedState } from "./useDashboardDerivedState";
import { useDashboardSignalArming } from "./useDashboardSignalArming";
import { invalidateDashboardSslProbe, useDashboardSslProbe } from "./useDashboardSslProbe";

export interface PlausibleSummary {
  visitors: number;
  pageviews: number;
  bounce_rate: number;
  visit_duration: number;
}

export interface CloudflareSummary {
  requests_total: number;
  cache_hit_rate: number;
  bandwidth_total: number;
  threats_blocked: number;
}

interface UptimeMonitorSummary {
  status: number;
  status_text: string;
  uptime_ratio: number;
  average_response: number;
}

export interface UptimeRobotSummary {
  monitors: UptimeMonitorSummary[];
}

type DashboardRuntimeState = {
  dashboardReady: boolean;
  probesRefreshing: boolean;
};

type DashboardRuntimeAction =
  | { type: "readyChanged"; ready: boolean }
  | { type: "probesRefreshingChanged"; refreshing: boolean }
  | { type: "refreshStarted" };

function dashboardRuntimeReducer(
  state: DashboardRuntimeState,
  action: DashboardRuntimeAction,
): DashboardRuntimeState {
  switch (action.type) {
    case "readyChanged":
      return { ...state, dashboardReady: action.ready };
    case "probesRefreshingChanged":
      return { ...state, probesRefreshing: action.refreshing };
    case "refreshStarted":
      return { dashboardReady: false, probesRefreshing: false };
    default:
      return state;
  }
}

function readStoredUpdates(projectPath: string | null) {
  if (!projectPath) return null;
  const snapshotUpdates = readUpdateSnapshot(projectPath);
  if (snapshotUpdates) return snapshotUpdates;
  const recentPendingUpdates = getRecentPendingProjectUpdates(projectPath);
  return recentPendingUpdates.length > 0 ? recentPendingUpdates : null;
}

export function useDashboardData({
  url,
  projectId,
  projectPath,
  latestResult,
  latestCodeResult,
  includeReferenceSignals = false,
}: {
  url: string;
  projectId: number;
  projectPath: string | null;
  latestResult: ScanResult | null;
  latestCodeResult: CodeScanResult | null;
  includeReferenceSignals?: boolean;
}) {
  const queryClient = useQueryClient();
  // Lazy initialization reads cached reducer seeds once per mount.
  const [initialDashboardBootstrap] = useState(() => {
    const cachedSnapshot = peekDashboardSnapshot(queryClient, projectId, url);
    return {
      dashboardState: createDashboardDataStateFromSnapshot(
        cachedSnapshot,
        projectId,
        includeReferenceSignals ? peekDashboardReferenceSignals(queryClient, projectId, url) : null,
        readStoredUpdates(projectPath),
      ),
      hasSnapshot: cachedSnapshot != null,
    };
  });
  const hasInitialDashboardSnapshot = initialDashboardBootstrap.hasSnapshot;
  const [dashboardState, dashboardDispatch] = useReducer(
    dashboardDataReducer,
    initialDashboardBootstrap.dashboardState,
  );
  const {
    trend,
    codeTrend,
    latestDetail,
    previousDetail,
    latestScanId,
    securityUpdates,
    allUpdates,
    integrations,
    configuredIntegrations,
    lastCIRun,
    commitsSinceLastScan,
    issueLinks,
    aggregatedCheckCounts,
    aggregatedFailedIssues,
    psiReport,
    dashboardLoadError,
    snapshotHydrated,
    dismissedIds,
    dismissedProjectId,
    latestCodeScanSummary,
    previousCodeScanSummary,
    latestCodeScanDetail,
    updatesCheckedAt,
    searchRegression,
    integrationFailureCount,
    staleIntegrationCount,
    firstScanBannerDismissed,
    workQueue,
    workSummary,
    referenceSignalsLoading,
  } = dashboardState;
  const [{ dashboardReady, probesRefreshing }, runtimeDispatch] = useReducer(
    dashboardRuntimeReducer,
    undefined,
    () => ({
      dashboardReady: hasInitialDashboardSnapshot,
      probesRefreshing: false,
    }),
  );
  const loadVersionRef = useRef(0);
  const { recentEvents, recentEventsLoading, loadRecentEvents, loadUpdateEvents, updateEvents } =
    useDashboardRecentEvents({
      includeReferenceSignals,
      projectId,
    });
  const { auxiliarySignalsArmed, disarmSignals, referenceSignalsArmed } = useDashboardSignalArming({
    dashboardReady,
    includeReferenceSignals,
    projectId,
    url,
  });
  const sslProbe = useDashboardSslProbe({
    auxiliarySignalsArmed,
    includeReferenceSignals,
    url,
  });

  const applyDashboardSnapshot = useCallback(
    (snapshot: Awaited<ReturnType<typeof getDashboardSnapshot>>) => {
      dashboardDispatch({
        type: "snapshotLoaded",
        snapshot,
        projectId,
        fallbackUpdates: readStoredUpdates(projectPath),
      });
    },
    [projectId, projectPath],
  );

  const resetDashboardState = useCallback(
    (options?: { referenceSignalsLoading?: boolean }) => {
      dashboardDispatch({
        type: "reset",
        projectId,
        referenceSignalsLoading: options?.referenceSignalsLoading,
      });
    },
    [projectId],
  );

  const hydrateCachedReferenceSignals = useCallback(
    (options?: { includePsi?: boolean }) => {
      if (!includeReferenceSignals) return false;
      const includePsi = options?.includePsi ?? false;
      const signals = peekDashboardReferenceSignals(queryClient, projectId, url, { includePsi });
      if (!signals) return false;
      dashboardDispatch({
        type: "referenceSignalsLoaded",
        signals,
        includePsi: includePsi || Boolean(signals.psiReport),
      });
      return true;
    },
    [includeReferenceSignals, projectId, queryClient, url],
  );

  const loadDashboardSnapshot = useCallback(
    async (options?: { bypassCache?: boolean; forceRefresh?: boolean }) => {
      const epoch = loadVersionRef.current;
      try {
        const snapshot = await getDashboardSnapshot(queryClient, projectId, url, {
          bypassCache: options?.bypassCache,
          forceRefresh: options?.forceRefresh,
        });
        if (loadVersionRef.current !== epoch) {
          return;
        }
        applyDashboardSnapshot(snapshot);
      } catch {
        if (loadVersionRef.current !== epoch) return;
        dashboardDispatch({ type: "snapshotFailed", message: "Issues could not load right now." });
      }
    },
    [applyDashboardSnapshot, projectId, queryClient, url],
  );

  const loadReferenceSignals = useCallback(
    async (options?: { includePsi?: boolean; bypassArm?: boolean; bypassCache?: boolean }) => {
      if (!includeReferenceSignals) return;
      if (!referenceSignalsArmed && !options?.bypassArm) return;
      const includePsi = options?.includePsi ?? false;
      if (!options?.bypassCache && hydrateCachedReferenceSignals({ includePsi })) return;
      const epoch = loadVersionRef.current;
      dashboardDispatch({ type: "referenceSignalsStarted" });
      try {
        const referenceSignals = await getDashboardReferenceSignals(queryClient, projectId, url, {
          includePsi,
          bypassCache: options?.bypassCache,
        });
        if (loadVersionRef.current !== epoch) return;
        dashboardDispatch({
          type: "referenceSignalsLoaded",
          signals: referenceSignals,
          includePsi,
        });
      } catch {
        if (loadVersionRef.current !== epoch) return;
        dashboardDispatch({
          type: "referenceSignalsFailed",
          includePsi,
        });
      }
    },
    [
      hydrateCachedReferenceSignals,
      includeReferenceSignals,
      projectId,
      queryClient,
      referenceSignalsArmed,
      url,
    ],
  );

  useEffect(() => {
    const version = ++loadVersionRef.current;
    const cachedSnapshot = peekDashboardSnapshot(queryClient, projectId, url);
    const hasCachedReferenceSignals = includeReferenceSignals
      ? Boolean(peekDashboardReferenceSignals(queryClient, projectId, url))
      : false;

    disarmSignals();
    resetDashboardState({
      referenceSignalsLoading: includeReferenceSignals && !hasCachedReferenceSignals,
    });
    hydrateCachedReferenceSignals();
    if (cachedSnapshot) {
      applyDashboardSnapshot(cachedSnapshot);
      runtimeDispatch({ type: "readyChanged", ready: true });
    } else {
      runtimeDispatch({ type: "readyChanged", ready: false });
    }

    Promise.allSettled([loadDashboardSnapshot()]).then(() => {
      if (loadVersionRef.current !== version) return;
      runtimeDispatch({ type: "readyChanged", ready: true });
      if (includeReferenceSignals) {
        void loadRecentEvents();
        void loadUpdateEvents();
      }
    });
  }, [
    url,
    projectId,
    queryClient,
    applyDashboardSnapshot,
    disarmSignals,
    loadDashboardSnapshot,
    loadUpdateEvents,
    loadRecentEvents,
    hydrateCachedReferenceSignals,
    includeReferenceSignals,
    resetDashboardState,
  ]);

  useEffect(() => {
    if (!includeReferenceSignals) {
      dashboardDispatch({ type: "referenceSignalsFailed", includePsi: true });
      return;
    }
    if (!referenceSignalsArmed) return;
    void loadReferenceSignals();
  }, [includeReferenceSignals, loadReferenceSignals, referenceSignalsArmed]);

  useEffect(() => {
    if (!latestResult) return;
    const scannedUrl = normalizeAppUrlForKey(latestResult.url);
    const currentUrl = normalizeAppUrlForKey(url);
    if (scannedUrl !== currentUrl) return;

    const version = loadVersionRef.current;
    runtimeDispatch({ type: "readyChanged", ready: false });
    runtimeDispatch({ type: "probesRefreshingChanged", refreshing: true });
    // A completed scan means the cached snapshot is stale by definition; the
    // snapshot cache TTL is long (freshness is event-driven), so bypass it.
    Promise.allSettled([loadDashboardSnapshot({ bypassCache: true })]).then(() => {
      if (loadVersionRef.current !== version) return;
      runtimeDispatch({ type: "readyChanged", ready: true });
      runtimeDispatch({ type: "probesRefreshingChanged", refreshing: false });
      if (includeReferenceSignals) {
        void loadRecentEvents({ force: true });
        void loadUpdateEvents({ force: true });
      }
    });
  }, [
    includeReferenceSignals,
    latestResult,
    projectId,
    url,
    loadDashboardSnapshot,
    loadUpdateEvents,
    loadReferenceSignals,
    loadRecentEvents,
  ]);

  // Mirror Code Scan completions into the dashboard's current report state.
  useEffect(() => {
    if (!latestCodeResult) return;
    if (latestCodeResult.projectId !== projectId) return;
    const version = loadVersionRef.current;
    runtimeDispatch({ type: "readyChanged", ready: false });
    // Same as the web-scan mirror above: fresh scan, stale cache, bypass it.
    Promise.allSettled([loadDashboardSnapshot({ bypassCache: true })]).then(() => {
      if (loadVersionRef.current !== version) return;
      runtimeDispatch({ type: "readyChanged", ready: true });
      if (includeReferenceSignals) {
        void loadRecentEvents({ force: true });
        void loadUpdateEvents({ force: true });
      }
    });
  }, [
    latestCodeResult,
    projectId,
    url,
    includeReferenceSignals,
    loadDashboardSnapshot,
    loadUpdateEvents,
    loadRecentEvents,
  ]);

  useTauriEvent(PROJECT_SIGNALS_CHANGED_EVENT, async (payload) => {
    if (!matchesProjectSignalsChangedEvent(payload, { projectId, url })) return;
    await loadDashboardSnapshot({ bypassCache: true });
    if (includeReferenceSignals) {
      await loadReferenceSignals({ bypassArm: true, bypassCache: true });
    }
  });

  // Score events rehydrate the mounted dashboard after background writes.
  useTauriEvent("site-score-changed", async (payload) => {
    if (payload?.projectId != null && payload.projectId !== projectId) return;
    await loadDashboardSnapshot({ bypassCache: true });
  });

  const effectiveCodeScanDetail = useDashboardCodeScanDetail({
    latestCodeScanDetail,
    latestCodeScanSummary,
  });
  const {
    bootstrapTasks,
    criticalCodeIssues,
    criticalRollup,
    criticalWebIssues,
    highWebIssues,
    verdict,
  } = useDashboardDerivedState({
    aggregatedFailedIssues,
    configuredIntegrations,
    effectiveCodeScanDetail,
    integrationFailureCount,
    lastCIRun,
    latestCodeScanSummary,
    projectPath,
    searchRegression,
    securityUpdates,
    sslProbe,
    staleIntegrationCount,
  });

  const refreshDashboard = () => {
    const version = ++loadVersionRef.current;
    disarmSignals();
    runtimeDispatch({ type: "refreshStarted" });
    resetDashboardState({ referenceSignalsLoading: includeReferenceSignals });

    invalidateProjectSignalSnapshot(queryClient, projectId, url);
    invalidateLatestCodeScanSnapshot(projectId);
    invalidateDashboardSslProbe(queryClient, url);

    void loadReferenceSignals({ includePsi: true, bypassArm: true, bypassCache: true });
    void loadRecentEvents({ force: true });
    void loadUpdateEvents({ force: true });

    Promise.allSettled([loadDashboardSnapshot({ forceRefresh: true })]).then(() => {
      if (loadVersionRef.current === version) {
        runtimeDispatch({ type: "readyChanged", ready: true });
      }
    });
  };

  const dismissFirstScanBanner = async () => {
    dashboardDispatch({ type: "dismissFirstScanBanner" });
    try {
      await dismissFirstScanBannerCmd({ projectId });
    } catch {
      dashboardDispatch({ type: "restoreFirstScanBanner" });
    }
  };

  return {
    trend,
    codeTrend,
    latestDetail,
    previousDetail,
    aggregatedCheckCounts,
    aggregatedFailedIssues,
    latestScanId,
    securityUpdates,
    allUpdates,
    integrations,
    configuredIntegrations,
    lastCIRun,
    commitsSinceLastScan,
    issueLinks,
    psiReport,
    dashboardReady,
    dashboardLoadError,
    snapshotHydrated,
    dismissedIds,
    dismissedProjectId,
    latestCodeScanSummary,
    previousCodeScanSummary,
    latestCodeScanDetail: effectiveCodeScanDetail,
    recentEvents,
    updateEvents,
    recentEventsLoading,
    updatesCheckedAt,
    searchRegression,
    integrationFailureCount,
    staleIntegrationCount,
    firstScanBannerDismissed,
    workQueue,
    workSummary,
    probesRefreshing,
    referenceSignalsLoading,
    dismissFirstScanBanner,
    refreshDashboard,
    sslProbe,
    verdict,
    criticalRollup,
    bootstrapTasks,
    criticalWebIssues,
    criticalCodeIssues,
    highWebIssues,
  };
}
