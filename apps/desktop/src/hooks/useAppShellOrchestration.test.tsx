import { StrictMode, useEffect } from "react";
import { act, render, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, safeListenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  safeListenMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@/lib/tauri-events", () => ({
  safeListen: (...args: unknown[]) => safeListenMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(async () => {}),
}));

import { useAppShellOrchestration } from "./useAppShellOrchestration";
import {
  ProjectProvider,
  useProject,
  type EnvironmentRecord,
  type ProjectRecord,
} from "./useProject";
import { resetActiveSelectionForTest, setActiveSelection } from "@/lib/active-selection-store";
import { createTestQueryClient } from "@/test-utils/query-client";

type ListenHandler = (event: { payload: unknown }) => void;

const registeredHandlers = new Map<string, ListenHandler[]>();
let queryClient: QueryClient;

function buildEnv(id: number, url: string): EnvironmentRecord {
  return {
    id,
    url,
    label: `Env ${id}`,
    environment: "production",
    source: "manual",
    lastScannedAt: null,
    latestScore: 84,
  };
}

function buildProject(id: number): ProjectRecord {
  return {
    id,
    name: `Project ${id}`,
    path: `/tmp/project-${id}`,
    framework: null,
    createdAt: `2026-04-14T12:0${id}:00Z`,
    environments: [buildEnv(id * 10 + 1, `https://project-${id}.example.com`)],
  };
}

const navigateTo = vi.fn();
const openTrayScanConfig = vi.fn();
const showBackgroundedScan = vi.fn();
const loadHistory = vi.fn();
const selectProject = vi.fn();
const refreshProjectsStub = vi.fn(async () => ({
  projects: [] as ProjectRecord[],
  newProject: null,
}));
const toast = { success: vi.fn(), warning: vi.fn(), info: vi.fn() };
const desktopPrefs = {
  backgroundMonitoring: false,
  desktopNotifications: false,
  fileWatchSuggestions: false,
  refreshOnFocus: true,
};
const normalizeUrl = (value?: string | null) => value ?? null;
const loadPrimaryWorkflowCue = vi.fn(async () => null);

function buildHookOptions(overrides: { projects: ProjectRecord[] }) {
  return {
    ...overrides,
    projectsLoading: false,
    refreshProjects: refreshProjectsStub,
    selectProject,
    navigateTo,
    openTrayScanConfig,
    showBackgroundedScan,
    loadHistory,
    toast,
    desktopPrefs,
    normalizeUrl,
    loadPrimaryWorkflowCue,
  };
}

function mockDesktopWatch() {
  const project = buildProject(1);
  const signal = {
    projectId: 1,
    url: project.environments[0]?.url,
    kind: "dependency-manifest",
    relativePath: "package.json",
    absolutePath: "/tmp/project-1/package.json",
    modifiedMs: 2,
    page: "updates",
    title: "Dependencies changed",
    detail: "package.json changed",
  };
  window.localStorage.setItem(
    "sitecmd_desktop_watch_snapshot_v1",
    JSON.stringify({ "1:package.json": 1 }),
  );
  const inspections: Array<(signals: (typeof signal)[]) => void> = [];
  invokeMock.mockImplementation((command: string) => {
    if (command === "inspect_desktop_watch_files") {
      return new Promise((resolve) => inspections.push(resolve));
    }
    return Promise.resolve(null);
  });
  const options = {
    ...buildHookOptions({ projects: [project] }),
    desktopPrefs: { ...desktopPrefs, backgroundMonitoring: true, fileWatchSuggestions: true },
  };
  return { options, signal, inspections };
}

const latest: {
  projects: ProjectRecord[];
  activeProject: ProjectRecord | null;
  activeEnv: EnvironmentRecord | null;
  projectsLoading: boolean;
} = {
  projects: [],
  activeProject: null,
  activeEnv: null,
  projectsLoading: true,
};

function OrchestrationHarness() {
  const { activeEnv, activeProject, projects, projectsLoading, refreshProjects, selectProject } =
    useProject();
  useEffect(() => {
    latest.projects = projects;
    latest.activeProject = activeProject;
    latest.activeEnv = activeEnv;
    latest.projectsLoading = projectsLoading;
  });
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
  return null;
}

