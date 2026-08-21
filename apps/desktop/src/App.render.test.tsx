import React from "react";
import { act, fireEvent, render as rtlRender, screen, waitFor } from "@testing-library/react";
import { withQueryClient } from "@/test-utils/query-client";

// The app shell (AppContent) calls useQueryClient/useQuery, so every render
// needs a QueryClientProvider. A fresh client per render keeps tests isolated.
const render = (ui: Parameters<typeof rtlRender>[0], options?: Parameters<typeof rtlRender>[1]) =>
  rtlRender(ui, { wrapper: withQueryClient(), ...options });
import { beforeEach, describe, expect, it, vi } from "vitest";

// This is a shell-routing contract test, not full desktop orchestration
// coverage. It keeps the Dashboard-first shell routing seam honest.

import type { EnvironmentRecord, ProjectRecord } from "@/hooks/useProject";
import type { NavTarget } from "@/components/layout/nav-page";

const {
  invokeMock,
  useProjectMock,
  useTierMock,
  useScanMock,
  useHistoryMock,
  refreshProjectsMock,
  selectProjectMock,
  getProjectNavBadgeSnapshotMock,
  publishProjectNavBadgesMock,
  getCurrentDeepLinksMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  useProjectMock: vi.fn(),
  useTierMock: vi.fn(),
  useScanMock: vi.fn(),
  useHistoryMock: vi.fn(),
  refreshProjectsMock: vi.fn(),
  selectProjectMock: vi.fn(),
  getProjectNavBadgeSnapshotMock: vi.fn(),
  publishProjectNavBadgesMock: vi.fn(),
  getCurrentDeepLinksMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-deep-link", () => ({
  getCurrent: () => getCurrentDeepLinksMock(),
  onOpenUrl: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn(() => Promise.resolve()),
  unregisterAll: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/components/ui/error-boundary", () => ({
  ErrorBoundary: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
}));

vi.mock("@/components/layout/TopBar", () => ({
  TopBar: () => React.createElement("div", { "data-testid": "top-bar" }, "TopBar"),
}));

vi.mock("@/components/layout/UpdateBanner", () => ({
  UpdateBanner: () => null,
}));

vi.mock("@/components/layout/ScanFollowUpBannerHost", () => ({
  ScanFollowUpBannerHost: () => null,
}));

vi.mock("@/components/layout/NavSidebar", () => ({
  NavSidebar: ({
    activePage,
    projectCount,
    onNavigate,
  }: {
    activePage: string;
    projectCount: number;
    onNavigate: (page: NavTarget) => void;
  }) =>
    React.createElement("nav", null, [
      React.createElement(
        "button",
        {
          key: "dashboard",
          type: "button",
          "aria-current": activePage === "dashboard" ? "page" : undefined,
          onClick: () => onNavigate("dashboard"),
        },
        "Dashboard",
      ),
      projectCount > 1
        ? React.createElement(
            "button",
            {
              key: "sites",
              type: "button",
              "aria-current": activePage === "sites" ? "page" : undefined,
              onClick: () => onNavigate("sites"),
            },
            "Overview",
          )
        : null,
    ]),
}));

vi.mock("@/components/layout/CommandPalette", () => ({
  CommandPalette: () => null,
}));

vi.mock("@/hooks/useProject", () => ({
  ProjectProvider: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
  useProject: () => useProjectMock(),
}));

vi.mock("@/hooks/useTier", () => ({
  TierProvider: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
  useTier: () => useTierMock(),
}));

vi.mock("@/hooks/useScan", () => ({
  useScan: () => useScanMock(),
}));

vi.mock("@/hooks/useHistory", () => ({
  useHistory: () => useHistoryMock(),
}));

vi.mock("@/hooks/useScanPrefs", () => ({
  useScanPrefs: () => ({
    prefs: { timeout: 30, retentionLimit: 10 },
    enabledCategories: [],
  }),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
    toast: vi.fn(),
  }),
}));

