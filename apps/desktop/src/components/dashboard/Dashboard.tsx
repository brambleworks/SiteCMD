import { useEffect, useMemo, useState } from "react";
import { formatRelativeTime, MS_PER_DAY } from "@/lib/format";
import { useCurrentTime } from "@/lib/useCurrentTime";
import { getHostname } from "@/lib/utils";
import { useCurrentScore } from "@/hooks/useCurrentScore";
import { useAlerts } from "@/hooks/useAlerts";
import { useSiteBaseline } from "@/hooks/useSiteBaseline";
import { formatScoreBreakdown } from "@/lib/score-breakdown";
import { isNavPageConnected } from "@/lib/nav-integrations";
import type { ScanResult, CodeScanResult } from "@/lib/types";
import { SurfaceState } from "@/components/ui/surface-state";
import { buildProjectIssueSummaryFromWorkSummary } from "@/lib/project-issue-summary";
import { buildPackageUpdateTarget, findStrongestPackageUpdate } from "@/lib/update-priority";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import { buildSetupRows } from "@/lib/dashboard/setup-rows";
import { buildDashboardActivity, buildDashboardActivityFromEvents } from "@/lib/dashboard/activity";
import type { ActivityRow, BootstrapTask, SetupRow } from "@/lib/dashboard/types";
import { toNavPage, type NavTarget } from "@/components/layout/nav-page";
import type { AppTarget } from "@/lib/app-targets";
import { buildEventScanTarget } from "@/components/events/event-presentation";
import {
  useDashboardData,
  type PlausibleSummary,
  type CloudflareSummary,
  type UptimeRobotSummary,
} from "./useDashboardData";
import { IdentityHealthStrip } from "./zones/IdentityHealthStrip";
import { AtAGlance } from "./zones/AtAGlance";
import { ActionItemsCard } from "./zones/ActionItemsCard";
import { ReferenceSignals } from "./zones/ReferenceSignals";
import { SiteBaselineCard } from "./zones/SiteBaselineCard";
import { WebVitalsDetailModal } from "./WebVitalsDetailModal";
import { RecentActivityCard, SetupCard } from "./zones/ActivityAndSetup";
import { useRenderSanityCheck } from "@/lib/render-sanity";
import { DashboardEmptyState, DashboardLoadingState } from "./DashboardEmptyState";
import { buildDashboardActionItems } from "./dashboard-action-items";
import { deriveDashboardSearchSignals } from "./dashboard-search-signals";
import { buildIssuesTrendModel, buildUpdatesTrendModel } from "./compact-trend-model";

interface DashboardProps {
  url: string;
  projectId: number;
  projectName: string;
  framework: string | null;
  projectPath: string | null;
  onViewResults: (scanId: number, category?: string) => void;
  onViewCodeScan: (scanId: number, itemId?: string | null) => void;
  onRescan: () => void;
  onOpenScanConfig: () => void;
  onOpenCodeScanConfig: () => void;
  onAddFolder: () => void;
  onNavigate: (page: NavTarget) => void;
  onOpenTarget: (target: AppTarget) => void;
  scanning: boolean;
  latestResult: ScanResult | null;
  latestCodeResult: CodeScanResult | null;
}

