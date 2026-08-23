import { Suspense } from "react";
import { ScanFollowUpBannerHost } from "@/components/layout/ScanFollowUpBannerHost";
import type { NavPage, NavTarget } from "@/components/layout/NavSidebar";
import type { ScanConfig, ScanConfigPreset } from "@/components/scan/ScanConfigOverlay";
import { useNavigation } from "@/app/navigation-context";
import type { EnvironmentRecord, ProjectRecord } from "@/hooks/useProject";
import type { ScanState } from "@/hooks/useScan";
import type { AppTarget } from "@/lib/app-targets";
import { getProjectCapabilities, NO_SITE_SCOPE_URL } from "@/lib/project-capabilities";
import type { PostScanFollowUpBanner } from "@/lib/scan-follow-up";
import { useScanProgress } from "@/lib/scan-progress-store";
import type { ScanRunStep } from "@/lib/scan-run-status";
import type { CodeScanResult, ScanResult, ScheduledScanType } from "@/lib/types";
import {
  AlertsPage,
  AnalyticsPage,
  Dashboard,
  DeploysPage,
  EventsPage,
  IntegrationsPage,
  IssuesPage,
  ReportsPage,
  ScanConfigOverlay,
  ScanOverlay,
  SearchConsolePage,
  SettingsPage,
  SitesOverview,
  UpdatesPage,
} from "@/app/lazy-pages";
import { ShellPageHeader, ShellPageLoading } from "@/app/ShellHeader";
import { SurfaceState } from "@/components/ui/surface-state";

interface AppRoutesProps {
  page: NavPage;
  activeProject: ProjectRecord | null;
  activeEnv: EnvironmentRecord | null;
  projectFolder: string | null;
  state: ScanState;
  currentScanType: ScheduledScanType | null;
  scanRunStep: ScanRunStep | null;
  result: ScanResult | null;
  codeResult: CodeScanResult | null;
  scanBackgrounded: boolean;
  showScanConfig: boolean;
  scanConfigPreset: ScanConfigPreset | null;
  scanFollowUpBanner: PostScanFollowUpBanner | null;
  onStartScan: (config?: ScanConfig) => void | Promise<void>;
  onCloseScanConfig: () => void;
  onCancelScan: () => void | Promise<void>;
  onBackgroundScan: (backgrounded: boolean) => void;
  onClearScanFollowUpBanner: () => void;
  onNavigate: (target: NavTarget) => void;
  onOpenTarget: (target: AppTarget) => void;
  onQuickScan: () => void;
  onOpenScanConfig: (preset?: ScanConfigPreset) => void;
  onAddFolder: () => void | Promise<void>;
  onOpenOverviewProject: (projectId: number) => void;
  onClearFocusIntegration: () => void;
  onProjectChanged: () => void | Promise<unknown>;
  onProjectDeleted: () => void | Promise<unknown>;
  openAddProject: () => void;
}

