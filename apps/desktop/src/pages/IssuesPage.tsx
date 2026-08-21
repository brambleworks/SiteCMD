import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { reopenIssue } from "@/lib/issues";
import { emitAppEvent } from "@/lib/app-events";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { useRenderSanityCheck } from "@/lib/render-sanity";
import { coerceJsonRecord } from "@/lib/json-record";
import { getIssuesStatusFromFocus, normalizeAppUrlForKey } from "@/lib/app-targets";
import { findUnifiedByCheckId, rankIssueGroups } from "@/lib/issue-ranking";
import { useToast } from "@/hooks/useToast";
import type { ScanConfigPreset } from "@/components/scan/ScanConfigOverlay";
import { useCurrentScore } from "@/hooks/useCurrentScore";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import {
  finishPerformanceTimerAfterPaint,
  startPerformanceTimer,
  type PerformanceTimer,
} from "@/lib/performance-metrics";
import { recordWorkflowHealthEvent } from "@/lib/observability";
import type { ScanResult, CodeScanResult } from "@/lib/types";
import { buildIssueGroupSummary } from "@/lib/project-issue-summary";
import { siteCmdScoreModelFromSnapshot } from "@/lib/sitecmd-score";
import { isCriticalSecurityUpdate } from "@/lib/update-priority";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import { useHistoryContext } from "@/app/history-context";
import { useNavigation } from "@/app/navigation-context";
import { PROJECT_SIGNALS_CHANGED_EVENT } from "@/lib/project-signal-events";
import { IssuesByPagePanel } from "@/pages/issues/IssuesByPagePanel";
import { IssuesHistoryPanel } from "@/pages/issues/IssuesHistoryPanel";
import { useIssuesPageGroups } from "@/pages/issues/useIssuesPageGroups";
import { useIssuesPageSnapshot } from "@/pages/issues/useIssuesPageSnapshot";
import { useIssueStatusResources } from "@/pages/issues/useIssueStatusResources";
import { useInactiveIssueKeys } from "@/pages/issues/useInactiveIssueKeys";
import { IssuesQueuePanel } from "@/pages/issues/IssuesQueuePanel";
import { IssuesScoreStrip } from "@/pages/issues/IssuesScoreStrip";
import { IssuesTabBar } from "@/pages/issues/IssuesTabBar";
import { useIssueDossierStack } from "@/pages/issues/useIssueDossierStack";
import { CompactTrendStrip } from "@/components/dashboard/CompactTrend";
import { buildIssuesTrendModel } from "@/components/dashboard/compact-trend-model";
import { loadIssuesView, saveIssuesView, type IssuesTab } from "@/pages/issues/issues-page-model";
import type { NavTarget } from "@/components/layout/nav-page";
import { errorMessage } from "@/lib/error-message";

const IssueDossier = lazy(() =>
  import("@/components/issues/IssueDossier").then((module) => ({ default: module.IssueDossier })),
);

interface IssuesPageProps {
  projectId: number;
  url: string;
  environmentId?: number;
  latestResult: ScanResult | null;
  latestCodeResult: CodeScanResult | null;
  projectPath: string | null;
  onNavigate: (page: NavTarget) => void;
  openScanConfig: (preset?: ScanConfigPreset) => void;
}

