import { useState, useEffect, useCallback, useMemo, Suspense } from "react";
import { AddProjectOverlay } from "@/app/AddProjectOverlay";
import type { AppShellHooks } from "@/app/AppProviders";
import { AppRoutes } from "@/app/AppRoutes";
import { HistoryProvider } from "@/app/HistoryProvider";
import type { NavigationContextValue } from "@/app/navigation-context";
import { NavigationProvider } from "@/app/NavigationProvider";
import { useAppProjectCreation } from "@/app/useAppProjectCreation";
import { useAppScanActions } from "@/app/useAppScanActions";
import { useBaselineScanQueue } from "@/app/useBaselineScanQueue";
import { FirstRunWalkthrough } from "@/app/lazy-pages";
import { StartupShell } from "@/app/StartupShell";
import {
  createProjectDeletedHandler,
  getProjectBootstrapState,
  shouldShowTelemetryConsentPrompt,
} from "@/app/app-shell-helpers";
import { useAppKeyboardShortcuts } from "@/app/useAppKeyboardShortcuts";
import { useDeepLinkTargets } from "@/app/useDeepLinkTargets";
import { useDesktopNotificationActions } from "@/app/useDesktopNotificationActions";
import { normalizeShellUrl, useProjectNavBadgeRefresh } from "@/app/useProjectNavBadgeRefresh";
import { formatScanScopeLabel, useScanShellStatus } from "@/app/useScanShellStatus";
import { useScanCompletionEffects } from "@/app/useScanCompletionEffects";
import { usePostScanSummary } from "@/app/usePostScanSummary";
import { useAppTargetNavigation } from "@/app/useAppTargetNavigation";
import { useNavigationState } from "@/app/useNavigationState";
import { useTraySummary } from "@/app/useTraySummary";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { TopBar } from "@/components/layout/TopBar";
import { UpdateBanner } from "@/components/layout/UpdateBanner";
import { JobsTray } from "@/components/layout/JobsTray";
import { ValidationStaleBanner } from "@/components/billing/ValidationStaleBanner";
import { TelemetryConsentPrompt } from "@/components/privacy/TelemetryConsentPrompt";
import { useHasCompletedFirstScan } from "@/lib/onboarding-flags";
import { useLicenseActivateDeepLink } from "@/hooks/useLicenseActivateDeepLink";
import { NavSidebar, type NavTarget } from "@/components/layout/NavSidebar";
import { toNavPage } from "@/components/layout/nav-page";
import { CommandPalette } from "@/components/layout/CommandPalette";
import { ScanSummaryOverlay } from "@/components/scan/ScanSummaryOverlay";
import { useToast } from "@/hooks/useToast";
import { useRenderSanityCheck } from "@/lib/render-sanity";
import { useAlerts } from "@/hooks/useAlerts";
import type { BackgroundJob, BackgroundJobTarget } from "@/lib/jobs";
import { useScanPrefs } from "@/hooks/useScanPrefs";
import { useProject } from "@/hooks/useProject";
import { useDesktopPromptCenter } from "@/lib/desktop-prompts";
import { useDesktopPrefs } from "@/lib/desktop-prefs";
import { readPersistedShellPage, writePersistedShellPage } from "@/lib/app-shell-state";
import { useAppShellOrchestration } from "@/hooks/useAppShellOrchestration";
import { type PostScanFollowUpBanner } from "@/lib/scan-follow-up";
import { loadPrimaryWorkflowCue } from "@/lib/scan-completion-effects";
import { readOnboardingSetupSteps, removeOnboardingSetupStep } from "@/lib/onboarding-setup";