export function Dashboard({
  url,
  projectId,
  projectName,
  framework,
  projectPath,
  onViewResults,
  onViewCodeScan,
  onRescan: _onRescan,
  onOpenScanConfig,
  onOpenCodeScanConfig,
  onAddFolder,
  onNavigate,
  onOpenTarget,
  scanning: _scanning,
  latestResult,
  latestCodeResult,
}: DashboardProps) {
  useRenderSanityCheck("Dashboard");
  const currentTimeMs = useCurrentTime();
  const { score: currentScore, refresh: refreshCurrentScore } = useCurrentScore(projectId, url);
  // Count only: the strip shows an unread badge; the rows live on the
  // Alerts page itself.
  const { unreadCount } = useAlerts(projectId, "unread", { includeRows: false });
  // Hide baseline comparison until a scan has established one.
  const siteBaseline = useSiteBaseline(url || null, projectId);

  const {
    trend,
    codeTrend,
    latestDetail,
    latestScanId,
    aggregatedCheckCounts,
    aggregatedFailedIssues,
    allUpdates,
    integrations,
    configuredIntegrations,
    lastCIRun,
    commitsSinceLastScan,
    dashboardReady,
    dashboardLoadError,
    latestCodeScanSummary,
    latestCodeScanDetail,
    updatesCheckedAt,
    searchRegression,
    dismissedIds,
    dismissedProjectId,
    psiReport,
    recentEvents,
    updateEvents,
    recentEventsLoading,
    refreshDashboard,
    referenceSignalsLoading,
    sslProbe,
    verdict,
    bootstrapTasks,
    workSummary,
  } = useDashboardData({
    url,
    projectId,
    projectPath,
    latestResult,
    latestCodeResult,
    includeReferenceSignals: true,
  });

  const visibleDashboardWebIssues = useMemo(
    () =>
      dismissedProjectId === projectId && dismissedIds.size > 0
        ? aggregatedFailedIssues.filter((issue) => !dismissedIds.has(issue.checkId))
        : aggregatedFailedIssues,
    [aggregatedFailedIssues, dismissedIds, dismissedProjectId, projectId],
  );

  const visibleLatestDetailIssues = useMemo(
    () =>
      (latestDetail?.issues ?? []).filter((issue) => {
        if (issue.status !== "fail" && issue.status !== "warn") return false;
        return !(dismissedProjectId === projectId && dismissedIds.has(issue.checkId));
      }),
    [dismissedIds, dismissedProjectId, latestDetail?.issues, projectId],
  );

  const hasAggregated = aggregatedCheckCounts.total > 0;
  // Active counts come from canonical backend groups, not raw scan artifacts.
  const issueSummary = useMemo(
    () => buildProjectIssueSummaryFromWorkSummary(workSummary),
    [workSummary],
  );
  const latest = trend.length > 0 ? trend[trend.length - 1] : null;

  const topCodeIssue = latestCodeScanDetail?.issues[0] ?? null;
  const codeFocusIssueId = topCodeIssue?.id ?? null;
  const latestCodeCheckedAt = latestCodeScanSummary?.checkedAt
    ? formatRelativeTime(new Date(latestCodeScanSummary.checkedAt), currentTimeMs)
    : null;

  useEffect(() => {
    if (!latestResult && !latestCodeResult) return;
    void refreshCurrentScore();
  }, [latestCodeResult, latestResult, refreshCurrentScore]);

  const openLatestCodeScan = () => {
    if (latestCodeScanSummary) {
      onViewCodeScan(latestCodeScanSummary.id, codeFocusIssueId);
      return;
    }
    onOpenCodeScanConfig();
  };
  const [webVitalsDetailOpen, setWebVitalsDetailOpen] = useState(false);

  const updateSummary = useMemo(() => buildUpdateQueueSummary(allUpdates), [allUpdates]);
  const issuesTrend = useMemo(
    () =>
      buildIssuesTrendModel({
        webTrend: trend,
        codeTrend,
        currentIssueCount: issueSummary.totalCount,
        criticalCount: issueSummary.severityCounts.critical,
      }),
    [codeTrend, issueSummary.severityCounts.critical, issueSummary.totalCount, trend],
  );
  const updatesTrend = useMemo(
    () => buildUpdatesTrendModel({ events: updateEvents, updates: allUpdates }),
    [allUpdates, updateEvents],
  );

  // A Code Scan baseline is sufficient even when no web trend exists.
  const hasAnyScan = Boolean(latest) || Boolean(latestCodeScanSummary);

  if (!dashboardReady && !hasAnyScan) {
    return <DashboardLoadingState />;
  }

  if (dashboardLoadError && !hasAnyScan) {
    return (
      <SurfaceState
        kind="error"
        title="Dashboard could not load"
        description="We could not load this project summary right now. Try again to rebuild the latest snapshot."
        primaryAction={{ label: "Retry", onClick: refreshDashboard }}
      />
    );
  }

  if (!hasAnyScan) {
    return (
      <DashboardEmptyState
        url={url}
        projectName={projectName}
        framework={framework}
        projectPath={projectPath}
        onOpenScanConfig={onOpenScanConfig}
        onAddFolder={onAddFolder}
        onNavigate={onNavigate}
      />
    );
  }

  const webIssueCount = hasAggregated
    ? visibleDashboardWebIssues.length
    : visibleLatestDetailIssues.length;

  // Fall back to Code Scan freshness for code-only projects.
  const timeAgo = latest
    ? formatRelativeTime(new Date(latest.timestamp), currentTimeMs)
    : (latestCodeCheckedAt ?? "");

  const handleWebScanOpen = () => {
    if (latestScanId) onViewResults(latestScanId);
    else onNavigate("issues");
  };

  const plausible = integrations.find((i) => i.integrationType === "plausible");
  const plausibleData = plausible?.data as PlausibleSummary | undefined;
  // Share the sidebar's connected-page predicate.
  const hasAnyAnalyticsConfigured = isNavPageConnected("analytics", configuredIntegrations);
  const hasSearchConfigured = isNavPageConnected("search-console", configuredIntegrations);
  const hasUptimeConfigured = configuredIntegrations.has("uptimerobot");
  const hasCloudflareConfigured = configuredIntegrations.has("cloudflare");
  const searchSignals = deriveDashboardSearchSignals({
    integrations,
    searchRegression,
  });

  const cloudflare = integrations.find((i) => i.integrationType === "cloudflare" && !i.error);
  const cloudflareData = cloudflare?.data as CloudflareSummary | undefined;

  const uptimeRobot = integrations.find((i) => i.integrationType === "uptimerobot" && !i.error);
  const uptimeData = uptimeRobot?.data as UptimeRobotSummary | undefined;
  const primaryMonitor = uptimeData?.monitors[0] ?? null;

  const strongestUpdate = findStrongestPackageUpdate(allUpdates);
  const openDependencyRisk = () => {
    if (strongestUpdate) {
      onOpenTarget(buildPackageUpdateTarget(projectId, url, strongestUpdate));
      return;
    }
    onNavigate("updates");
  };

  const detectedStack = latestDetail?.detectedStack as
    Record<string, string | null> | null | undefined;
  const stackChip = {
    framework: framework ?? (detectedStack?.framework as string | null) ?? null,
    host: (detectedStack?.cdn as string | null) ?? null,
    environment: (detectedStack?.environment as string | null) ?? null,
  };

  const siteScoreIssueCount = issueSummary.totalCount;

  const hasSiteScoreBaseline = Boolean(latestDetail || latestResult || latestCodeScanDetail);

  const siteScoreData =
    currentScore && hasSiteScoreBaseline
      ? {
          value: Math.round(currentScore.overall),
          delta: null,
          issueCount: siteScoreIssueCount,
          criticalCount: currentScore.criticalCount,
          scanAgeLabel: timeAgo,
          breakdown: formatScoreBreakdown(currentScore),
        }
      : null;

  const lastCheckedData = buildLastCheckedData({
    webCheckedAt: latestDetail?.timestamp ?? latest?.timestamp ?? null,
    codeCheckedAt: latestCodeScanSummary?.checkedAt ?? null,
    nowMs: currentTimeMs,
  });

  const visitorsData = plausibleData
    ? {
        visitors: plausibleData.visitors,
        pageviews: plausibleData.pageviews,
        bouncePct: plausibleData.bounce_rate,
        deltaPct: null,
      }
    : null;

  const uptimeTileData = primaryMonitor
    ? {
        ratio: primaryMonitor.uptime_ratio,
        avgResponseMs: primaryMonitor.average_response,
        outageCount: 0,
      }
    : null;

  // Keep this cheap rebuild outside useMemo because earlier returns are conditional.
  const actionItemsData = buildDashboardActionItems({
    allUpdates,
    issueSummary,
    issuesTrend,
    onNavigate,
    updatesTrend,
  });

  // Web Vitals come only from PageSpeed data.
  const webVitalsData = psiReport
    ? {
        score: psiReport.performanceScore,
        lcpMs: psiReport.lcpMs,
        cls: psiReport.cls,
        tbtMs: psiReport.tbtMs,
      }
    : null;

  const latestDeployState = lastCIRun ? lastCIRun.conclusion : null;
  const latestDeployCheckedAt = lastCIRun
    ? formatRelativeTime(new Date(lastCIRun.updatedAt), currentTimeMs)
    : null;

  const deployReleaseData = lastCIRun
    ? {
        tagName: lastCIRun.name,
        conclusion: latestDeployState,
        ageLabel: latestDeployCheckedAt ?? "",
        commitsSince: commitsSinceLastScan.length > 0 ? commitsSinceLastScan.length : null,
      }
    : null;

  const deliveryData = cloudflareData
    ? {
        cacheHitPct: cloudflareData.cache_hit_rate,
        requestsTotal: cloudflareData.requests_total,
        threatsBlocked: cloudflareData.threats_blocked,
        bandwidthMb: cloudflareData.bandwidth_total / (1024 * 1024),
      }
    : null;

  // Not memoized, for the same conditional-hook reason as actionItemsData.
  const syntheticActivityItems = buildDashboardActivity({
    latestDeploy: lastCIRun,
    commitsSinceLastScan,
    latestWebScan: latestDetail,
    webIssueCount,
    latestCodeScan: latestCodeScanSummary,
    updatesCheckedAt,
    updateBreakdown: updateSummary.breakdown,
  });

  const rawActivityItems =
    recentEvents.length > 0
      ? buildDashboardActivityFromEvents(recentEvents)
      : syntheticActivityItems;

  const activityItems: ActivityRow[] = rawActivityItems.map((item) => {
    const eventTarget = item.parsedDetail
      ? buildEventScanTarget(projectId, item.parsedDetail)
      : null;

    return {
      id: item.id,
      label: item.label,
      value: item.value,
      valueColor: item.valueColor,
      eventType: item.eventType,
      source: item.source,
      occurredAt: item.occurredAt,
      timeAgo: formatRelativeTime(new Date(item.occurredAt), currentTimeMs),
      onOpen: eventTarget
        ? () => onOpenTarget(eventTarget)
        : item.target === "deploys"
          ? () => onNavigate("deploys")
          : item.target === "updates"
            ? openDependencyRisk
            : item.target === "code-scan"
              ? openLatestCodeScan
              : item.target === "issues"
                ? () => onNavigate("issues")
                : () => onNavigate("events"),
    };
  });

  const handleBootstrapOpen = (task: BootstrapTask) => {
    const t = task.target;
    if (t.type === "nav") {
      onNavigate(toNavPage(t.page));
      return;
    }
    if (t.type === "nav-settings") {
      onNavigate(`settings:${t.tab}`);
      return;
    }
    if (t.action === "add-folder") {
      onAddFolder();
      return;
    }
    if (t.action === "open-code-scan-config") {
      onOpenCodeScanConfig();
      return;
    }
  };

  const setupRows: SetupRow[] = buildSetupRows(bootstrapTasks ?? [], handleBootstrapOpen);

  return (
    <div className="dashboard-zones">
      <IdentityHealthStrip
        domain={getHostname(url)}
        stack={stackChip}
        sslDaysRemaining={sslProbe?.days_remaining ?? null}
        verdict={verdict ?? { kind: "healthy", phrase: "Healthy", reasons: [] }}
        lastScanIso={latest?.timestamp ?? null}
        unreadAlertCount={unreadCount}
        onOpenAlerts={() => onNavigate("alerts")}
      />

      <ActionItemsCard items={actionItemsData} />

      <AtAGlance
        siteScore={siteScoreData}
        lastChecked={lastCheckedData}
        uptime={uptimeTileData}
        uptimeConfigured={hasUptimeConfigured}
        uptimeLoading={referenceSignalsLoading && hasUptimeConfigured}
        visitors={visitorsData}
        analyticsConfigured={hasAnyAnalyticsConfigured}
        analyticsLoading={referenceSignalsLoading}
        seoClicks={searchSignals.seoClicks}
        searchConfigured={hasSearchConfigured}
        searchLoading={referenceSignalsLoading && hasSearchConfigured}
        onOpenIssues={() => onNavigate("issues")}
        onRunScan={handleWebScanOpen}
        onOpenUptime={() => onNavigate("integrations")}
        onOpenAnalytics={() => onNavigate("analytics")}
        onOpenSearchConsole={() => onNavigate("search-console")}
        onOpenIntegrations={() => onNavigate("integrations")}
      />

      <ReferenceSignals
        webVitals={webVitalsData}
        webVitalsLoading={referenceSignalsLoading && !webVitalsData}
        searchIndex={searchSignals.searchIndex}
        searchConfigured={hasSearchConfigured}
        searchLoading={referenceSignalsLoading && hasSearchConfigured}
        delivery={deliveryData}
        deliveryConfigured={hasCloudflareConfigured}
        deliveryLoading={referenceSignalsLoading && hasCloudflareConfigured}
        deployRelease={deployReleaseData}
        deploysFolderLinked={Boolean(projectPath)}
        onOpenWebVitals={() => setWebVitalsDetailOpen(true)}
        onOpenSearchConsole={() => onNavigate("search-console")}
        onOpenDelivery={() => onNavigate("integrations")}
        onOpenDeploys={() => onNavigate("deploys")}
        onOpenIntegrations={() => onNavigate("integrations")}
      />

      <SiteBaselineCard
        baseline={siteBaseline.baseline}
        deciding={siteBaseline.deciding}
        refusal={siteBaseline.refusal}
        onDecide={siteBaseline.decide}
      />

      <div
        className={`dashboard-activity-grid ${setupRows.length > 0 ? "dashboard-activity-grid--split" : ""}`}>
        <RecentActivityCard
          activity={activityItems}
          activityLoading={recentEventsLoading && recentEvents.length === 0}
          onOpenEmptyActivity={onOpenScanConfig}
          onOpenAllActivity={() => onNavigate("events")}
        />
        <SetupCard rows={setupRows} />
      </div>

      {webVitalsDetailOpen && (
        <WebVitalsDetailModal
          url={url}
          hostname={getHostname(url)}
          onClose={() => setWebVitalsDetailOpen(false)}
        />
      )}
    </div>
  );
}

const LAST_CHECKED_STALE_MS = 7 * MS_PER_DAY;

function buildLastCheckedData({
  webCheckedAt,
  codeCheckedAt,
  nowMs,
}: {
  webCheckedAt: string | null;
  codeCheckedAt: string | null;
  nowMs: number;
}) {
  const entries = [
    webCheckedAt ? { kind: "web" as const, label: "Web", iso: webCheckedAt } : null,
    codeCheckedAt ? { kind: "code" as const, label: "Code", iso: codeCheckedAt } : null,
  ].filter((entry): entry is NonNullable<typeof entry> => entry !== null);

  if (entries.length === 0) return null;

  const newest = entries.reduce((best, entry) =>
    new Date(entry.iso).getTime() > new Date(best.iso).getTime() ? entry : best,
  );
  const newestTime = new Date(newest.iso).getTime();

  return {
    label: formatRelativeTime(new Date(newest.iso), nowMs),
    kind: newest.kind,
    sub: entries
      .map((entry) => `${entry.label} ${formatRelativeTime(new Date(entry.iso), nowMs)}`)
      .join(" · "),
    stale: Number.isFinite(newestTime) ? nowMs - newestTime > LAST_CHECKED_STALE_MS : false,
  };
}