vi.mock("@/lib/jobs", () => ({
  addJob: vi.fn(),
  completeJob: vi.fn(),
  failJob: vi.fn(),
  removeRunningJob: vi.fn(),
  useJobs: () => [],
  useRunningJobsCount: () => 0,
}));

vi.mock("@/lib/desktop-prompts", () => ({
  getDesktopPromptById: vi.fn(() => null),
  resolveDesktopPrompt: vi.fn(),
  useDesktopPromptCenter: () => [],
}));

vi.mock("@/lib/actionable-notifications", () => ({
  sendActionableDesktopNotification: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/desktop-actions", () => ({
  openPathInEditor: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: {
      desktopNotifications: false,
      backgroundMonitoring: false,
      fileWatchSuggestions: false,
    },
  }),
}));

vi.mock("@/lib/pending-verification", () => ({
  usePendingVerificationCenter: () => [],
}));

vi.mock("@/lib/open-url", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/update-priority", () => ({
  buildUpdateCampaignCopy: vi.fn(() => null),
}));

vi.mock("@/lib/onboarding-setup", () => ({
  consumeOnboardingSetupStepForTarget: vi.fn(),
}));

vi.mock("@/lib/scan-completion-effects", () => ({
  handleCodeScanCompletion: vi.fn(),
  handleMultiScanCompletion: vi.fn(),
  handleWebScanCompletion: vi.fn(),
  loadPrimaryWorkflowCue: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@/hooks/useAppShellOrchestration", () => ({
  IMPORT_SYNC_WINDOW_MS: 30_000,
  useAppShellOrchestration: vi.fn(),
}));

vi.mock("@/lib/project-summary-signals", () => ({
  getProjectNavBadgeSnapshot: (...args: unknown[]) => getProjectNavBadgeSnapshotMock(...args),
}));

vi.mock("@/lib/project-nav-badges", () => ({
  publishProjectNavBadges: (...args: unknown[]) => publishProjectNavBadgesMock(...args),
}));

vi.mock("@/components/dashboard/Dashboard", () => ({
  Dashboard: ({ projectName }: { projectName: string }) =>
    React.createElement("div", null, `Dashboard page for ${projectName}`),
}));

vi.mock("@/components/dashboard/UpdatesPage", () => ({
  UpdatesPage: ({ projectName }: { projectName: string }) =>
    React.createElement("div", null, `Updates page for ${projectName}`),
}));

vi.mock("@/components/sites/SitesOverview", () => ({
  SitesOverview: ({
    currentProjectId,
    onSelectProject,
  }: {
    currentProjectId?: number;
    onSelectProject: (projectId: number) => void;
  }) =>
    React.createElement("div", null, [
      React.createElement("div", { key: "title" }, `Overview page for project ${currentProjectId}`),
      React.createElement(
        "button",
        { key: "select-beta", type: "button", onClick: () => onSelectProject(2) },
        "Open Beta dashboard",
      ),
    ]),
}));

function buildEnv(id: number, url: string): EnvironmentRecord {
  return {
    id,
    url,
    label: url,
    environment: "production",
    source: "manual",
    lastScannedAt: null,
    latestScore: 84,
  };
}