export function AppRoutes({
  page,
  activeProject,
  activeEnv,
  projectFolder,
  state,
  currentScanType,
  scanRunStep,
  result,
  codeResult,
  scanBackgrounded,
  showScanConfig,
  scanConfigPreset,
  scanFollowUpBanner,
  onStartScan,
  onCloseScanConfig,
  onCancelScan,
  onBackgroundScan,
  onClearScanFollowUpBanner,
  onNavigate,
  onOpenTarget,
  onQuickScan,
  onOpenScanConfig,
  onAddFolder,
  onOpenOverviewProject,
  onClearFocusIntegration,
  onProjectChanged,
  onProjectDeleted,
  openAddProject,
}: AppRoutesProps) {
  // Bridge context targets into pages that still expose prop-based APIs.
  const {
    settingsTab,
    searchFocus,
    searchItemId,
    searchLane,
    updatesTarget,
    alertsTarget,
    focusIntegration,
    arrivalPrompt,
  } = useNavigation();
  const capabilities = getProjectCapabilities({
    environmentUrl: activeEnv?.url ?? null,
    projectFolder,
  });
  // Code-only projects have no environment; require either a site or codebase.
  if (!activeProject || (!activeEnv && !capabilities.hasCode)) {
    return (
      <InactiveProjectRoutes
        page={page}
        activeProjectId={activeProject?.id}
        onOpenOverviewProject={onOpenOverviewProject}
        openAddProject={openAddProject}
      />
    );
  }

  // Code-only findings use the backend's empty environment scope.
  const scopeUrl = activeEnv?.url ?? NO_SITE_SCOPE_URL;
  const environmentId = activeEnv?.id;

  return (
    <>
      <ShellPageHeader page={page} showScanHeader={false} />

      <Suspense fallback={<ShellPageLoading page={page} />}>
        {showScanConfig && (
          <ScanConfigOverlay
            projectId={activeProject.id}
            siteUrl={scopeUrl}
            projectPath={projectFolder}
            onStart={onStartScan}
            onCancel={onCloseScanConfig}
            initialScanType={scanConfigPreset?.scanType}
            initialAxeEnabled={scanConfigPreset?.axeEnabled}
          />
        )}
        {state === "scanning" && !scanBackgrounded && (
          <LiveScanOverlay
            scanType={currentScanType}
            scanRunStep={scanRunStep}
            url={scopeUrl}
            onCancel={onCancelScan}
            onMinimize={() => onBackgroundScan(true)}
          />
        )}
        <ScanFollowUpBannerHost
          page={page}
          scanState={state}
          banner={scanFollowUpBanner}
          onOpenTarget={onOpenTarget}
          onClearBanner={onClearScanFollowUpBanner}
        />

        {page === "dashboard" && (
          <Dashboard
            key={`dashboard:${activeProject.id}:${environmentId ?? "code-only"}`}
            url={scopeUrl}
            projectId={activeProject.id}
            projectName={activeProject.name}
            framework={activeProject.framework || null}
            projectPath={projectFolder}
            onViewResults={() => onNavigate("issues")}
            onRescan={onQuickScan}
            onViewCodeScan={() => onNavigate("issues")}
            onOpenScanConfig={onOpenScanConfig}
            onOpenCodeScanConfig={() => onOpenScanConfig({ scanType: "code" })}
            onAddFolder={onAddFolder}
            onNavigate={onNavigate}
            onOpenTarget={onOpenTarget}
            scanning={state === "scanning"}
            latestResult={result}
            latestCodeResult={codeResult}
          />
        )}
        {page === "analytics" && (
          <AnalyticsPage projectId={activeProject.id} url={scopeUrl} onNavigate={onNavigate} />
        )}
        {page === "issues" && (
          <Suspense fallback={<ShellPageLoading page={page} />}>
            <IssuesPage
              projectId={activeProject.id}
              url={scopeUrl}
              environmentId={environmentId}
              latestResult={result}
              latestCodeResult={codeResult}
              projectPath={projectFolder}
              onNavigate={onNavigate}
              openScanConfig={onOpenScanConfig}
            />
          </Suspense>
        )}
        {page === "deploys" && (
          <DeploysPage
            projectPath={projectFolder}
            projectId={activeProject.id}
            url={scopeUrl}
            onScan={onQuickScan}
            scanning={state === "scanning"}
            onViewScan={() => onNavigate("issues")}
            onAddFolder={onAddFolder}
          />
        )}
        {page === "updates" && (
          <UpdatesPage
            projectId={activeProject.id}
            url={scopeUrl}
            projectPath={projectFolder}
            projectName={activeProject.name}
            onAddFolder={onAddFolder}
            initialTarget={updatesTarget}
            arrivalPrompt={arrivalPrompt?.page === "updates" ? arrivalPrompt.entry : null}
          />
        )}
        {page === "events" && (
          <EventsPage projectId={activeProject.id} onOpenTarget={onOpenTarget} />
        )}
        {page === "search-console" && (
          <SearchConsolePage
            projectId={activeProject.id}
            url={scopeUrl}
            projectPath={projectFolder || undefined}
            onNavigate={onNavigate}
            initialFocus={searchFocus}
            initialItemId={searchItemId}
            initialLane={searchLane}
            arrivalPrompt={arrivalPrompt?.page === "search-console" ? arrivalPrompt.entry : null}
          />
        )}
        {page === "integrations" && (
          <IntegrationsPage
            projectId={activeProject.id}
            projectName={activeProject.name}
            url={scopeUrl}
            focusIntegration={focusIntegration}
            onFocusHandled={onClearFocusIntegration}
          />
        )}
        {page === "sites" && (
          <SitesOverview
            currentProjectId={activeProject.id}
            onSelectProject={onOpenOverviewProject}
            onAddProject={openAddProject}
          />
        )}
        {page === "alerts" && (
          <AlertsPage
            projectId={activeProject.id}
            environmentScopeKey={scopeUrl}
            onNavigate={onNavigate}
            deepLinkTarget={alertsTarget}
          />
        )}
        {page === "reports" && (
          <ReportsPage
            projectId={activeProject.id}
            siteUrl={scopeUrl}
            projectPath={projectFolder}
          />
        )}
        {page === "settings" && (
          <SettingsPage
            projectId={activeProject.id}
            environmentId={environmentId}
            framework={activeProject.framework ?? undefined}
            projectName={activeProject.name}
            url={scopeUrl}
            projectPath={projectFolder}
            initialTab={settingsTab}
            projectEnvironments={activeProject.environments}
            onProjectChanged={onProjectChanged}
            onProjectDeleted={onProjectDeleted}
          />
        )}
      </Suspense>
    </>
  );
}

/** Isolates high-frequency scan progress updates to the overlay. */
function LiveScanOverlay(props: {
  scanType: ScheduledScanType | null;
  scanRunStep: ScanRunStep | null;
  url: string;
  onCancel: () => void | Promise<void>;
  onMinimize: () => void;
}) {
  const { progress, multiProgress } = useScanProgress();
  return <ScanOverlay progress={progress} multiProgress={multiProgress} {...props} />;
}

export function InactiveProjectRoutes({
  page,
  activeProjectId,
  onOpenOverviewProject,
  openAddProject,
}: {
  page: NavPage;
  activeProjectId?: number;
  onOpenOverviewProject: (projectId: number) => void;
  openAddProject: () => void;
}) {
  if (page === "sites") {
    return (
      <Suspense fallback={<ShellPageLoading page={page} />}>
        <SitesOverview
          currentProjectId={activeProjectId}
          onSelectProject={onOpenOverviewProject}
          onAddProject={openAddProject}
        />
      </Suspense>
    );
  }

  if (page === "settings") {
    return <SettingsPage />;
  }

  return (
    <SurfaceState
      kind="empty"
      className="inactive-route-empty"
      title="Select a project to get started"
      description="Pick a project from the top bar, or add a new site or folder now."
      primaryAction={{ label: "Add Project", onClick: openAddProject }}
    />
  );
}