function getProjectsCallCount() {
  return invokeMock.mock.calls.filter((call) => call[0] === "get_projects").length;
}

describe("useAppShellOrchestration listener stability", () => {
  beforeEach(() => {
    queryClient = createTestQueryClient();
    registeredHandlers.clear();
    invokeMock.mockReset();
    safeListenMock.mockReset();
    safeListenMock.mockImplementation(async (event: string, handler: ListenHandler) => {
      const bucket = registeredHandlers.get(event) ?? [];
      bucket.push(handler);
      registeredHandlers.set(event, bucket);
      return () => {};
    });
    navigateTo.mockClear();
    openTrayScanConfig.mockClear();
    showBackgroundedScan.mockClear();
    loadHistory.mockClear();
    selectProject.mockClear();
    refreshProjectsStub.mockClear();
    toast.success.mockClear();
    toast.warning.mockClear();
    toast.info.mockClear();
    loadPrimaryWorkflowCue.mockClear();
    window.localStorage.clear();
    resetActiveSelectionForTest();
    latest.projects = [];
    latest.activeProject = null;
    latest.activeEnv = null;
    latest.projectsLoading = true;
  });

  it("coalesces overlapping watch triggers and notifies once per file change", async () => {
    const { options, signal, inspections } = mockDesktopWatch();
    const { unmount } = renderHook(() => useAppShellOrchestration(options));
    try {
      expect(inspections).toHaveLength(1);
      act(() => {
        window.dispatchEvent(new Event("focus"));
        document.dispatchEvent(new Event("visibilitychange"));
      });
      expect(inspections).toHaveLength(1);
      await act(async () => inspections[0]?.([signal]));
      expect(toast.info).toHaveBeenCalledTimes(1);

      act(() => window.dispatchEvent(new Event("focus")));
      expect(inspections).toHaveLength(2);
      await act(async () => inspections[1]?.([signal]));
      expect(toast.info).toHaveBeenCalledTimes(1);
    } finally {
      unmount();
      window.localStorage.clear();
    }
  });

  it("immediately resumes watching after a StrictMode effect restart", async () => {
    const { options, signal, inspections } = mockDesktopWatch();
    renderHook(() => useAppShellOrchestration(options), { wrapper: StrictMode });
    expect(inspections).toHaveLength(1);

    await act(async () => inspections[0]?.([signal]));
    expect(toast.info).not.toHaveBeenCalled();
    expect(inspections).toHaveLength(2);

    await act(async () => inspections[1]?.([signal]));
    expect(toast.info).toHaveBeenCalledTimes(1);
  });

  it("cancels a queued watch restart when monitoring is disabled", async () => {
    const { options, signal, inspections } = mockDesktopWatch();
    const { rerender } = renderHook((props) => useAppShellOrchestration(props), {
      initialProps: options,
      wrapper: StrictMode,
    });
    expect(inspections).toHaveLength(1);

    rerender({ ...options, desktopPrefs });
    await act(async () => inspections[0]?.([signal]));
    expect(inspections).toHaveLength(1);
    expect(toast.info).not.toHaveBeenCalled();
  });

  it("startup runs one get_projects and registers each shell listener once", async () => {
    const backendProjects = [buildProject(1), buildProject(2)];
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_projects") return JSON.parse(JSON.stringify(backendProjects));
      throw new Error(`unmocked command: ${command}`);
    });

    render(
      <QueryClientProvider client={queryClient}>
        <ProjectProvider>
          <OrchestrationHarness />
        </ProjectProvider>
      </QueryClientProvider>,
    );

    await waitFor(() => {
      expect(latest.projectsLoading).toBe(false);
      expect(latest.activeProject?.id).toBe(1);
    });
    await waitFor(() => {
      expect(safeListenMock).toHaveBeenCalledTimes(5);
    });
    // Let any stray follow-up sync land before asserting the fetch count.
    await act(async () => {
      await Promise.resolve();
    });

    expect(getProjectsCallCount()).toBe(1);
    for (const handlers of registeredHandlers.values()) {
      expect(handlers).toHaveLength(1);
    }
  });

  it("window focus with unchanged data re-registers nothing and keeps identities", async () => {
    const backendProjects = [buildProject(1), buildProject(2)];
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_projects") return JSON.parse(JSON.stringify(backendProjects));
      throw new Error(`unmocked command: ${command}`);
    });

    render(
      <QueryClientProvider client={queryClient}>
        <ProjectProvider>
          <OrchestrationHarness />
        </ProjectProvider>
      </QueryClientProvider>,
    );

    await waitFor(() => {
      expect(latest.projectsLoading).toBe(false);
      expect(latest.activeProject?.id).toBe(1);
    });
    await waitFor(() => {
      expect(safeListenMock).toHaveBeenCalledTimes(5);
    });

    const registrationsBefore = safeListenMock.mock.calls.length;
    const projectsBefore = latest.projects;
    const activeProjectBefore = latest.activeProject;
    const activeEnvBefore = latest.activeEnv;

    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => {
      expect(getProjectsCallCount()).toBe(2);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(safeListenMock.mock.calls.length).toBe(registrationsBefore);
    expect(latest.projects).toBe(projectsBefore);
    expect(latest.activeProject).toBe(activeProjectBefore);
    expect(latest.activeEnv).toBe(activeEnvBefore);
    expect(navigateTo).not.toHaveBeenCalled();
    expect(toast.success).not.toHaveBeenCalled();
  });

  it("never re-registers on selection change and reads the current selection from the store at event time", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      throw new Error(`unmocked command: ${command}`);
    });

    const project = buildProject(1);
    const firstEnv = project.environments[0];
    setActiveSelection(project.id, firstEnv.url);

    const { rerender } = renderHook((props) => useAppShellOrchestration(props), {
      initialProps: buildHookOptions({ projects: [project] }),
    });

    await waitFor(() => {
      expect(safeListenMock).toHaveBeenCalledTimes(5);
    });

    // A fresh props object must not tear listeners down.
    rerender(buildHookOptions({ projects: [project] }));
    expect(safeListenMock).toHaveBeenCalledTimes(5);

    const secondEnv = buildEnv(99, "https://switched.example.com");
    setActiveSelection(project.id, secondEnv.url);
    rerender(buildHookOptions({ projects: [project] }));
    expect(safeListenMock).toHaveBeenCalledTimes(5);

    const scheduledHandlers = registeredHandlers.get("scheduled-scan-complete") ?? [];
    expect(scheduledHandlers).toHaveLength(1);
    await act(async () => {
      scheduledHandlers[0]({
        payload: {
          projectId: project.id,
          url: secondEnv.url,
          score: 88,
          issues: 2,
          scanType: "health",
          status: "complete",
        },
      });
    });

    await waitFor(() => {
      expect(loadHistory).toHaveBeenCalledWith(secondEnv.url, project.id);
    });
  });

  it("warns when a scheduled scan completes with partial coverage", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      throw new Error(`unmocked command: ${command}`);
    });

    const project = buildProject(1);
    renderHook(() => useAppShellOrchestration(buildHookOptions({ projects: [project] })));

    await waitFor(() => {
      expect(registeredHandlers.get("scheduled-scan-complete")).toHaveLength(1);
    });

    const handler = registeredHandlers.get("scheduled-scan-complete")?.[0];
    expect(handler).toBeDefined();
    await act(async () => {
      handler?.({
        payload: {
          projectId: project.id,
          url: project.environments[0].url,
          score: 61,
          issues: 2,
          scanType: "health",
          status: "partial",
          completedPages: 2,
          totalPages: 2,
          incompleteDetail: "Browser analysis failed: browser unavailable",
        },
      });
    });

    expect(toast.warning).toHaveBeenCalledWith(
      "Scheduled Web Scan Partially Complete - 61/100",
      "Browser analysis failed: browser unavailable. 2 issues found on project-1.example.com. Some issues to address.",
    );
    expect(toast.success).not.toHaveBeenCalled();
  });
});
