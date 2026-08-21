import { type ReactNode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import {
  ProjectProvider,
  findNewestImportedProject,
  useProject,
  type ProjectRecord,
} from "./useProject";
import { getActiveSelection, resetActiveSelectionForTest } from "@/lib/active-selection-store";
import { createTestQueryClient } from "@/test-utils/query-client";
import { queryKeys } from "@/lib/query/query-keys";

let queryClient: QueryClient;

function buildProject(
  overrides: Partial<ProjectRecord> & Pick<ProjectRecord, "id" | "name">,
): ProjectRecord {
  return {
    id: overrides.id,
    name: overrides.name,
    path: overrides.path ?? `/tmp/${overrides.name}`,
    framework: overrides.framework ?? null,
    createdAt: overrides.createdAt ?? "2026-04-13T12:00:00Z",
    environments: overrides.environments ?? [],
  };
}

function wrapper({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <ProjectProvider>{children}</ProjectProvider>
    </QueryClientProvider>
  );
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

describe("useProject", () => {
  beforeEach(() => {
    queryClient = createTestQueryClient();
    invokeMock.mockReset();
    window.localStorage.clear();
    window.sessionStorage.clear();
    resetActiveSelectionForTest();
  });

  it("finds the newest imported project when the list grows", () => {
    const previous = [buildProject({ id: 1, name: "Existing", createdAt: "2026-04-13T12:00:00Z" })];
    const next = [
      buildProject({ id: 2, name: "Newer", createdAt: "2026-04-13T12:05:00Z" }),
      previous[0],
    ];

    expect(findNewestImportedProject(previous, next)?.id).toBe(2);
  });

  it("can promote a newly imported project during a refresh", async () => {
    const existing = buildProject({
      id: 1,
      name: "Existing",
      environments: [
        {
          id: 11,
          url: "https://existing.test",
          label: "Existing",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });
    const imported = buildProject({
      id: 2,
      name: "Imported",
      createdAt: "2026-04-13T12:05:00Z",
      environments: [
        {
          id: 21,
          url: "https://imported.test",
          label: "Imported",
          environment: "production",
          source: "sitecmd-cli",
          lastScannedAt: null,
          latestScore: 88,
        },
      ],
    });

    invokeMock.mockResolvedValueOnce([existing]).mockResolvedValueOnce([imported, existing]);

    const { result } = renderHook(() => useProject(), { wrapper });

    await waitFor(() => {
      expect(result.current.projects.map((project) => project.id)).toEqual([1]);
      expect(result.current.activeProject?.id).toBe(1);
    });

    await act(async () => {
      const refresh = await result.current.refreshProjects({ selectNewestImportedProject: true });
      expect(refresh.newProject?.id).toBe(2);
    });

    expect(result.current.projects.map((project) => project.id)).toEqual([2, 1]);
    expect(result.current.activeProject?.id).toBe(2);
    expect(result.current.activeEnv?.url).toBe("https://imported.test");
  });

  it("keeps projects/activeProject/activeEnv identities when a refresh returns unchanged data", async () => {
    const project = buildProject({
      id: 1,
      name: "Stable",
      environments: [
        {
          id: 11,
          url: "https://stable.test",
          label: "Stable",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: 90,
        },
      ],
    });

    // Return a fresh deep clone on every call so retained identities can only
    // come from the provider's deep-equal bail, not from the mock.
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_projects") return JSON.parse(JSON.stringify([project]));
      return null;
    });

    const { result } = renderHook(() => useProject(), { wrapper });

    await waitFor(() => {
      expect(result.current.activeProject?.id).toBe(1);
    });

    const projectsBefore = result.current.projects;
    const activeProjectBefore = result.current.activeProject;
    const activeEnvBefore = result.current.activeEnv;

    await act(async () => {
      await result.current.refreshProjects();
    });
    await act(async () => {
      await result.current.refreshProjects({ selectNewestImportedProject: true });
    });

    expect(result.current.projects).toBe(projectsBefore);
    expect(result.current.activeProject).toBe(activeProjectBefore);
    expect(result.current.activeEnv).toBe(activeEnvBefore);
  });

  it("adopts refreshed data when the backend list actually changed", async () => {
    const project = buildProject({
      id: 1,
      name: "Rescored",
      environments: [
        {
          id: 11,
          url: "https://rescored.test",
          label: "Rescored",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: 70,
        },
      ],
    });
    const rescored: ProjectRecord = JSON.parse(JSON.stringify(project));
    rescored.environments[0].latestScore = 95;

    invokeMock.mockResolvedValueOnce([project]).mockResolvedValueOnce([rescored]);

    const { result } = renderHook(() => useProject(), { wrapper });

    await waitFor(() => {
      expect(result.current.activeEnv?.latestScore).toBe(70);
    });

    const projectsBefore = result.current.projects;
    const sitesKey = queryKeys.sites.overview();
    queryClient.setQueryData(sitesKey, { marker: "keep-fresh" });

    await act(async () => {
      await result.current.refreshProjects();
    });

    await waitFor(() => {
      expect(result.current.activeEnv?.latestScore).toBe(95);
    });
    expect(result.current.projects).not.toBe(projectsBefore);
    expect(result.current.activeProject?.environments[0]?.latestScore).toBe(95);
    // A scan-updated score should not invalidate project-definition caches;
    // scan events own the affected data families directly.
    expect(queryClient.getQueryState(sitesKey)?.isInvalidated).toBe(false);
  });

  it("invalidates derived page caches when project metadata changes", async () => {
    const project = buildProject({
      id: 1,
      name: "Before rename",
      environments: [
        {
          id: 11,
          url: "https://rename.test",
          label: "Production",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: 80,
        },
      ],
    });
    const renamed = { ...project, name: "After rename" };
    invokeMock.mockResolvedValueOnce([project]).mockResolvedValueOnce([renamed]);

    const { result } = renderHook(() => useProject(), { wrapper });
    await waitFor(() => {
      expect(result.current.activeProject?.name).toBe("Before rename");
    });

    const derivedKeys = [
      queryKeys.sites.overview(),
      queryKeys.reports.history(1),
      queryKeys.deploys.overview(1, "https://rename.test", project.path),
      queryKeys.updates.report(1, project.path, "https://rename.test"),
    ] as const;
    for (const key of derivedKeys) queryClient.setQueryData(key, { marker: "old" });
    const summaryKey = queryKeys.projectSummary.snapshot(1, "https://rename.test");
    queryClient.setQueryData(summaryKey, {
      snapshot: { marker: "old" },
      cachedAt: Date.now(),
    });

    await act(async () => {
      await result.current.refreshProjects();
    });

    await waitFor(() => {
      expect(result.current.activeProject?.name).toBe("After rename");
    });
    for (const key of derivedKeys) {
      expect(queryClient.getQueryState(key)?.isInvalidated).toBe(true);
    }
    expect(queryClient.getQueryState(summaryKey)).toBeUndefined();
  });

  it("restores the previously selected project and environment on mount", async () => {
    window.localStorage.setItem(
      "sitecmd_project_selection_v1",
      JSON.stringify({
        projectId: 2,
        envUrl: "https://staging.test",
      }),
    );

    const first = buildProject({
      id: 1,
      name: "First",
      environments: [
        {
          id: 11,
          url: "https://first.test",
          label: "First",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });
    const second = buildProject({
      id: 2,
      name: "Second",
      environments: [
        {
          id: 21,
          url: "https://second.test",
          label: "Second",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
        {
          id: 22,
          url: "https://staging.test",
          label: "Second staging",
          environment: "staging",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });

    invokeMock.mockResolvedValueOnce([first, second]);

    const { result } = renderHook(() => useProject(), { wrapper });

    await waitFor(() => {
      expect(result.current.activeProject?.id).toBe(2);
      expect(result.current.activeEnv?.url).toBe("https://staging.test");
    });
  });

  it("keeps the newer manual project selection when an older refresh resolves late", async () => {
    const alpha = buildProject({
      id: 1,
      name: "Alpha",
      environments: [
        {
          id: 11,
          url: "https://alpha.test",
          label: "Alpha",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });
    const beta = buildProject({
      id: 2,
      name: "Beta",
      environments: [
        {
          id: 21,
          url: "https://beta.test",
          label: "Beta",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });
    const refreshGate = deferred<ProjectRecord[]>();

    invokeMock
      .mockResolvedValueOnce([alpha, beta])
      .mockImplementationOnce(() => refreshGate.promise);

    const { result } = renderHook(() => useProject(), { wrapper });

    await waitFor(() => {
      expect(result.current.activeProject?.id).toBe(1);
    });

    let refreshPromise: Promise<{
      projects: ProjectRecord[];
      newProject: ProjectRecord | null;
    }> | null = null;
    await act(async () => {
      refreshPromise = result.current.refreshProjects();
      result.current.selectProject(beta);
    });

    expect(result.current.activeProject?.id).toBe(2);

    await act(async () => {
      refreshGate.resolve([alpha, beta]);
      await refreshPromise;
    });

    expect(result.current.activeProject?.id).toBe(2);
    expect(result.current.activeEnv?.url).toBe("https://beta.test");
  });

  it("exposes the active selection through the store synchronously after selection changes", async () => {
    const project = buildProject({
      id: 3,
      name: "Gamma",
      environments: [
        {
          id: 31,
          url: "https://prod.test",
          label: "Prod",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
        {
          id: 32,
          url: "https://stg.test",
          label: "Stg",
          environment: "staging",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });
    invokeMock.mockResolvedValueOnce([project]);

    const { result } = renderHook(() => useProject(), { wrapper });
    await waitFor(() => {
      expect(result.current.activeProject?.id).toBe(3);
    });

    // Bootstrap selected the production env; the store is the owner and reflects
    // it synchronously - the value a long-lived listener would read at fire time.
    expect(getActiveSelection()).toEqual({ projectId: 3, envUrl: "https://prod.test" });

    act(() => {
      result.current.selectEnv(project.environments[1]);
    });
    // No await: selectEnv writes the store synchronously.
    expect(getActiveSelection()).toEqual({ projectId: 3, envUrl: "https://stg.test" });
    expect(result.current.activeEnv?.url).toBe("https://stg.test");
  });

  it("fires onEnvChange once per real selection change and not on a no-op refresh", async () => {
    const onEnvChange = vi.fn();
    function envWrapper({ children }: { children: ReactNode }) {
      return (
        <QueryClientProvider client={queryClient}>
          <ProjectProvider onEnvChange={onEnvChange}>{children}</ProjectProvider>
        </QueryClientProvider>
      );
    }
    const project = buildProject({
      id: 4,
      name: "Delta",
      environments: [
        {
          id: 41,
          url: "https://prod.test",
          label: "Prod",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
        {
          id: 42,
          url: "https://stg.test",
          label: "Stg",
          environment: "staging",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });
    // Bootstrap and the later refresh both return identical data.
    invokeMock.mockResolvedValue([project]);

    const { result } = renderHook(() => useProject(), { wrapper: envWrapper });
    await waitFor(() => {
      expect(result.current.activeProject?.id).toBe(4);
    });

    // Bootstrap selecting the production env is the one real change so far.
    expect(onEnvChange).toHaveBeenCalledTimes(1);

    // A refresh that returns identical data must not fire onEnvChange again.
    await act(async () => {
      await result.current.refreshProjects();
    });
    expect(onEnvChange).toHaveBeenCalledTimes(1);

    // Switching to a different env is a real change.
    act(() => {
      result.current.selectEnv(project.environments[1]);
    });
    expect(onEnvChange).toHaveBeenCalledTimes(2);
  });

  it("falls back to the first available project when the stored selection is stale", async () => {
    window.localStorage.setItem(
      "sitecmd_project_selection_v1",
      JSON.stringify({
        projectId: 99,
        envUrl: "https://missing.test",
      }),
    );

    const first = buildProject({
      id: 1,
      name: "First",
      environments: [
        {
          id: 11,
          url: "https://first.test",
          label: "First",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });

    invokeMock.mockResolvedValueOnce([first]);

    const { result } = renderHook(() => useProject(), { wrapper });

    await waitFor(() => {
      expect(result.current.activeProject?.id).toBe(1);
      expect(result.current.activeEnv?.url).toBe("https://first.test");
    });

    expect(JSON.parse(window.localStorage.getItem("sitecmd_project_selection_v1") ?? "{}")).toEqual(
      {
        projectId: 1,
        envUrl: "https://first.test",
      },
    );
  });

  it("surfaces startup load failures and recovers on retry", async () => {
    const first = buildProject({
      id: 1,
      name: "Recovered",
      environments: [
        {
          id: 11,
          url: "https://recovered.test",
          label: "Recovered",
          environment: "production",
          source: "manual",
          lastScannedAt: null,
          latestScore: null,
        },
      ],
    });

    invokeMock
      .mockRejectedValueOnce(new Error("db offline"))
      .mockRejectedValueOnce(new Error("db offline"))
      .mockRejectedValueOnce(new Error("db offline"))
      .mockResolvedValueOnce([first]);

    const { result } = renderHook(() => useProject(), { wrapper });

    await waitFor(() => {
      expect(result.current.projectsLoading).toBe(false);
      expect(result.current.projectsLoadError).toBe("We could not load your projects right now.");
      expect(result.current.projects).toEqual([]);
    });

    await act(async () => {
      await result.current.retryProjectsLoad();
    });

    await waitFor(() => {
      expect(result.current.projectsLoadError).toBeNull();
      expect(result.current.activeProject?.id).toBe(1);
      expect(result.current.activeEnv?.url).toBe("https://recovered.test");
    });
  });
});
