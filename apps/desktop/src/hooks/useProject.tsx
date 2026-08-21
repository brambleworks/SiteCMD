import {
  createContext,
  useContext,
  useEffect,
  useCallback,
  useMemo,
  useRef,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getProjects, updateProjectPath } from "@/lib/commands";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import {
  finishPerformanceTimer,
  startPerformanceTimer,
  type PerformanceTimer,
} from "@/lib/performance-metrics";
import {
  clearStoredProjectSelection,
  normalizeStoredProjectSelectionUrl,
  readStoredProjectSelection,
} from "@/lib/project-selection-state";
import {
  getActiveSelection,
  setActiveSelection,
  subscribeActiveSelection,
} from "@/lib/active-selection-store";
import { queryKeys } from "@/lib/query/query-keys";
import { clearProjectSignalSnapshotCache } from "@/lib/project-summary-signals";

export interface EnvironmentRecord {
  id: number;
  url: string;
  label: string;
  environment: string;
  source: string | null;
  lastScannedAt: string | null;
  latestScore: number | null;
}

export interface ProjectRecord {
  id: number;
  name: string;
  path: string;
  framework: string | null;
  createdAt: string;
  environments: EnvironmentRecord[];
}

interface ProjectContextValue {
  projects: ProjectRecord[];
  projectsLoading: boolean;
  projectsLoadError: string | null;
  activeProject: ProjectRecord | null;
  activeEnv: EnvironmentRecord | null;
  projectFolder: string | null;

  selectProject: (project: ProjectRecord) => void;
  selectEnv: (env: EnvironmentRecord | null) => void;
  refreshProjects: (options?: { selectNewestImportedProject?: boolean }) => Promise<{
    projects: ProjectRecord[];
    newProject: ProjectRecord | null;
  }>;
  retryProjectsLoad: () => Promise<void>;
  handleAddFolder: () => Promise<void>;
}

const ProjectContext = createContext<ProjectContextValue | null>(null);

const PROJECT_BOOTSTRAP_TIMEOUT_MS = import.meta.env.MODE === "test" ? 20 : 1200;
const PROJECT_BOOTSTRAP_MAX_ATTEMPTS = 3;
const PROJECT_BOOTSTRAP_RETRY_DELAY_MS = import.meta.env.MODE === "test" ? 5 : 150;
const PROJECTS_QUERY_KEY = queryKeys.projects.list();

class ProjectBootstrapTimeoutError extends Error {
  constructor() {
    super("Timed out while loading projects");
    this.name = "ProjectBootstrapTimeoutError";
  }
}

function sleep(ms: number) {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  let timeoutId: number | null = null;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        timeoutId = window.setTimeout(() => {
          reject(new ProjectBootstrapTimeoutError());
        }, timeoutMs);
      }),
    ]);
  } finally {
    if (timeoutId != null) {
      window.clearTimeout(timeoutId);
    }
  }
}

async function loadProjectsWithRetry() {
  let lastError: unknown = null;

  for (let attempt = 1; attempt <= PROJECT_BOOTSTRAP_MAX_ATTEMPTS; attempt += 1) {
    try {
      return await withTimeout(getProjects(), PROJECT_BOOTSTRAP_TIMEOUT_MS);
    } catch (error) {
      lastError = error;
      if (attempt >= PROJECT_BOOTSTRAP_MAX_ATTEMPTS) break;
      await sleep(PROJECT_BOOTSTRAP_RETRY_DELAY_MS * attempt);
    }
  }

  throw lastError ?? new Error("We could not load your projects right now.");
}

