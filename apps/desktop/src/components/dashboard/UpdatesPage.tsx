/* eslint-disable react-refresh/only-export-components -- test helpers are exported here. */

import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { detectUpdates } from "@/lib/commands";
import { logger } from "@/lib/logger";
import { recordUpdateEvent } from "@/lib/event-writes";
import { emitAppEvent } from "@/lib/app-events";
import { HeaderActions } from "@/app/ShellHeader";
import { SurfaceState } from "@/components/ui/surface-state";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import type { UpdateReport, PackageUpdate } from "@/lib/types";
import { useToast } from "@/hooks/useToast";
import { WatchedFileArrivalBanner } from "@/components/issues/WatchedFileArrivalBanner";
import { openPathInEditor } from "@/lib/desktop-actions";
import type { DesktopPromptEntry } from "@/lib/desktop-prompts";
import {
  getRecentPendingProjectUpdates,
  markUpdateVerified,
  readUpdateSnapshot,
  recordSeenUpdates,
  writeUpdateSnapshot,
} from "@/lib/update-memory";
import { normalizeUpdateReport } from "@/lib/package-update-normalize";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import { usePendingVerificationCenter } from "@/lib/pending-verification";
import { RefreshCw } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { publishUpdatesBadgeForReport } from "@/lib/project-nav-badges";
import { PROJECT_SIGNALS_CHANGED_EVENT } from "@/lib/project-signal-events";
import { formatUrlDisplay } from "@/lib/utils";
import { CompactTrendStrip } from "./CompactTrend";
import { buildUpdatesTrendModel } from "./compact-trend-model";
import {
  buildAppliedUpdateEventSourceId,
  buildUpdateRefreshHistoryDraft,
  getClearedUpdates,
  getTrustedPreviousUpdates,
} from "./update-history";
import {
  CopyAllButton,
  PendingUpdatesVerificationSection,
  SecurityBanner,
  UpdateFilterPills,
  UpdateSection,
  UpdatesHistorySection,
  UpdatesLoadingState,
  UpdatesRowsSkeleton,
  UpdatesStatCards,
  type UpdateFilter,
} from "./update-sections";
import { UpdateDossier } from "./UpdateDossier";
import {
  buildUpdateDisplayModel,
  findPackageUpdateByItemId,
  formatLastChecked,
  getPendingUpdateEntries,
  UPDATE_REPORT_CACHE_TTL_MS,
} from "./updates-page-model";
import { useUpdatesHistory } from "./useUpdatesHistory";
import { useUpdatesVerificationActions } from "./useUpdatesVerificationActions";
import { Button } from "@/components/ui/button";
import { queryKeys } from "@/lib/query/query-keys";
import { useCurrentTime } from "@/lib/useCurrentTime";

export { buildAiTask, buildCommand } from "./update-commands";
export { buildUpdateRefreshHistoryDraft } from "./update-history";
export { UpdateDossier } from "./UpdateDossier";

interface UpdatesPageProps {
  projectId: number;
  url: string;
  projectPath: string | null;
  projectName: string;
  onAddFolder: () => void;
  initialTarget?: {
    lane?: "pending-verification" | null;
    itemId?: string | null;
  } | null;
  arrivalPrompt?: DesktopPromptEntry | null;
}

interface CachedUpdateReport {
  report: UpdateReport;
  timestamp: number;
}

