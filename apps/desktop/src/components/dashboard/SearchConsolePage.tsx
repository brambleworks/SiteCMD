/* eslint-disable react-refresh/only-export-components -- test helpers are exported here. */
import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import type { NavTarget } from "@/components/layout/nav-page";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { SurfaceState } from "@/components/ui/surface-state";
import { HeaderActions } from "@/app/ShellHeader";
import { WatchedFileArrivalBanner } from "@/components/issues/WatchedFileArrivalBanner";
import { Loader2, RefreshCw, Search } from "lucide-react";
import type { CheckResult, CategoryScore } from "@/lib/types";
import type { SearchConsoleData, BingSearchData } from "@/lib/analytics-types";
import { useToast } from "@/hooks/useToast";
import { openPathInEditor } from "@/lib/desktop-actions";
import { useDesktopPrefs } from "@/lib/desktop-prefs";
import type { DesktopPromptEntry } from "@/lib/desktop-prompts";
import { usePendingVerificationCenter } from "@/lib/pending-verification";
import { buildAnalyticsSnapshotKey } from "@/lib/analytics-snapshot-cache";
import { useAnalyticsQuery } from "@/components/dashboard/useAnalyticsQuery";
import {
  BingSection,
  PendingSearchVerificationSection,
  SearchConsoleLoadingState,
  SearchEngineSection,
} from "@/components/dashboard/SearchConsoleSections";
import { InlineIntegrationSetup } from "@/components/settings/InlineIntegrationSetup";
import { SeoIssueDossier } from "@/components/dashboard/SeoIssueDossier";
import {
  getSingleFocusedSeoIssueId,
  type Period,
} from "@/components/dashboard/search-console-page-model";
import { useSearchConsoleVerificationActions } from "@/components/dashboard/useSearchConsoleVerificationActions";
import { Button } from "@/components/ui/button";
import { useSearchScanQuery } from "./useSearchScanQuery";
import { userFacingError } from "@/lib/user-facing-error";

export {
  buildSeoCategoryScore,
  getSingleFocusedSeoIssueId,
  inferSeoFocus,
  matchesSeoFocus,
} from "@/components/dashboard/search-console-page-model";

interface SearchConsolePageProps {
  projectId: number;
  url: string;
  projectPath?: string;
  onNavigate: (page: NavTarget) => void;
  initialFocus?: string | null;
  initialItemId?: string | null;
  initialLane?: "pending-verification" | null;
  arrivalPrompt?: DesktopPromptEntry | null;
}

