import type { ActionableDesktopNotificationEvent } from "@/lib/actionable-notifications";
import type { AppTarget } from "@/lib/app-targets";
import { normalizeTargetUrl, withNormalizedTarget } from "@/lib/app-targets";
import { parseDeepLinkUrl } from "@/lib/deep-links";

export function getLatestDeepLinkTarget(urls: string[] | null | undefined): AppTarget | null {
  if (!urls || urls.length === 0) return null;
  return (
    [...urls]
      .reverse()
      .map((url) => parseDeepLinkUrl(url))
      .find((candidate): candidate is AppTarget => candidate != null) ?? null
  );
}

export function getLatestDeepLinkEnvelope(urls: string[] | null | undefined): {
  target: AppTarget;
  dedupeKey: string;
} | null {
  const target = getLatestDeepLinkTarget(urls);
  if (!target) return null;
  return {
    target,
    dedupeKey: JSON.stringify(withNormalizedTarget(target)),
  };
}

export function shouldIgnoreRepeatedDeepLink({
  nextKey,
  lastKey,
  elapsedMs,
  dedupeWindowMs = 1500,
}: {
  nextKey: string;
  lastKey: string | null;
  elapsedMs: number;
  dedupeWindowMs?: number;
}): boolean {
  if (!lastKey) return false;
  if (nextKey !== lastKey) return false;
  return elapsedMs >= 0 && elapsedMs <= dedupeWindowMs;
}

export function getProjectBootstrapState({
  projectCount,
  projectsLoading,
  projectsLoadError,
  showAddProject,
}: {
  projectCount: number;
  projectsLoading: boolean;
  projectsLoadError: string | null;
  showAddProject: boolean;
}): "loading" | "error" | "welcome" | null {
  if (projectCount > 0 || showAddProject) return null;
  if (projectsLoading) return "loading";
  if (projectsLoadError) return "error";
  return "welcome";
}

export function shouldDeferAppTargetUntilProjectsReady({
  projectCount,
  projectsLoading,
  target,
}: {
  projectCount: number;
  projectsLoading: boolean;
  target: AppTarget;
}): boolean {
  if (!projectsLoading || projectCount > 0) return false;
  return target.projectId != null || normalizeTargetUrl(target.url) != null;
}

/** Routes a post-delete selection change to the promoted project's dashboard. */
export function createProjectDeletedHandler({
  refreshProjects,
  navigateTo,
}: {
  refreshProjects: () => Promise<unknown>;
  navigateTo: (page: "dashboard") => void;
}): () => Promise<void> {
  return async () => {
    await refreshProjects();
    navigateTo("dashboard");
  };
}

export async function handleDesktopNotificationAction(
  payload: ActionableDesktopNotificationEvent,
  handlers: {
    openFilePath?: (path: string) => Promise<unknown> | void;
    openTarget?: (target: AppTarget) => void;
  },
): Promise<void> {
  if (payload.filePath) {
    await handlers.openFilePath?.(payload.filePath);
  }
  if (payload.target) {
    handlers.openTarget?.(payload.target);
  }
}