/** Safe because both values use the same backend serialization order. */
function projectDataEqual(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function projectCacheInputs(projects: ProjectRecord[]) {
  return projects.map((project) => ({
    id: project.id,
    name: project.name,
    path: project.path,
    framework: project.framework,
    environments: project.environments.map((environment) => ({
      id: environment.id,
      url: environment.url,
      label: environment.label,
      environment: environment.environment,
      source: environment.source,
    })),
  }));
}

function invalidateChangedProjectCaches(
  queryClient: ReturnType<typeof useQueryClient>,
  previous: ProjectRecord[],
  next: ProjectRecord[],
) {
  if (projectDataEqual(projectCacheInputs(previous), projectCacheInputs(next))) return;

  for (const queryKey of [
    queryKeys.projectSummary.all,
    queryKeys.sites.all,
    queryKeys.reports.all,
    queryKeys.deploys.all,
    queryKeys.updates.all,
  ]) {
    void queryClient.invalidateQueries({ queryKey });
  }

  const previousById = new Map(previous.map((project) => [project.id, project]));
  const nextById = new Map(next.map((project) => [project.id, project]));
  const projectIds = new Set([...previousById.keys(), ...nextById.keys()]);
  for (const projectId of projectIds) {
    const before = previousById.get(projectId);
    const after = nextById.get(projectId);
    if (
      before &&
      after &&
      projectDataEqual(projectCacheInputs([before]), projectCacheInputs([after]))
    ) {
      continue;
    }
    const urls = new Set([
      ...(before?.environments.map((environment) => environment.url) ?? []),
      ...(after?.environments.map((environment) => environment.url) ?? []),
    ]);
    for (const url of urls) {
      clearProjectSignalSnapshotCache(queryClient, projectId, url);
    }
  }
}

export function findNewestImportedProject(
  previous: ProjectRecord[],
  next: ProjectRecord[],
): ProjectRecord | null {
  const previousIds = new Set(previous.map((project) => project.id));
  const newcomers = next.filter((project) => !previousIds.has(project.id));
  if (newcomers.length === 0) return null;
  return (
    newcomers.sort((a, b) => {
      const aTime = Date.parse(a.createdAt);
      const bTime = Date.parse(b.createdAt);
      if (Number.isFinite(aTime) && Number.isFinite(bTime) && aTime !== bTime) {
        return bTime - aTime;
      }
      return b.id - a.id;
    })[0] ?? null
  );
}

export function ProjectProvider({
  children,
  onEnvChange,
}: {
  children: ReactNode;
  onEnvChange?: (env: EnvironmentRecord | null) => void;
}) {
  const queryClient = useQueryClient();
  const projectsQuery = useQuery<ProjectRecord[]>({
    queryKey: PROJECTS_QUERY_KEY,
    queryFn: loadProjectsWithRetry,
    // refreshProjects owns identity preservation.
    structuralSharing: false,
  });
  const refetchProjects = projectsQuery.refetch;
  const projects = useMemo(() => projectsQuery.data ?? [], [projectsQuery.data]);
  const projectsLoading = projectsQuery.isPending;
  const projectsLoadError =
    projectsQuery.isError && projectsQuery.data == null
      ? "We could not load your projects right now."
      : null;
  const initialProjectLoadTimerRef = useRef<PerformanceTimer | null>(
    startPerformanceTimer("app.first_project_load_ms"),
  );
  const bootstrapLoadCompletedRef = useRef(false);
  // Updated after commit so discarded renders cannot leak into async refreshes.
  const projectsRef = useRef<ProjectRecord[]>(projects);
  useEffect(() => {
    projectsRef.current = projects;
  }, [projects]);

  // Rich records are derived from the selection store and current project list.
  const selection = useSyncExternalStore(
    subscribeActiveSelection,
    getActiveSelection,
    getActiveSelection,
  );
  const activeProject = useMemo(
    () => projects.find((project) => project.id === selection.projectId) ?? null,
    [projects, selection.projectId],
  );
  const activeEnv = useMemo(() => {
    if (!activeProject) return null;
    if (!selection.envUrl) return null;
    return (
      activeProject.environments.find(
        (env) => normalizeStoredProjectSelectionUrl(env.url) === selection.envUrl,
      ) ?? null
    );
  }, [activeProject, selection.envUrl]);

  // Notify only when the persisted selection actually changes.
  const applySelection = useCallback(
    (project: ProjectRecord | null, env: EnvironmentRecord | null) => {
      const changed = setActiveSelection(project?.id ?? null, env?.url ?? null);
      if (changed) {
        onEnvChange?.(env);
      }
    },
    [onEnvChange],
  );

  const selectProjectInternal = useCallback(
    (project: ProjectRecord, preferredEnvUrl?: string | null) => {
      const preferred = preferredEnvUrl
        ? project.environments.find(
            (env) =>
              normalizeStoredProjectSelectionUrl(env.url) ===
              normalizeStoredProjectSelectionUrl(preferredEnvUrl),
          )
        : null;
      const prod = project.environments.find((e) => e.environment === "production");
      const env = preferred || prod || project.environments[0] || null;
      applySelection(project, env);
    },
    [applySelection],
  );

  // Restore a valid selection after the project list loads.
  useEffect(() => {
    if (projectsQuery.isPending || bootstrapLoadCompletedRef.current) return;
    if (projectsQuery.isError && projectsQuery.data == null) {
      initialProjectLoadTimerRef.current = null;
      return;
    }

    const loadedProjects = projectsQuery.data ?? [];
    if (getActiveSelection().projectId == null && loadedProjects.length > 0) {
      const storedSelection = readStoredProjectSelection();
      const storedProject = storedSelection
        ? (loadedProjects.find((project) => project.id === storedSelection.projectId) ?? null)
        : null;
      selectProjectInternal(storedProject ?? loadedProjects[0], storedSelection?.envUrl ?? null);
    } else if (loadedProjects.length === 0) {
      setActiveSelection(null, null);
      clearStoredProjectSelection();
    }
    finishPerformanceTimer(initialProjectLoadTimerRef.current, {
      status: loadedProjects.length === 0 ? "empty" : "ready",
      projectCount: loadedProjects.length,
    });
    initialProjectLoadTimerRef.current = null;
    bootstrapLoadCompletedRef.current = true;
  }, [projectsQuery.data, projectsQuery.isError, projectsQuery.isPending, selectProjectInternal]);

  const selectProject = useCallback(
    (project: ProjectRecord) => {
      selectProjectInternal(project);
    },
    [selectProjectInternal],
  );

  const selectEnv = useCallback(
    (env: EnvironmentRecord | null) => {
      const changed = setActiveSelection(getActiveSelection().projectId, env?.url ?? null);
      if (changed) {
        onEnvChange?.(env);
      }
    },
    [onEnvChange],
  );

  const refreshProjects = useCallback(
    async (options?: { selectNewestImportedProject?: boolean }) => {
      try {
        const fetched = await getProjects();
        const previous = projectsRef.current;
        const newProject = findNewestImportedProject(previous, fetched);
        // Preserve identity when the backend data is unchanged.
        const updated = projectDataEqual(fetched, previous) ? previous : fetched;
        queryClient.setQueryData<ProjectRecord[]>(PROJECTS_QUERY_KEY, updated);
        invalidateChangedProjectCaches(queryClient, previous, fetched);
        if (options?.selectNewestImportedProject && newProject) {
          selectProjectInternal(newProject);
          return { projects: updated, newProject };
        }
        // Respect selection changes that occurred during the fetch.
        const current = getActiveSelection();
        if (current.projectId != null) {
          const refreshed = updated.find((p) => p.id === current.projectId);
          if (refreshed) {
            const refreshedEnv = current.envUrl
              ? refreshed.environments.find(
                  (env) => normalizeStoredProjectSelectionUrl(env.url) === current.envUrl,
                )
              : null;
            const nextEnv =
              refreshedEnv ??
              refreshed.environments.find((env) => env.environment === "production") ??
              refreshed.environments[0] ??
              null;
            applySelection(refreshed, nextEnv);
          } else if (updated.length > 0) {
            selectProjectInternal(updated[0]);
          } else {
            applySelection(null, null);
          }
        } else if (updated.length > 0) {
          const storedSelection = readStoredProjectSelection();
          const storedProject = storedSelection
            ? (updated.find((project) => project.id === storedSelection.projectId) ?? null)
            : null;
          selectProjectInternal(storedProject ?? updated[0], storedSelection?.envUrl ?? null);
        }
        return { projects: updated, newProject };
      } catch {
        // Keep the current list.
        return { projects: projectsRef.current, newProject: null };
      }
    },
    [applySelection, queryClient, selectProjectInternal],
  );

  const retryProjectsLoad = useCallback(async () => {
    bootstrapLoadCompletedRef.current = false;
    await refetchProjects();
  }, [refetchProjects]);

  const handleAddFolder = useCallback(async () => {
    if (!activeProject) return;
    try {
      const selected = await openFolderDialog({ directory: true, title: "Select project folder" });
      if (!selected) return;
      const path = typeof selected === "string" ? selected : String(selected);
      await updateProjectPath({ projectId: activeProject.id, path });
      await refreshProjects();
    } catch {
      // The user can retry from the same control.
    }
  }, [activeProject, refreshProjects]);

  const projectFolder =
    activeProject?.path && !activeProject.path.startsWith("__url__") ? activeProject.path : null;

  const value = useMemo(
    () => ({
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
    }),
    [
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
    ],
  );

  return <ProjectContext.Provider value={value}>{children}</ProjectContext.Provider>;
}

export function useProject(): ProjectContextValue {
  const ctx = useContext(ProjectContext);
  if (!ctx) throw new Error("useProject must be used within <ProjectProvider>");
  return ctx;
}