export function SearchConsolePage({
  projectId,
  url,
  projectPath,
  onNavigate,
  initialFocus,
  initialItemId,
  initialLane,
  arrivalPrompt,
}: SearchConsolePageProps) {
  const toast = useToast();
  const { prefs: desktopPrefs } = useDesktopPrefs();
  const pendingSectionRef = useRef<HTMLDivElement | null>(null);
  const seoIssuesStateRef = useRef<CheckResult[]>([]);
  const seoPassedChecksStateRef = useRef<CheckResult[]>([]);
  const autoOpenedFocusRef = useRef<string | null>(null);
  const pendingVerificationEntries = usePendingVerificationCenter();
  const normalizedUrl = useMemo(() => normalizeAppUrlForKey(url), [url]);
  const handleOpenArrivalFile = useCallback(() => {
    if (!arrivalPrompt?.absolutePath) return;
    openPathInEditor(arrivalPrompt.absolutePath)
      .then(() => toast.success("Opened changed file", arrivalPrompt.relativePath))
      .catch((err) =>
        toast.error(
          "Could not open editor",
          userFacingError(
            err,
            "SiteCMD could not open it. Open the file from your editor instead.",
          ),
        ),
      );
  }, [arrivalPrompt, toast]);
  const [period, setPeriod] = useState<Period>("28d");
  // The shared query owns transport and persistence; this page derives search views.
  const {
    data: externalData,
    fetchedAt: externalLoadedAt,
    isFetching: gscLoading,
    isError: externalError,
    refresh: refreshExternal,
  } = useAnalyticsQuery({
    projectId,
    period,
    snapshotKey: buildAnalyticsSnapshotKey(projectId, period),
  });

  // Configured integrations remain reconnectable when their latest fetch fails.
  const gscData: SearchConsoleData | null = externalData?.search_console ?? null;
  const bingData: BingSearchData | null = externalData?.bing ?? null;
  const gscError = externalData?.search_console_error ?? null;
  const bingError = externalData?.bing_error ?? null;
  const gscConnected = Boolean(gscData) || Boolean(gscError);
  const bingConnected = Boolean(bingData) || Boolean(bingError);

  // Connected query results re-seed the local optimistic verification overlay.
  const searchScan = useSearchScanQuery(projectId, url);
  const [seoScore, setSeoScore] = useState<CategoryScore | null>(
    () => searchScan.data?.score ?? null,
  );
  const [seoIssues, setSeoIssues] = useState<CheckResult[]>(() => searchScan.data?.issues ?? []);
  const [seoPassedChecks, setSeoPassedChecks] = useState<CheckResult[]>(
    () => searchScan.data?.passedChecks ?? [],
  );
  const scanLoading = searchScan.loading;
  const scanError = searchScan.error;
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(null);

  useResetOnChange(url, () => {
    setSeoScore(null);
    setSeoIssues([]);
    setSeoPassedChecks([]);
  });
  useResetOnChange(searchScan.data, () => {
    setSeoScore(searchScan.data?.score ?? null);
    setSeoIssues(searchScan.data?.issues ?? []);
    setSeoPassedChecks(searchScan.data?.passedChecks ?? []);
  });

  useEffect(() => {
    seoIssuesStateRef.current = seoIssues;
  }, [seoIssues]);

  useEffect(() => {
    seoPassedChecksStateRef.current = seoPassedChecks;
  }, [seoPassedChecks]);

  const pendingSearchEntries = useMemo(() => {
    return pendingVerificationEntries
      .filter(
        (entry) =>
          entry.projectId === projectId &&
          entry.url === normalizedUrl &&
          entry.page === "search-console",
      )
      .sort((a, b) => b.updatedAt - a.updatedAt);
  }, [normalizedUrl, pendingVerificationEntries, projectId]);

  const {
    handleVerifyAllPending,
    handleVerifyIssue,
    handleVerifyPendingEntry,
    verifyingAllPending,
    verifyingCheckId,
    verifyingPendingId,
  } = useSearchConsoleVerificationActions({
    desktopNotificationsEnabled: desktopPrefs.desktopNotifications,
    normalizedUrl,
    pendingSearchEntries,
    projectId,
    seoIssuesStateRef,
    seoPassedChecksStateRef,
    setSeoIssues,
    setSeoPassedChecks,
    setSeoScore,
    toast,
    url,
  });

  const hasScanData = seoScore !== null;
  const hasExternalData = gscConnected || bingConnected;

  const selectedIssue = selectedIssueId
    ? ([...seoIssues, ...seoPassedChecks].find((issue) => issue.checkId === selectedIssueId) ??
      null)
    : null;

  useEffect(() => {
    if (initialLane !== "pending-verification" || pendingSearchEntries.length === 0) return;
    requestAnimationFrame(() => {
      pendingSectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
  }, [initialLane, pendingSearchEntries.length]);

  useResetOnChange(url, () => setSelectedIssueId(null));

  useEffect(() => {
    if (!initialItemId) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- applies the deep-linked item selection; runs on mount and when the target id changes
    setSelectedIssueId(initialItemId);
  }, [initialItemId]);

  useEffect(() => {
    autoOpenedFocusRef.current = null;
  }, [initialFocus, normalizedUrl]);

  useEffect(() => {
    if (!initialFocus || initialItemId || scanLoading) return;
    const singleMatchId = getSingleFocusedSeoIssueId(seoIssues, initialFocus);
    if (!singleMatchId) return;

    const focusKey = `${normalizedUrl}:${initialFocus}`;
    if (autoOpenedFocusRef.current === focusKey) return;

    autoOpenedFocusRef.current = focusKey;
    setSelectedIssueId(singleMatchId);
  }, [initialFocus, initialItemId, normalizedUrl, scanLoading, seoIssues]);

  if (!hasScanData && !hasExternalData && (scanLoading || gscLoading)) {
    return <SearchConsoleLoadingState />;
  }

  // Truly empty - no scan data and no integrations
  if (!hasScanData && !hasExternalData && (scanError || externalError)) {
    return (
      <SurfaceState
        kind="error"
        icon={<Search className="empty-state-icon" />}
        title="Search could not load"
        description="We could not load the SEO view for this project right now. Retry in a moment and SiteCMD will rebuild it."
        className="page-content"
        primaryAction={{ label: "Retry", onClick: () => void searchScan.refetch() }}
      />
    );
  }

  if (!hasScanData && !hasExternalData && !scanLoading) {
    return (
      <div className="page-content stack-hero">
        <SurfaceState
          kind="empty"
          icon={<Search className="empty-state-icon" />}
          title="No search data yet"
          description="Run a scan to check your SEO, or connect Google Search Console and Bing for rankings and search visibility data."
          primaryAction={{ label: "Open Issues", onClick: () => onNavigate("issues") }}
        />
        <InlineIntegrationSetup
          serviceTypes={["googlesearchconsole", "bingwebmaster"]}
          projectId={projectId}
          url={url}
          includeGoogle
          onConnected={() => refreshExternal()}
        />
      </div>
    );
  }

  return (
    <div className="page-content stack-section">
      {hasExternalData && (
        <HeaderActions>
          <Button
            unstyled
            onClick={refreshExternal}
            disabled={gscLoading}
            aria-label="Refresh search data"
            className="refresh-icon-button">
            {gscLoading ? (
              <Loader2 className="icon-md animate-spin text-muted-foreground" />
            ) : (
              <RefreshCw className="icon-md text-muted-foreground" />
            )}
          </Button>
        </HeaderActions>
      )}

      {hasScanData && (
        <>
          {arrivalPrompt ? (
            <WatchedFileArrivalBanner
              prompt={arrivalPrompt}
              onOpenFile={arrivalPrompt.absolutePath ? handleOpenArrivalFile : null}
              onReview={null}
              reviewLabel="Review SEO checks"
            />
          ) : null}

          <PendingSearchVerificationSection
            pendingEntries={pendingSearchEntries}
            sectionRef={pendingSectionRef}
            verifyingAllPending={verifyingAllPending}
            verifyingCheckId={verifyingCheckId}
            verifyingPendingId={verifyingPendingId}
            onVerifyAll={handleVerifyAllPending}
            onVerifyEntry={handleVerifyPendingEntry}
          />
        </>
      )}

      {gscConnected && gscData ? (
        <SearchEngineSection
          title="Google Search Visibility"
          data={gscData}
          period={period}
          setPeriod={setPeriod}
          loading={gscLoading}
          updatedAt={externalLoadedAt}
        />
      ) : (
        <div className="stack-base">
          {gscError ? (
            <p className="text-body text-amber-300 text-relaxed">
              Your Google sign-in expired. Sign in again to reconnect Search Console.
            </p>
          ) : null}
          <InlineIntegrationSetup
            serviceTypes={["googlesearchconsole"]}
            projectId={projectId}
            url={url}
            includeGoogle
            onConnected={() => refreshExternal()}
            allowReconnect={gscError ? ["googlesearchconsole"] : []}
          />
        </div>
      )}

      {bingConnected && bingData ? (
        <BingSection data={bingData} loading={gscLoading} updatedAt={externalLoadedAt} />
      ) : (
        <div className="stack-base">
          {bingError ? (
            <p className="text-body text-amber-300 text-relaxed">
              Bing Webmaster Tools stopped syncing. Reconnect to restore it.
            </p>
          ) : null}
          <InlineIntegrationSetup
            serviceTypes={["bingwebmaster"]}
            projectId={projectId}
            url={url}
            onConnected={() => refreshExternal()}
            allowReconnect={bingError ? ["bingwebmaster"] : []}
          />
        </div>
      )}

      {selectedIssue && (
        <SeoIssueDossier
          issue={selectedIssue}
          detectedStack={searchScan.data?.detectedStack ?? null}
          projectId={projectId}
          url={url}
          projectPath={projectPath}
          arrivalPrompt={arrivalPrompt}
          onClose={() => setSelectedIssueId(null)}
          onVerify={() => handleVerifyIssue(selectedIssue)}
          verifying={verifyingCheckId === selectedIssue.checkId}
          onOpenScan={() => onNavigate("issues")}
        />
      )}
    </div>
  );
}