function buildProject(id: number, name: string, env: EnvironmentRecord): ProjectRecord {
  return {
    id,
    name,
    path: `/tmp/${name.toLowerCase()}`,
    framework: "nextjs",
    createdAt: "2026-04-15T12:00:00Z",
    environments: [env],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function buildBadgeSnapshot(overrides: Record<string, unknown> = {}) {
  return {
    aggregatedFailedIssues: [],
    inactiveCheckIds: [],
    signals: {
      updates: { updates: [] },
      codeScanSummary: null,
      codeScanDetail: null,
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
    ...overrides,
  };
}

describe("App shell rendering", () => {
  beforeEach(() => {
    vi.useRealTimers();
    window.localStorage.clear();
    invokeMock.mockReset();
    useProjectMock.mockReset();
    useTierMock.mockReset();
    useScanMock.mockReset();
    useHistoryMock.mockReset();
    refreshProjectsMock.mockReset();
    selectProjectMock.mockReset();
    getProjectNavBadgeSnapshotMock.mockReset();
    publishProjectNavBadgesMock.mockReset();
    getCurrentDeepLinksMock.mockReset();
    invokeMock.mockResolvedValue(null);
    getCurrentDeepLinksMock.mockResolvedValue([]);
    getProjectNavBadgeSnapshotMock.mockResolvedValue(buildBadgeSnapshot());

    const alphaEnv = buildEnv(11, "https://alpha.test");
    const betaEnv = buildEnv(21, "https://beta.test");
    const alphaProject = buildProject(1, "Alpha", alphaEnv);
    const betaProject = buildProject(2, "Beta", betaEnv);

    useProjectMock.mockReturnValue({
      projects: [alphaProject, betaProject],
      projectsLoading: false,
      projectsLoadError: null,
      activeProject: alphaProject,
      activeEnv: alphaEnv,
      projectFolder: alphaProject.path,
      selectProject: selectProjectMock,
      selectEnv: vi.fn(),
      refreshProjects: refreshProjectsMock.mockResolvedValue({
        projects: [alphaProject, betaProject],
        newProject: null,
      }),
      retryProjectsLoad: vi.fn(() => Promise.resolve()),
      handleAddFolder: vi.fn(() => Promise.resolve()),
    });

    useTierMock.mockReturnValue({
      hasFeature: vi.fn(() => false),
      licenseInfo: {
        checkoutUrls: {
          coreMonthly: "",
          coreAnnual: "",
          proMonthly: "",
          proAnnual: "",
        },
      },
    });

    useScanMock.mockReturnValue({
      state: "idle",
      currentScanType: null,
      result: null,
      codeResult: null,
      multiResult: null,
      error: null,
      progress: null,
      multiProgress: null,
      scan: vi.fn(),
      scanCode: vi.fn(),
      scanMulti: vi.fn(),
      cancelScan: vi.fn(),
      reset: vi.fn(),
    });

    useHistoryMock.mockReturnValue({
      history: [],
      executions: [],
      codeHistory: [],
      sessions: [],
      loading: false,
      historyError: null,
      loadHistory: vi.fn(() => Promise.resolve()),
    });

    window.localStorage.setItem("sitecmd_shell_state_v1", JSON.stringify({ page: "sites" }));
  });

  it("restores to Dashboard first, then routes through Overview back into the chosen project dashboard", async () => {
    const { default: App } = await import("./App");

    render(<App />);

    expect(screen.getByRole("button", { name: "Overview" })).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText("Dashboard page for Alpha")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Overview" }));

    await waitFor(() => {
      expect(screen.getByText("Overview page for project 1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Open Beta dashboard" }));

    await waitFor(() => {
      expect(screen.getByText("Dashboard page for Alpha")).toBeInTheDocument();
    });

    expect(selectProjectMock).toHaveBeenCalledWith(
      expect.objectContaining({ id: 2, name: "Beta" }),
    );
  }, 20_000);

  it("publishes sidebar badge data when the active project loads", async () => {
    const { default: App } = await import("./App");

    render(<App />);

    await waitFor(() => {
      expect(getProjectNavBadgeSnapshotMock).toHaveBeenCalledWith(
        expect.anything(),
        1,
        "https://alpha.test",
      );
    });

    await waitFor(() => {
      expect(publishProjectNavBadgesMock).toHaveBeenCalledWith(
        1,
        expect.objectContaining({
          aggregatedFailedIssues: [],
          inactiveCheckIds: [],
        }),
      );
    });
  });

  it("keeps Launch Plan out of the sidebar", async () => {
    const { default: App } = await import("./App");

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("Dashboard page for Alpha")).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: "Launch Plan" })).not.toBeInTheDocument();
  });

  it("hydrates nav badges once on initial load without a forced follow-up refresh", async () => {
    const { default: App } = await import("./App");

    render(<App />);

    await waitFor(() => {
      expect(getProjectNavBadgeSnapshotMock).toHaveBeenCalledWith(
        expect.anything(),
        1,
        "https://alpha.test",
      );
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getProjectNavBadgeSnapshotMock).toHaveBeenCalledTimes(1);
  });

  it("ignores stale badge snapshots after switching to a different project", async () => {
    const { default: App } = await import("./App");

    const alphaEnv = buildEnv(11, "https://alpha.test");
    const betaEnv = buildEnv(21, "https://beta.test");
    const alphaProject = buildProject(1, "Alpha", alphaEnv);
    const betaProject = buildProject(2, "Beta", betaEnv);
    const alphaSnapshot = buildBadgeSnapshot({
      aggregatedFailedIssues: [
        {
          check_id: "alpha-critical",
          category: "security",
          title: "Alpha issue",
          description: "Only belongs to alpha",
          status: "fail",
          severity: "critical",
        },
      ],
    });
    const betaSnapshot = buildBadgeSnapshot({
      signals: {
        updates: {
          updates: [
            {
              ecosystem: "npm",
              name: "react",
              current_version: "18.0.0",
              latest_version: "19.0.0",
              update_type: "major",
              is_security: false,
              advisory_severity: null,
              advisory_url: null,
              source: "npm",
              is_dev: false,
            },
          ],
        },
        codeScanSummary: null,
        codeScanDetail: null,
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
    });
    const alphaDeferred = deferred<typeof alphaSnapshot>();
    let currentProject = alphaProject;
    let currentEnv = alphaEnv;

    selectProjectMock.mockImplementation((project: ProjectRecord) => {
      currentProject = project;
      currentEnv = project.environments[0];
    });

    useProjectMock.mockImplementation(() => ({
      projects: [alphaProject, betaProject],
      projectsLoading: false,
      projectsLoadError: null,
      activeProject: currentProject,
      activeEnv: currentEnv,
      projectFolder: currentProject.path,
      selectProject: selectProjectMock,
      selectEnv: vi.fn(),
      refreshProjects: refreshProjectsMock.mockResolvedValue({
        projects: [alphaProject, betaProject],
        newProject: null,
      }),
      retryProjectsLoad: vi.fn(() => Promise.resolve()),
      handleAddFolder: vi.fn(() => Promise.resolve()),
    }));

    getProjectNavBadgeSnapshotMock.mockImplementation(
      (_queryClient: unknown, projectId: number) => {
        if (projectId === 1) return alphaDeferred.promise;
        if (projectId === 2) return Promise.resolve(betaSnapshot);
        return Promise.resolve(buildBadgeSnapshot());
      },
    );

    render(<App />);

    await waitFor(() => {
      expect(getProjectNavBadgeSnapshotMock).toHaveBeenCalledWith(
        expect.anything(),
        1,
        "https://alpha.test",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Overview" }));

    await waitFor(() => {
      expect(screen.getByText("Overview page for project 1")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Open Beta dashboard" }));

    await waitFor(() => {
      expect(getProjectNavBadgeSnapshotMock).toHaveBeenCalledWith(
        expect.anything(),
        2,
        "https://beta.test",
      );
    });

    await waitFor(() => {
      expect(publishProjectNavBadgesMock).toHaveBeenCalledWith(2, betaSnapshot);
    });

    await act(async () => {
      alphaDeferred.resolve(alphaSnapshot);
      await Promise.resolve();
    });

    expect(publishProjectNavBadgesMock).not.toHaveBeenCalledWith(1, alphaSnapshot);
  });
});
