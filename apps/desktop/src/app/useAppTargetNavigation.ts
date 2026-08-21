import { useCallback, useEffect, useRef } from "react";

import { toNavPage } from "@/components/layout/nav-page";
import { getDesktopPromptById, resolveDesktopPrompt } from "@/lib/desktop-prompts";
import { normalizeTargetUrl, withNormalizedTarget, type AppTarget } from "@/lib/app-targets";
import { consumeOnboardingSetupStepForTarget } from "@/lib/onboarding-setup";
import { shouldDeferAppTargetUntilProjectsReady } from "@/app/app-shell-helpers";
import type { NavigationDispatch } from "@/app/useNavigationState";
import type { EnvironmentRecord, ProjectRecord } from "@/hooks/useProject";

interface UseAppTargetNavigationParams {
  activeEnv: EnvironmentRecord | null;
  activeProject: ProjectRecord | null;
  projects: ProjectRecord[];
  projectsLoading: boolean;
  selectProject: (project: ProjectRecord) => void;
  selectEnv: (environment: EnvironmentRecord) => void;
  dispatch: NavigationDispatch;
  openScanConfig: () => void;
  updateScanBackgrounded: (next: boolean) => void;
}

export function useAppTargetNavigation({
  activeEnv,
  activeProject,
  projects,
  projectsLoading,
  selectProject,
  selectEnv,
  dispatch,
  openScanConfig,
  updateScanBackgrounded,
}: UseAppTargetNavigationParams) {
  const pendingAppTargetRef = useRef<AppTarget | null>(null);
  const activeProjectId = activeProject?.id ?? null;

  const navigateTo = useCallback(
    (target: string) => {
      const canonicalTarget =
        target === "today" ? "sites" : target === "issues" ? "issues" : target;
      const resolvedTarget =
        canonicalTarget === "settings:integrations"
          ? "integrations"
          : canonicalTarget.split(":")[0];

      if (canonicalTarget.startsWith("integrations:")) {
        const type = canonicalTarget.slice("integrations:".length) || null;
        dispatch({ type: "OPEN_INTEGRATIONS", focus: type });
      } else if (canonicalTarget === "settings:integrations") {
        dispatch({ type: "OPEN_INTEGRATIONS", focus: null });
      } else if (canonicalTarget.startsWith("settings:")) {
        dispatch({ type: "OPEN_SETTINGS", tab: canonicalTarget.split(":")[1] });
      } else if (canonicalTarget.startsWith("search-console:")) {
        const searchTarget = canonicalTarget.slice("search-console:".length) || null;
        dispatch({ type: "OPEN_SEARCH_CONSOLE", focus: searchTarget });
      } else if (canonicalTarget.startsWith("scans:")) {
        dispatch({ type: "NAVIGATE_ISSUES" });
      } else if (canonicalTarget === "reports") {
        dispatch({ type: "NAVIGATE_GENERIC", page: "reports" });
      } else if (canonicalTarget === "issues") {
        dispatch({ type: "NAVIGATE_GENERIC", page: "issues" });
        dispatch({ type: "RESET_ISSUES_TAB" });
      } else {
        dispatch({ type: "NAVIGATE_GENERIC", page: toNavPage(canonicalTarget) });
      }

      if (activeProjectId != null) {
        consumeOnboardingSetupStepForTarget(activeProjectId, resolvedTarget);
      }
    },
    [activeProjectId, dispatch],
  );

  const openProjectSettings = useCallback(
    (project: ProjectRecord) => {
      selectProject(project);
      navigateTo("settings:site-setup");
    },
    [navigateTo, selectProject],
  );

  const openAppTargetInCurrentContext = useCallback(
    (rawTarget: AppTarget) => {
      const target = withNormalizedTarget(rawTarget);
      const consumedPrompt = target.promptId ? getDesktopPromptById(target.promptId) : null;

      if (target.promptId) {
        resolveDesktopPrompt(target.promptId);
      }

      if (target.restoreScan) {
        updateScanBackgrounded(false);
        return;
      }

      if (target.page === "search-console") {
        dispatch({
          type: "OPEN_SEARCH_CONSOLE",
          focus: target.focus ?? null,
          itemId: target.itemId ?? null,
          lane: target.lane ?? null,
          prompt:
            consumedPrompt && consumedPrompt.page === "search-console"
              ? { page: "search-console", entry: consumedPrompt }
              : null,
        });
        return;
      }

      if (target.page === "updates") {
        dispatch({
          type: "OPEN_UPDATES",
          lane: target.lane ?? null,
          itemId: target.itemId ?? null,
          prompt:
            consumedPrompt && consumedPrompt.page === "updates"
              ? { page: "updates", entry: consumedPrompt }
              : null,
        });
        return;
      }

      if (target.page === "alerts") {
        dispatch({
          type: "OPEN_ALERTS",
          target: { alertId: target.itemId ?? null, reason: target.reason ?? null },
        });
        return;
      }

      // Unknown settings targets normalize to a real tab.
      if (target.page === "settings" && target.focus) {
        navigateTo(`settings:${target.focus}`);
        return;
      }

      if (target.page === "issues") {
        if (target.reason === "no-first-scan") {
          dispatch({ type: "NAVIGATE_ISSUES" });
          openScanConfig();
          return;
        }

        if (target.reason === "onboarding-baseline") {
          dispatch({ type: "NAVIGATE_ISSUES" });
          dispatch({ type: "RESET_ISSUES_TAB" });
          return;
        }

        dispatch({
          type: "NAVIGATE_ISSUES",
          target: {
            focus: target.focus ?? null,
            itemId: target.itemId ?? null,
          },
        });
        return;
      }

      navigateTo(target.page);
    },
    [dispatch, navigateTo, openScanConfig, updateScanBackgrounded],
  );

  const findProjectForTarget = useCallback(
    (target: AppTarget) => {
      const normalizedTarget = withNormalizedTarget(target);
      if (normalizedTarget.projectId != null) {
        return projects.find((entry) => entry.id === normalizedTarget.projectId) ?? null;
      }
      if (!normalizedTarget.url) return null;
      return (
        projects.find((project) =>
          project.environments.some((env) => normalizeTargetUrl(env.url) === normalizedTarget.url),
        ) ?? null
      );
    },
    [projects],
  );

  const openAppTarget = useCallback(
    (rawTarget: AppTarget) => {
      const target = withNormalizedTarget(rawTarget);
      if (
        shouldDeferAppTargetUntilProjectsReady({
          projectCount: projects.length,
          projectsLoading,
          target,
        })
      ) {
        pendingAppTargetRef.current = target;
        return;
      }
      const targetProject = findProjectForTarget(target);
      const normalizedTargetUrl = normalizeTargetUrl(target.url);
      let switchedContext = false;

      if (targetProject && activeProject?.id !== targetProject.id) {
        switchedContext = true;
        selectProject(targetProject);
      }

      if (targetProject && normalizedTargetUrl) {
        const matchingEnv = targetProject.environments.find(
          (env) => normalizeTargetUrl(env.url) === normalizedTargetUrl,
        );
        if (matchingEnv && normalizeTargetUrl(activeEnv?.url) !== normalizedTargetUrl) {
          switchedContext = true;
          selectEnv(matchingEnv);
        }
      }

      if (switchedContext) {
        pendingAppTargetRef.current = target;
        return;
      }

      openAppTargetInCurrentContext(target);
    },
    [
      activeEnv?.url,
      activeProject?.id,
      findProjectForTarget,
      openAppTargetInCurrentContext,
      projects.length,
      projectsLoading,
      selectEnv,
      selectProject,
    ],
  );

  const openOverviewProject = useCallback(
    (projectId: number) => {
      const project = projects.find((entry) => entry.id === projectId);
      if (!project) return;
      selectProject(project);
      navigateTo("dashboard");
    },
    [navigateTo, projects, selectProject],
  );

  useEffect(() => {
    const pendingTarget = pendingAppTargetRef.current;
    if (!pendingTarget) return;
    if (projectsLoading) return;
    const normalizedTarget = withNormalizedTarget(pendingTarget);
    const targetProjectId = normalizedTarget.projectId;
    const targetUrl = normalizeTargetUrl(normalizedTarget.url);
    const projectMatches = targetProjectId == null || activeProject?.id === targetProjectId;
    const urlMatches = !targetUrl || normalizeTargetUrl(activeEnv?.url) === targetUrl;
    if (!projectMatches || !urlMatches) {
      pendingAppTargetRef.current = null;
      openAppTarget(normalizedTarget);
      return;
    }
    pendingAppTargetRef.current = null;
    openAppTargetInCurrentContext(normalizedTarget);
  }, [
    activeEnv?.url,
    activeProject?.id,
    openAppTarget,
    openAppTargetInCurrentContext,
    projects.length,
    projectsLoading,
  ]);

  return {
    navigateTo,
    openAppTarget,
    openOverviewProject,
    openProjectSettings,
  };
}