export function AppContent({ scanHook, historyHook }: AppShellHooks) {
  useRenderSanityCheck("AppContent");
  const hasCompletedFirstScan = useHasCompletedFirstScan();
  useLicenseActivateDeepLink();
  const {
    projects,
    projectsLoading,
    projectsLoadError,
    activeProject,
    activeEnv,
    projectFolder,
    selectProject,
    selectEnv,
    refreshProjects,
    retryProjectsLoad,
    handleAddFolder,
  } = useProject();
  const { state: navState, dispatch: navDispatch } = useNavigationState({
    initialPage: toNavPage(readPersistedShellPage()),
  });
  const {
    page,
    issuesTabResetKey,
    settingsTab,
    showCommandPalette,
    issuesTarget,
    searchFocus,
    searchItemId,
    searchLane,
    updatesTarget,
    alertsTarget,
    focusIntegration,
    arrivalPrompt,
  } = navState;
  const openCommandPalette = useCallback(
    () => navDispatch({ type: "SET_COMMAND_PALETTE_OPEN", open: true }),
    [navDispatch],
  );
  const closeCommandPalette = useCallback(
    () => navDispatch({ type: "SET_COMMAND_PALETTE_OPEN", open: false }),
    [navDispatch],
  );
  const [scanFollowUpBanner, setScanFollowUpBanner] = useState<PostScanFollowUpBanner | null>(null);

  const {
    state,
    currentScanType,
    currentExecutionMode,
    result,
    codeResult,
    codeResultFromBackground,
    multiResult,
    executionIncompleteDetail,
    error,
    cancelScan,
  } = scanHook;
  const { prefs, enabledCategories } = useScanPrefs();
  const toast = useToast();
  const { history, codeHistory, sessions, loadHistory } = historyHook;
  const desktopPrompts = useDesktopPromptCenter();
  const { prefs: desktopPrefs } = useDesktopPrefs();
  // Shell badges need counts only; omit unread alert rows from IPC.
  const alertsHook = useAlerts(activeProject?.id ?? null, "unread", {
    includeRows: false,
    deferMs: 2000,
  });
  const alertsBadge = alertsHook.unreadCount > 0 ? alertsHook.unreadCount : null;
  const alertsCriticalBadge =
    alertsHook.unreadCriticalCount > 0 ? alertsHook.unreadCriticalCount : null;
  const activeProjectId = activeProject?.id ?? null;
  const activeEnvUrl = activeEnv?.url ?? null;
  // A project can scan when it has either an environment or a code folder.
  const canScanActiveProject = Boolean(activeEnv) || Boolean(projectFolder);
  const normalizeUrl = normalizeShellUrl;
  const activeScanScope = formatScanScopeLabel(activeProject?.name ?? null, activeEnv?.url ?? null);

  useProjectNavBadgeRefresh({
    activeProjectId,
    activeEnvUrl,
    activeProjectPath: projectFolder,
    result,
    codeResult,
  });

  useEffect(() => {
    writePersistedShellPage(page);
  }, [page]);

  const {
    closeScanConfig,
    handleQuickScan,
    handleScan,
    handleShortcutScan,
    openScanConfig,
    openTrayScanConfig,
    scanBackgrounded,
    scanBackgroundedRef,
    scanConfigPreset,
    scanJobContextRef,
    scanRunStep,
    showBackgroundedScan,
    showScanConfig,
    updateScanBackgrounded,
  } = useAppScanActions({
    activeEnv,
    activeProject,
    enabledCategories,
    prefs,
    projectFolder,
    scanHook,
    toast,
  });

  const { navigateTo, openAppTarget, openOverviewProject, openProjectSettings } =
    useAppTargetNavigation({
      activeEnv,
      activeProject,
      projects,
      projectsLoading,
      selectProject,
      selectEnv,
      dispatch: navDispatch,
      openScanConfig,
      updateScanBackgrounded,
    });

  const runProjectBaselineScan = useCallback(() => {
    // Empty urls: planScan falls back to the active environment URL, which is
    // fresh by the time the queue fires this.
    void handleScan({ urls: [], axeEnabled: false, scanType: "full" });
  }, [handleScan]);

  const { queueBaselineScan } = useBaselineScanQueue({
    activeProjectId: activeProject?.id ?? null,
    canScan: canScanActiveProject,
    runBaselineScan: runProjectBaselineScan,
  });

  const { closeAddProject, handleProjectCreated, openAddProject, showAddProject } =
    useAppProjectCreation({
      refreshProjects,
      queueBaselineScan,
      selectProject,
    });

  useScanShellStatus({
    activeEnvUrl: activeEnv?.url ?? null,
    activeProjectId: activeProject?.id ?? null,
    activeScanScope,
    currentScanType,
    scanRunStep,
    scanJobContextRef,
    state,
  });

  useScanCompletionEffects({
    state,
    currentScanType,
    currentExecutionMode,
    result,
    codeResult,
    multiResult,
    executionIncompleteDetail,
    codeResultFromBackground,
    error,
    activeEnvUrl: activeEnv?.url,
    activeProjectId: activeProject?.id,
    activeProjectName: activeProject?.name,
    activeScanScope,
    history,
    codeHistory,
    scanBackgroundedRef,
    scanJobContextRef,
    desktopNotificationsEnabled: desktopPrefs.desktopNotifications,
    loadHistory,
    openAppTarget,
    refreshProjects,
    setScanFollowUpBanner,
    toast,
  });

  useAppKeyboardShortcuts({
    activeEnvUrl: activeEnv?.url ?? null,
    enabledCategories,
    navigateTo,
    openAddProject,
    openCommandPalette,
    openScanConfig,
    page,
    scan: handleShortcutScan,
    scanState: state,
    timeout: prefs.timeout,
  });

  // Shell listeners live in a dedicated hook so App.tsx can stay focused on
  // navigation and rendering instead of long-lived background orchestration.
  useAppShellOrchestration({
    projects,
    projectsLoading,
    refreshProjects,
    selectProject,
    navigateTo,
    openTrayScanConfig,
    showBackgroundedScan,
    loadHistory,
    toast,
    desktopPrefs,
    normalizeUrl,
    loadPrimaryWorkflowCue,
  });

  useTraySummary({
    desktopPrompts,
    projects,
  });

  const handleCommandAction = useCallback(
    (action: string) => {
      if (action === "scan") openScanConfig();
      if (action === "add-project") openAddProject();
    },
    [openScanConfig, openAddProject],
  );

  const handleOpenJob = useCallback(
    (job: BackgroundJob) => {
      const target = resolveJobTarget(job.target, {
        page: "dashboard",
        projectId: activeProject?.id,
        url: activeEnv?.url,
      });
      openAppTarget(target);
    },
    [activeEnv?.url, activeProject?.id, openAppTarget],
  );

  const firstRunWalkthroughKey = `${activeProject?.id ?? "none"}:${activeEnv?.url ?? "none"}:${
    result?.timestamp ?? codeResult?.checkedAt ?? "no-scan"
  }`;
  const [dismissedFirstRunWalkthroughKey, setDismissedFirstRunWalkthroughKey] = useState<
    string | null
  >(null);
  // Full Scan step markers are presentational; the scan hook owns lifecycle.
  const fullScanStillRunning = state === "scanning";

  const { closeScanSummary, scanSummary, showScanSummary } = usePostScanSummary({
    state,
    currentExecutionMode,
    result,
    codeResult,
    multiResult,
    executionIncompleteDetail,
    activeProjectId,
    activeEnvUrl,
    activeScanScope,
    fullScanStillRunning,
    scanBackgrounded,
    codeResultFromBackground,
    history,
    codeHistory,
    sessions,
    showScanConfig,
  });
  const reviewScanSummaryIssues = useCallback(() => {
    closeScanSummary();
    setScanFollowUpBanner(null);
    navigateTo("issues");
  }, [closeScanSummary, navigateTo]);
  const showFirstRunWalkthrough =
    Boolean(activeProject && canScanActiveProject && (result || codeResult)) &&
    !fullScanStillRunning &&
    !showScanSummary &&
    dismissedFirstRunWalkthroughKey !== firstRunWalkthroughKey &&
    (activeProjectId != null
      ? readOnboardingSetupSteps(activeProjectId).includes("baseline-review")
      : false);

  const closeFirstRunWalkthrough = useCallback(() => {
    if (activeProjectId != null) {
      removeOnboardingSetupStep(activeProjectId, "baseline-review");
    }
    setDismissedFirstRunWalkthroughKey(firstRunWalkthroughKey);
  }, [activeProjectId, firstRunWalkthroughKey]);

  const handleFirstRunWalkthroughNavigate = useCallback(
    (target: NavTarget) => {
      navigateTo(target);
    },
    [navigateTo],
  );

  const showTelemetryConsentPrompt = shouldShowTelemetryConsentPrompt({
    hasCompletedFirstScan,
    projectCount: projects.length,
    showScanSummary,
    showFirstRunWalkthrough,
  });

  const handleProjectDeleted = useMemo(
    () => createProjectDeletedHandler({ refreshProjects, navigateTo }),
    [navigateTo, refreshProjects],
  );

  useDeepLinkTargets(openAppTarget);
  useDesktopNotificationActions(openAppTarget);

  // Stable identity prevents unrelated shell updates from rerendering every route.
  const navigationContextValue = useMemo<NavigationContextValue>(
    () => ({
      page,
      settingsTab,
      issuesTarget,
      issuesTabResetKey,
      searchFocus,
      searchItemId,
      searchLane,
      updatesTarget,
      alertsTarget,
      focusIntegration,
      arrivalPrompt,
    }),
    [
      page,
      settingsTab,
      issuesTarget,
      issuesTabResetKey,
      searchFocus,
      searchItemId,
      searchLane,
      updatesTarget,
      alertsTarget,
      focusIntegration,
      arrivalPrompt,
    ],
  );

  const bootstrapState = getProjectBootstrapState({
    projectCount: projects.length,
    projectsLoading,
    projectsLoadError,
    showAddProject,
  });

  if (bootstrapState) {
    // Ask for telemetry consent only after the first scan demonstrates value.
    return (
      <StartupShell
        state={bootstrapState}
        projects={projects}
        onAddProject={openAddProject}
        onOpenSearch={openCommandPalette}
        onRetryProjectsLoad={() => {
          void retryProjectsLoad();
        }}
      />
    );
  }

  return (
    <div className="app-shell">
      <TopBar
        projects={projects}
        activeProject={activeProject}
        activeEnv={activeEnv}
        onSelectProject={selectProject}
        onOpenProjectSettings={openProjectSettings}
        onSelectEnv={selectEnv}
        onAddProject={() => openAddProject()}
        onOpenSearch={openCommandPalette}
        onRunScan={canScanActiveProject ? handleQuickScan : undefined}
        onOpenScanConfig={canScanActiveProject ? () => openScanConfig() : undefined}
        scanning={state === "scanning"}
      />
      <div className="app-body">
        <NavSidebar
          activePage={page}
          activeProjectId={activeProject?.id}
          projectCount={projects.length}
          hasLinkedFolder={Boolean(projectFolder)}
          onNavigate={navigateTo}
          alertsBadge={alertsBadge}
          alertsCriticalBadge={alertsCriticalBadge}
        />
        <main className="app-main">
          <ErrorBoundary>
            <UpdateBanner />
            <ValidationStaleBanner />
            <div className="app-content-pad">
              <NavigationProvider value={navigationContextValue}>
                <HistoryProvider value={historyHook}>
                  <AppRoutes
                    page={page}
                    activeProject={activeProject}
                    activeEnv={activeEnv}
                    projectFolder={projectFolder}
                    state={state}
                    currentScanType={currentScanType}
                    scanRunStep={scanRunStep}
                    result={result}
                    codeResult={codeResult}
                    scanBackgrounded={scanBackgrounded}
                    showScanConfig={showScanConfig}
                    scanConfigPreset={scanConfigPreset}
                    scanFollowUpBanner={scanFollowUpBanner}
                    onStartScan={handleScan}
                    onCloseScanConfig={closeScanConfig}
                    onCancelScan={cancelScan}
                    onBackgroundScan={updateScanBackgrounded}
                    onClearScanFollowUpBanner={() => setScanFollowUpBanner(null)}
                    onNavigate={navigateTo}
                    onOpenTarget={openAppTarget}
                    onQuickScan={handleQuickScan}
                    onOpenScanConfig={openScanConfig}
                    onAddFolder={handleAddFolder}
                    onOpenOverviewProject={openOverviewProject}
                    onClearFocusIntegration={() =>
                      navDispatch({ type: "SET_FOCUS_INTEGRATION", value: null })
                    }
                    onProjectChanged={refreshProjects}
                    onProjectDeleted={handleProjectDeleted}
                    openAddProject={() => openAddProject()}
                  />
                </HistoryProvider>
              </NavigationProvider>
            </div>
          </ErrorBoundary>
          <JobsTray onOpenJob={handleOpenJob} />
          {showFirstRunWalkthrough && activeProject ? (
            <Suspense fallback={null}>
              <FirstRunWalkthrough
                currentPage={page}
                projectName={activeProject.name}
                onClose={closeFirstRunWalkthrough}
                onNavigate={handleFirstRunWalkthroughNavigate}
              />
            </Suspense>
          ) : null}
        </main>
      </div>
      {showAddProject && (
        <AddProjectOverlay
          onCreated={handleProjectCreated}
          onCancel={closeAddProject}
          onNavigate={navigateTo}
        />
      )}
      <CommandPalette
        open={showCommandPalette}
        onClose={closeCommandPalette}
        onNavigate={navigateTo}
        onAction={handleCommandAction}
      />
      {showScanSummary && scanSummary ? (
        <ScanSummaryOverlay
          summary={scanSummary}
          onClose={closeScanSummary}
          onReviewIssues={reviewScanSummaryIssues}
        />
      ) : null}
      {showTelemetryConsentPrompt ? <TelemetryConsentPrompt /> : null}
    </div>
  );
}

function resolveJobTarget(
  target: BackgroundJobTarget | null | undefined,
  fallback: {
    page: "dashboard";
    projectId?: number | null;
    url?: string | null;
  },
) {
  if (!target) return fallback;
  if ("page" in target) return target;
  return {
    page: "dashboard" as const,
    restoreScan: true,
    projectId: target.projectId ?? fallback.projectId,
    url: target.url ?? fallback.url,
  };
}