export function IssuesPage({
  projectId,
  url,
  environmentId,
  latestResult,
  latestCodeResult,
  projectPath,
  onNavigate,
  openScanConfig,
}: IssuesPageProps) {
  useRenderSanityCheck("IssuesPage");
  const { issuesTarget, issuesTabResetKey: tabResetKey } = useNavigation();
  const initialFocus = issuesTarget?.focus ?? null;
  const { executions, loadHistory } = useHistoryContext();
  const [activeTab, setActiveTab] = useState<IssuesTab>(loadIssuesView);
  const toast = useToast();
  const { score: currentScore, refresh: refreshCurrentScore } = useCurrentScore(projectId, url);

  const [selectedPageUrl, setSelectedPageUrl] = useState<string | null>(null);
  const [localDismissedIds, setLocalDismissedIds] = useState<Set<string>>(new Set());
  const [restoringPausedCheckId, setRestoringPausedCheckId] = useState<string | null>(null);

  const issuesReadyTimerRef = useRef<PerformanceTimer | null>(null);
  const issuesOpenEventRef = useRef<string | null>(null);

  const {
    dashboardLoadError,
    dashboardReady,
    effectiveAllUpdates,
    effectiveIssueLinks,
    effectiveLatestCodeScanSummary,
    effectiveLatestDetail,
    effectiveLatestScanId,
    effectiveCodeTrend,
    effectiveTrend,
    effectiveSecurityUpdates,
    issuesSnapshotReady,
    lastCIRun,
    refreshDashboard,
  } = useIssuesPageSnapshot({
    url,
    projectId,
    projectPath,
    latestResult,
    latestCodeResult,
  });

  useEffect(() => {
    saveIssuesView(activeTab);
  }, [activeTab]);

  const {
    pageGroups,
    loading: pageGroupsLoading,
    error: pageGroupsError,
    retry: retryPageGroups,
  } = useIssuesPageGroups({ projectId, selectedPageUrl, url });

  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(url), [url]);
  const {
    statusFilter,
    setStatusFilter,
    pausedWorkItems,
    resolvedList,
    resourceLoading: statusResourceLoading,
    resourceError: statusResourceError,
    retryResource: retryStatusResource,
  } = useIssueStatusResources({ projectId, normalizedUrl });

  useEffect(() => {
    if (activeTab !== "issues") return;
    issuesReadyTimerRef.current = startPerformanceTimer("issues.initial_ready_ms", {
      projectId,
      environmentId: environmentId ?? null,
    });
  }, [activeTab, environmentId, normalizedUrl, projectId, tabResetKey]);

  // Scan identity changes clear optimistic dismissals before rendering.
  const scanIdentity = `${effectiveLatestCodeScanSummary?.id ?? ""}:${effectiveLatestScanId ?? ""}`;
  useResetOnChange(scanIdentity, () => setLocalDismissedIds(new Set()));

  // The action bar owns the command; this callback only updates the list.
  const handleDismissIssue = useCallback(
    (checkId: string) => {
      setLocalDismissedIds((prev) => new Set([...prev, checkId]));
      emitAppEvent(PROJECT_SIGNALS_CHANGED_EVENT, {
        projectId,
        url,
        source: "issues",
      });
    },
    [projectId, url],
  );

  // These lifecycle states exclude work items from both the list and score.
  const {
    groups: issueGroups,
    isLoading: issueGroupsLoading,
    isError: inactiveIssueKeysError,
    refetch: refetchInactiveIssueKeys,
  } = useInactiveIssueKeys(projectId, normalizedUrl);

  const visibleIssueGroups = useMemo(
    () => issueGroups.filter((group) => !localDismissedIds.has(group.checkId)),
    [issueGroups, localDismissedIds],
  );
  const rankedIssues = useMemo(() => rankIssueGroups(visibleIssueGroups), [visibleIssueGroups]);

  const updateSummary = useMemo(
    () => buildUpdateQueueSummary(effectiveAllUpdates),
    [effectiveAllUpdates],
  );

  const nonSecurityUpdates = useMemo(() => updateSummary.regularUpdates, [updateSummary]);
  const criticalSecurityUpdates = useMemo(
    () => effectiveSecurityUpdates.filter(isCriticalSecurityUpdate),
    [effectiveSecurityUpdates],
  );

  useEffect(() => {
    if (activeTab !== "issues" || !issuesReadyTimerRef.current) return;
    if (!dashboardReady && !dashboardLoadError) return;
    const status = dashboardLoadError ? "failed" : "succeeded";
    const eventKey = `${status}:${projectId}:${environmentId ?? "none"}:${tabResetKey ?? "default"}`;
    if (issuesOpenEventRef.current !== eventKey) {
      recordWorkflowHealthEvent("open_issues", status, {
        issueCount: rankedIssues.length,
        hasError: Boolean(dashboardLoadError),
      });
      issuesOpenEventRef.current = eventKey;
    }
    finishPerformanceTimerAfterPaint(issuesReadyTimerRef.current, {
      status: dashboardLoadError ? "error" : "ready",
      issueCount: rankedIssues.length,
    });
    issuesReadyTimerRef.current = null;
  }, [
    activeTab,
    dashboardLoadError,
    dashboardReady,
    environmentId,
    rankedIssues.length,
    projectId,
    tabResetKey,
  ]);

  const sitecmdScore = useMemo(
    () => (currentScore ? siteCmdScoreModelFromSnapshot(currentScore) : null),
    [currentScore],
  );

  useEffect(() => {
    if (!latestResult && !latestCodeResult) return;
    void refreshCurrentScore();
  }, [latestCodeResult, latestResult, refreshCurrentScore]);

  // One score event covers every issue lifecycle transition.
  useTauriEvent("site-score-changed", () => {
    void refreshCurrentScore();
  });
  const issueSummary = useMemo(
    () => buildIssueGroupSummary(visibleIssueGroups),
    [visibleIssueGroups],
  );
  const issuesTrend = useMemo(
    () =>
      buildIssuesTrendModel({
        webTrend: effectiveTrend,
        codeTrend: effectiveCodeTrend,
        currentIssueCount: issueSummary.totalCount,
        criticalCount: issueSummary.severityCounts.critical,
      }),
    [
      effectiveCodeTrend,
      effectiveTrend,
      issueSummary.severityCounts.critical,
      issueSummary.totalCount,
    ],
  );
  const scoreStripCheckedAt = newestTimestamp([
    effectiveLatestDetail?.timestamp ?? latestResult?.timestamp ?? null,
    effectiveLatestCodeScanSummary?.checkedAt ?? latestCodeResult?.checkedAt ?? null,
  ]);
  const hasIssueContent = rankedIssues.length > 0;
  const hasCurrentScanData = Boolean(
    effectiveLatestDetail || effectiveLatestCodeScanSummary || latestResult || latestCodeResult,
  );
  const hasHistoricalScanData = executions.length > 0;
  const showInitialIssuesLoading =
    activeTab === "issues" && issueGroupsLoading && !hasCurrentScanData && !hasIssueContent;
  const showIssuesFailure =
    activeTab === "issues" &&
    (inactiveIssueKeysError ||
      (Boolean(dashboardLoadError) && !hasCurrentScanData && !hasIssueContent));
  const showFirstScanEmpty =
    activeTab === "issues" &&
    issuesSnapshotReady &&
    !dashboardLoadError &&
    !hasCurrentScanData &&
    !hasHistoricalScanData &&
    !hasIssueContent;

  const handleMissingCause = useCallback(
    () => toast.info("Not in list", "That related issue is not in your current list."),
    [toast],
  );
  const {
    selectedStack,
    selectedIssue,
    selectIssue,
    closeIssue,
    goBack,
    openCause,
    resetIssueStack,
  } = useIssueDossierStack(rankedIssues, handleMissingCause);

  const handleSelectPageIssue = useCallback(
    (checkId: string) => {
      const match = findUnifiedByCheckId(rankedIssues, checkId);
      if (!match) {
        toast.info(
          "Issue changed",
          "This finding is no longer in the active list. Refresh the page list to update it.",
        );
        return;
      }
      selectIssue(match);
    },
    [rankedIssues, selectIssue, toast],
  );

  const handleOpenIntegrations = useCallback(
    (integration: string) => {
      onNavigate(`integrations:${integration}`);
      resetIssueStack();
    },
    [onNavigate, resetIssueStack],
  );

  // Project changes clear per-project state. Deep links and repeated nav clicks
  // reset only focus and dossier state. Previous keys preserve mount behavior.
  const lastResetKeysRef = useRef<{
    projectKey: string | null;
    initialFocus: string | null | undefined;
    tabResetKey: number | undefined;
  }>({
    projectKey: null,
    initialFocus: undefined,
    tabResetKey: undefined,
  });

  useEffect(() => {
    const prev = lastResetKeysRef.current;
    const projectKey = `${projectId}::${url}`;
    const projectChanged = prev.projectKey !== projectKey;
    const focusChanged = prev.initialFocus !== initialFocus;
    const tabResetChanged = prev.tabResetKey !== tabResetKey;

    if (projectChanged) {
      // Full per-project reset: all selection state belongs to the previous
      // project and would mislead the user if it leaked across.
      setLocalDismissedIds(new Set());
      setStatusFilter("active");
      setSelectedPageUrl(null);
      resetIssueStack();
    }

    // Focus resets preserve user-owned dismissal and grouping state.
    const focusDrivenReset =
      (focusChanged && initialFocus != null && initialFocus !== "") || tabResetChanged;
    if (focusDrivenReset) {
      setStatusFilter(getIssuesStatusFromFocus(initialFocus) ?? "active");
      setActiveTab("issues");
      resetIssueStack();
    }

    lastResetKeysRef.current = { projectKey, initialFocus, tabResetKey };
  }, [projectId, url, initialFocus, tabResetKey, resetIssueStack, setStatusFilter]);

  useEffect(() => {
    if (activeTab !== "history") return;
    void loadHistory(url, projectId);
  }, [activeTab, loadHistory, projectId, url]);

  const handleTabSwitch = useCallback((tab: IssuesTab) => {
    setSelectedPageUrl(null);
    setActiveTab(tab);
  }, []);

  const refreshIssueData = useCallback(async () => {
    await Promise.all([refreshDashboard(), refetchInactiveIssueKeys()]);
  }, [refreshDashboard, refetchInactiveIssueKeys]);

  const handleRestorePausedIssue = useCallback(
    async (checkId: string) => {
      if (restoringPausedCheckId) return;
      setRestoringPausedCheckId(checkId);
      try {
        await reopenIssue(projectId, normalizedUrl, checkId);
        setLocalDismissedIds((current) => {
          const next = new Set(current);
          next.delete(checkId);
          return next;
        });
        retryStatusResource();
        void refetchInactiveIssueKeys();
        toast.success("Issue restored", "It is active again and will appear in the issues list.");
      } catch (error) {
        toast.error("Issue was not restored", errorMessage(error));
      } finally {
        setRestoringPausedCheckId(null);
      }
    },
    [
      normalizedUrl,
      projectId,
      refetchInactiveIssueKeys,
      restoringPausedCheckId,
      retryStatusResource,
      toast,
    ],
  );

  return (
    <div className="issues-page-layout">
      <div className="issues-score-trend-row">
        <IssuesScoreStrip
          score={sitecmdScore}
          checkedAt={scoreStripCheckedAt}
          issueSummary={issueSummary}
        />
        <CompactTrendStrip models={[issuesTrend]} />
      </div>

      <div className="issue-page-panel">
        <IssuesTabBar activeTab={activeTab} onSwitch={handleTabSwitch} />

        <div className="issues-page-content">
          {activeTab === "by-page" ? (
            <IssuesByPagePanel
              projectId={projectId}
              url={url}
              selectedPageUrl={selectedPageUrl}
              pageGroups={pageGroups}
              pageGroupsLoading={pageGroupsLoading}
              pageGroupsError={pageGroupsError}
              onRetryPageGroups={retryPageGroups}
              onSelectPage={setSelectedPageUrl}
              onSelectIssue={handleSelectPageIssue}
            />
          ) : activeTab === "issues" ? (
            <IssuesQueuePanel
              detectedStack={coerceJsonRecord(latestResult?.detectedStack)}
              rankedIssues={rankedIssues}
              initialFocus={initialFocus}
              issueLinks={effectiveIssueLinks}
              issueSummary={issueSummary}
              onClearSelection={resetIssueStack}
              onOpenScanConfig={openScanConfig}
              onRefreshDashboard={refreshIssueData}
              onRestorePausedIssue={handleRestorePausedIssue}
              onSelect={selectIssue}
              onStatusChange={setStatusFilter}
              pausedWorkItems={pausedWorkItems}
              projectPath={projectPath}
              resolvedList={resolvedList}
              restoringPausedCheckId={restoringPausedCheckId}
              selectedIssueId={selectedIssue?.id ?? null}
              showFirstScanEmpty={showFirstScanEmpty}
              showInitialIssuesLoading={showInitialIssuesLoading}
              showIssuesFailure={showIssuesFailure}
              statusFilter={statusFilter}
              statusResourceError={statusResourceError}
              statusResourceLoading={statusResourceLoading}
              onRetryStatusResource={retryStatusResource}
              url={url}
            />
          ) : (
            <IssuesHistoryPanel projectId={projectId} url={url} openScanConfig={openScanConfig} />
          )}
        </div>
      </div>

      {selectedIssue && (
        <Suspense fallback={null}>
          <IssueDossier
            selected={selectedIssue}
            detectedStack={coerceJsonRecord(latestResult?.detectedStack)}
            projectId={projectId}
            url={url}
            projectPath={projectPath}
            latestScanId={effectiveLatestScanId}
            onIssueLinkCreated={refreshDashboard}
            securityUpdates={criticalSecurityUpdates}
            nonSecurityUpdates={nonSecurityUpdates}
            lastCIRun={lastCIRun}
            framework={effectiveLatestCodeScanSummary?.framework}
            onDismiss={handleDismissIssue}
            onClose={closeIssue}
            onOpenCause={openCause}
            onOpenIntegrations={handleOpenIntegrations}
            onBack={selectedStack.length > 1 ? goBack : undefined}
          />
        </Suspense>
      )}
    </div>
  );
}

function newestTimestamp(values: Array<string | null>): string | null {
  return values.reduce<string | null>((latest, value) => {
    if (!value) return latest;
    if (!latest) return value;
    const valueMs = Date.parse(value);
    const latestMs = Date.parse(latest);
    if (!Number.isFinite(valueMs)) return latest;
    if (!Number.isFinite(latestMs)) return value;
    return valueMs > latestMs ? value : latest;
  }, null);
}