export function UpdatesPage({
  projectId,
  url,
  projectPath,
  projectName,
  onAddFolder,
  initialTarget,
  arrivalPrompt,
}: UpdatesPageProps) {
  const nowMs = useCurrentTime();
  const [report, setReport] = useState<UpdateReport | null>(null);
  const [lastChecked, setLastChecked] = useState<number | null>(null);
  const [loading, setLoading] = useState(() => Boolean(projectPath));
  const [selectedUpdate, setSelectedUpdate] = useState<PackageUpdate | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<UpdateFilter>("all");
  const toast = useToast();
  const queryClient = useQueryClient();
  const toastRef = useRef(toast);
  const pendingSectionRef = useRef<HTMLDivElement | null>(null);
  const updatesSectionRef = useRef<HTMLDivElement | null>(null);
  const autoOpenedArrivalRef = useRef<string | null>(null);
  const reportRef = useRef<UpdateReport | null>(null);
  const activeProjectScopeRef = useRef<string>("");
  const pendingVerificationEntries = usePendingVerificationCenter();
  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(url), [url]);
  const hostname = useMemo(() => formatUrlDisplay(normalizedUrl), [normalizedUrl]);
  const projectScopeKey = useMemo(
    () => `${projectId}:${projectPath ?? "no-path"}:${normalizedUrl}`,
    [normalizedUrl, projectId, projectPath],
  );
  const reportQueryKey = useMemo(
    () => queryKeys.updates.report(projectId, projectPath ?? "", normalizedUrl),
    [normalizedUrl, projectId, projectPath],
  );
  const { loadUpdateHistory, updateHistory, updateHistoryLoading } = useUpdatesHistory({
    projectId,
    projectPath,
    report,
  });
  useEffect(() => {
    toastRef.current = toast;
  }, [toast]);
  const handleOpenArrivalFile = useCallback(() => {
    if (!arrivalPrompt?.absolutePath) return;
    openPathInEditor(arrivalPrompt.absolutePath)
      .then(() => toast.success("Opened changed file", arrivalPrompt.relativePath))
      .catch((err) => toast.error("Could not open editor", String(err)));
  }, [arrivalPrompt, toast]);
  const handleReviewArrivalWork = useCallback(() => {
    updatesSectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, []);

  const loadReport = useCallback(
    async (options?: { showToast?: boolean; recordHistory?: boolean }) => {
      const scopeKey = projectScopeKey;
      if (!projectPath || activeProjectScopeRef.current !== scopeKey) return null;
      const showToast = options?.showToast ?? false;
      const recordHistory = options?.recordHistory ?? true;
      const hadExistingReport = reportRef.current != null;
      setLoading(true);
      setError(null);
      try {
        const rawResult = await detectUpdates({ projectId, projectPath });
        if (activeProjectScopeRef.current !== scopeKey) return null;
        // Normalize partial update payloads at the command boundary.
        const result = normalizeUpdateReport(rawResult);
        // An empty package census cannot prove that prior updates were resolved.
        const scanObservedDependencies = result.packages.length > 0;
        const resultSummary = buildUpdateQueueSummary(result.updates);
        const previousReport = reportRef.current;
        const previousSnapshot = readUpdateSnapshot(projectPath);
        const recentPendingUpdates = getRecentPendingProjectUpdates(projectPath);
        const previousUpdates = getTrustedPreviousUpdates(
          previousReport?.updates ??
            (previousSnapshot && previousSnapshot.length > 0 ? previousSnapshot : null) ??
            (recentPendingUpdates.length > 0 ? recentPendingUpdates : null),
          result,
        );
        setReport(result);
        reportRef.current = result;
        // Keep the last observed list rather than letting an unobserved scan
        // erase it; the next real scan diffs against something true.
        if (scanObservedDependencies) {
          writeUpdateSnapshot(projectPath, result.updates);
        }
        emitAppEvent(PROJECT_SIGNALS_CHANGED_EVENT, {
          projectId,
          url,
          source: "updates",
          updates: result,
        });
        const now = Date.now();
        queryClient.setQueryData<CachedUpdateReport>(reportQueryKey, {
          report: result,
          timestamp: now,
        });
        setLastChecked(now);
        if (recordHistory && previousUpdates && scanObservedDependencies) {
          const clearedUpdates = getClearedUpdates(previousUpdates, result.updates);
          for (const clearedUpdate of clearedUpdates) {
            markUpdateVerified(projectPath, clearedUpdate);
          }
          const refreshHistoryDraft = buildUpdateRefreshHistoryDraft(
            previousUpdates,
            result.updates,
          );
          if (refreshHistoryDraft && clearedUpdates.length > 0) {
            void recordUpdateEvent({
              projectId,
              title: refreshHistoryDraft.title,
              summary: refreshHistoryDraft.summary,
              detail: JSON.stringify({
                ...refreshHistoryDraft.detail,
                project_path: projectPath,
              }),
              sourceId: buildAppliedUpdateEventSourceId(
                "updates-refresh",
                projectId,
                clearedUpdates,
                resultSummary.total,
                resultSummary.security,
              ),
              severity: refreshHistoryDraft.severity,
            })
              .then(() => loadUpdateHistory())
              .catch(() => {});
          }
        }
        void loadUpdateHistory();
        if (showToast) {
          const secCount = resultSummary.security;
          if (secCount > 0)
            toastRef.current.warning(
              "Security updates available",
              `${secCount} package${secCount === 1 ? " has" : "s have"} known vulnerabilities`,
            );
          else if (resultSummary.total > 0)
            toastRef.current.success(
              "Updates found",
              `${resultSummary.total} package${resultSummary.total === 1 ? "" : "s"} can be updated`,
            );
          else toastRef.current.success("All up to date", "No updates available");
        }
        return result;
      } catch (e) {
        // Route technical details through the redacting logger, never user-facing copy.
        logger.error(
          "detect_updates failed",
          e instanceof Error ? (e.stack ?? e.message) : String(e),
        );
        if (activeProjectScopeRef.current === scopeKey) {
          if (!hadExistingReport) {
            setError("We could not check for package updates right now. Try again in a moment.");
          }
          if (showToast || !hadExistingReport) {
            toastRef.current.error(
              "Update check failed",
              "We could not reach the dependency scanner. Try again in a moment.",
            );
          }
        }
        return null;
      } finally {
        if (activeProjectScopeRef.current === scopeKey) {
          setLoading(false);
        }
      }
    },
    [loadUpdateHistory, projectId, projectPath, projectScopeKey, queryClient, reportQueryKey, url],
  );

  const runScan = useCallback(
    async (showToast: boolean) => {
      await loadReport({ showToast, recordHistory: true });
    },
    [loadReport],
  );

  const handleRefresh = useCallback(() => runScan(true), [runScan]);

  useEffect(() => {
    activeProjectScopeRef.current = projectScopeKey;
    reportRef.current = null;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- resets all update-panel state when the project scope changes, before the async report reload
    setReport(null);
    setLastChecked(null);
    setLoading(Boolean(projectPath));
    setSelectedUpdate(null);
    setError(null);
    if (!projectPath) {
      publishUpdatesBadgeForReport(projectId, null);
    }
  }, [projectId, projectPath, projectScopeKey]);

  useEffect(() => {
    if (!projectPath) return;
    let cancelled = false;
    const scopeKey = projectScopeKey;

    // Persisted snapshots omit the package census; render only live or fresh
    // in-session reports.
    const showCachedReportIfFresh = () => {
      const cached = queryClient.getQueryData<CachedUpdateReport>(reportQueryKey);
      if (!cached || Date.now() - cached.timestamp >= UPDATE_REPORT_CACHE_TTL_MS) return;
      if (cancelled || activeProjectScopeRef.current !== scopeKey) return;
      setReport(cached.report);
      reportRef.current = cached.report;
      setLastChecked(cached.timestamp);
      setLoading(false);
      void loadUpdateHistory();
    };

    showCachedReportIfFresh();
    // loadReport synchronously enters loading before its first await.
    // eslint-disable-next-line react-hooks/set-state-in-effect -- see above
    void runScan(false);

    return () => {
      cancelled = true;
    };
  }, [loadUpdateHistory, projectPath, projectScopeKey, queryClient, reportQueryKey, runScan]);

  useEffect(() => {
    if (!projectPath || !report) return;
    publishUpdatesBadgeForReport(projectId, report);
    recordSeenUpdates(projectPath, report.updates);
  }, [projectId, projectPath, report]);

  useEffect(() => {
    reportRef.current = report;
  }, [report]);

  const pendingUpdateEntries = useMemo(() => {
    return getPendingUpdateEntries(pendingVerificationEntries, projectId, normalizedUrl);
  }, [normalizedUrl, pendingVerificationEntries, projectId]);
  const {
    handleVerifyAllPending,
    handleVerifyPendingEntry,
    handleVerifyUpdate,
    verifyingAllPending,
    verifyingPendingId,
    verifyingUpdateKey,
  } = useUpdatesVerificationActions({
    hostname,
    loadReport,
    loadUpdateHistory,
    normalizedUrl,
    pendingUpdateEntries,
    projectId,
    projectName,
    projectPath,
    report,
    reportRef,
    toast,
  });

  useEffect(() => {
    if (initialTarget?.lane !== "pending-verification" || pendingUpdateEntries.length === 0) return;
    requestAnimationFrame(() => {
      pendingSectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  }, [initialTarget?.lane, pendingUpdateEntries.length]);

  useEffect(() => {
    if (!arrivalPrompt || !updatesSectionRef.current) return;
    requestAnimationFrame(() => {
      updatesSectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  }, [arrivalPrompt, arrivalPrompt?.id]);

  useEffect(() => {
    autoOpenedArrivalRef.current = null;
  }, [arrivalPrompt?.id, normalizedUrl]);

  useEffect(() => {
    if (!initialTarget?.itemId || !report || loading) return;
    const targetUpdate = findPackageUpdateByItemId(report.updates, initialTarget.itemId);
    if (!targetUpdate) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- selects the deep-linked update once the async report has loaded
    setSelectedUpdate((current) =>
      current?.ecosystem === targetUpdate.ecosystem && current?.name === targetUpdate.name
        ? current
        : targetUpdate,
    );
  }, [initialTarget?.itemId, loading, report]);

  useEffect(() => {
    if (!arrivalPrompt || selectedUpdate || loading) return;
    if ((report?.updates.length ?? 0) !== 1) return;
    const focusKey = `${arrivalPrompt.id}:${normalizedUrl}`;
    if (autoOpenedArrivalRef.current === focusKey) return;
    autoOpenedArrivalRef.current = focusKey;
    setSelectedUpdate(report!.updates[0]!);
  }, [arrivalPrompt, loading, normalizedUrl, report, selectedUpdate]);
  if (!projectPath) {
    return (
      <SurfaceState
        kind="empty"
        title="No project folder linked"
        description="Link a local project folder so SiteCMD can inspect dependencies, spot vulnerable packages, and suggest the next update to verify."
        className="page-content"
        primaryAction={{ label: "Add Folder", onClick: onAddFolder }}
      />
    );
  }

  if (error) {
    return (
      <SurfaceState
        kind="error"
        title="Updates could not load"
        description={error}
        className="page-content"
        primaryAction={{ label: "Retry", onClick: handleRefresh }}
      />
    );
  }

  if (loading && !report) {
    return <UpdatesLoadingState />;
  }

  const updateDisplay = buildUpdateDisplayModel(report, filter);
  const updatesTrend = buildUpdatesTrendModel({
    events: updateHistory,
    updates: report?.updates ?? [],
  });

  return (
    <div className="page-content stack-hero">
      <HeaderActions>
        <Button
          unstyled
          onClick={handleRefresh}
          disabled={loading}
          className="btn-ghost-xs header-refresh-btn">
          <RefreshCw className="icon-sm" /> Refresh
        </Button>
        <CopyAllButton updates={updateDisplay.copyableUpdates} label="Copy All Commands" />
      </HeaderActions>

      <UpdatesStatCards
        securityCount={updateDisplay.securityUpdates.length}
        packageCount={updateDisplay.packageCount}
        lastAuditLabel={lastChecked ? formatLastChecked(lastChecked, nowMs) : "-"}
        loading={loading}
      />

      <CompactTrendStrip models={[updatesTrend]} />

      {arrivalPrompt ? (
        <WatchedFileArrivalBanner
          prompt={arrivalPrompt}
          onOpenFile={arrivalPrompt.absolutePath ? handleOpenArrivalFile : null}
          onReview={handleReviewArrivalWork}
          reviewLabel="Review package changes"
        />
      ) : null}

      <PendingUpdatesVerificationSection
        sectionRef={pendingSectionRef}
        pendingEntries={pendingUpdateEntries}
        verifyingAllPending={verifyingAllPending}
        verifyingPendingId={verifyingPendingId}
        verifyingUpdateKey={verifyingUpdateKey}
        onVerifyAll={handleVerifyAllPending}
        onVerifyEntry={handleVerifyPendingEntry}
      />

      {updateDisplay.securityUpdates.length > 0 && (
        <SecurityBanner updates={updateDisplay.securityUpdates} onOpenDossier={setSelectedUpdate} />
      )}

      <UpdateFilterPills
        sectionRef={updatesSectionRef}
        filter={filter}
        counts={{
          all: updateDisplay.totalCount,
          major: updateDisplay.majors.length,
          minor: updateDisplay.minors.length,
          patch: updateDisplay.patches.length,
        }}
        onFilterChange={setFilter}
      />

      {loading ? (
        <UpdatesRowsSkeleton />
      ) : updateDisplay.totalCount === 0 ? (
        <SurfaceState
          kind="empty"
          title="All packages are up to date"
          description={`${updateDisplay.packageCount} packages checked. No known vulnerabilities or pending version bumps are waiting here right now.`}
        />
      ) : updateDisplay.sections.length === 0 ? (
        <SurfaceState
          kind="empty"
          title="No updates in this filter"
          description="Security updates are listed above. No regular major, minor, or patch updates match this view."
        />
      ) : (
        updateDisplay.sections.map((section) => (
          <UpdateSection key={section.label} {...section} onOpenDossier={setSelectedUpdate} />
        ))
      )}

      {updateHistoryLoading || updateHistory.length > 0 ? (
        <UpdatesHistorySection events={updateHistory} loading={updateHistoryLoading} />
      ) : null}

      {selectedUpdate && projectPath && (
        <UpdateDossier
          update={selectedUpdate}
          allUpdates={report?.updates ?? []}
          projectId={projectId}
          url={url}
          projectPath={projectPath}
          arrivalPrompt={arrivalPrompt}
          onClose={() => setSelectedUpdate(null)}
          onVerify={() => handleVerifyUpdate(selectedUpdate)}
          verifying={verifyingUpdateKey === `${selectedUpdate.ecosystem}:${selectedUpdate.name}`}
        />
      )}
    </div>
  );
}
