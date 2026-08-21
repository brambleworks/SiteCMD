import { useCallback, useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { normalizeAppUrlForOptionalKey } from "@/lib/app-targets";
import { publishProjectNavBadges, publishUpdatesBadgeForReport } from "@/lib/project-nav-badges";
import {
  getProjectNavBadgeSnapshot,
  primeProjectUpdatesSnapshot,
} from "@/lib/project-summary-signals";
import {
  matchesProjectSignalsChangedEvent,
  PROJECT_SIGNALS_CHANGED_EVENT,
} from "@/lib/project-signal-events";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { getRecentPendingProjectUpdates, readUpdateSnapshot } from "@/lib/update-memory";
import type { CodeScanResult, ScanResult, UpdateReport } from "@/lib/types";

export function normalizeShellUrl(value?: string | null) {
  return normalizeAppUrlForOptionalKey(value);
}

interface UseProjectNavBadgeRefreshOptions {
  activeProjectId: number | null;
  activeEnvUrl: string | null;
  activeProjectPath?: string | null;
  result: ScanResult | null;
  codeResult: CodeScanResult | null;
}

function normalizeProjectPath(value?: string | null) {
  const path = value?.trim();
  if (!path || path.startsWith("__url__")) return null;
  return path;
}

function readStoredUpdatesReport(
  projectPath?: string | null,
): Pick<UpdateReport, "updates"> | null {
  const normalizedPath = normalizeProjectPath(projectPath);
  if (!normalizedPath) return null;

  const snapshotUpdates = readUpdateSnapshot(normalizedPath);
  if (snapshotUpdates) {
    return { updates: snapshotUpdates };
  }

  const recentPendingUpdates = getRecentPendingProjectUpdates(normalizedPath);
  return recentPendingUpdates.length > 0 ? { updates: recentPendingUpdates } : null;
}

function publishStoredUpdatesBadge(projectId: number, projectPath?: string | null) {
  const storedReport = readStoredUpdatesReport(projectPath);
  if (!storedReport) return null;
  return publishUpdatesBadgeForReport(projectId, storedReport);
}

export function useProjectNavBadgeRefresh({
  activeProjectId,
  activeEnvUrl,
  activeProjectPath,
  result,
  codeResult,
}: UseProjectNavBadgeRefreshOptions) {
  const queryClient = useQueryClient();
  const loadVersionRef = useRef(0);
  const loadStateRef = useRef<{
    key: string | null;
    inFlight: Promise<void> | null;
    lastCompletedAt: number;
  }>({
    key: null,
    inFlight: null,
    lastCompletedAt: 0,
  });
  const activeScopeRef = useRef<{
    projectId: number | null;
    url: string | null;
    projectPath: string | null;
  }>({
    projectId: null,
    url: null,
    projectPath: null,
  });

  useEffect(() => {
    activeScopeRef.current = {
      projectId: activeProjectId,
      url: normalizeShellUrl(activeEnvUrl),
      projectPath: normalizeProjectPath(activeProjectPath),
    };
  }, [activeEnvUrl, activeProjectId, activeProjectPath]);

  const refreshProjectNavBadges = useCallback(
    async (options?: { bypassCooldown?: boolean }) => {
      if (!activeProjectId || !activeEnvUrl) return;

      const projectId = activeProjectId;
      const environmentUrl = activeEnvUrl;
      const projectPath = normalizeProjectPath(activeProjectPath);
      const normalizedEnvironmentUrl = normalizeShellUrl(environmentUrl);
      const scopeKey = `${projectId}:${normalizedEnvironmentUrl ?? environmentUrl}:${
        projectPath ?? ""
      }`;
      const version = ++loadVersionRef.current;
      const loadState = loadStateRef.current;

      if (loadState.key === scopeKey && loadState.inFlight) {
        return loadState.inFlight;
      }

      if (
        !options?.bypassCooldown &&
        loadState.key === scopeKey &&
        Date.now() - loadState.lastCompletedAt < 1500
      ) {
        return;
      }

      const request = (async () => {
        try {
          const snapshot = options?.bypassCooldown
            ? await getProjectNavBadgeSnapshot(queryClient, projectId, environmentUrl, {
                forceRefresh: true,
              })
            : await getProjectNavBadgeSnapshot(queryClient, projectId, environmentUrl);
          const activeScope = activeScopeRef.current;
          if (loadVersionRef.current !== version) return;
          if (activeScope.projectId !== projectId) return;
          if (activeScope.url !== normalizedEnvironmentUrl) return;
          if (activeScope.projectPath !== projectPath) return;
          publishProjectNavBadges(projectId, snapshot);
          if (!snapshot.signals.updates) {
            publishStoredUpdatesBadge(projectId, projectPath);
          }
        } catch {
          // Sidebar badges are best-effort only.
        } finally {
          if (loadStateRef.current.key === scopeKey) {
            loadStateRef.current.inFlight = null;
            loadStateRef.current.lastCompletedAt = Date.now();
          }
        }
      })();

      loadStateRef.current = {
        key: scopeKey,
        inFlight: request,
        lastCompletedAt: loadState.lastCompletedAt,
      };

      return request;
    },
    [activeEnvUrl, activeProjectId, activeProjectPath, queryClient],
  );

  useEffect(() => {
    if (!activeProjectId || !activeEnvUrl) return;
    publishStoredUpdatesBadge(activeProjectId, activeProjectPath);
    void refreshProjectNavBadges();

    return () => {
      loadVersionRef.current += 1;
      loadStateRef.current.inFlight = null;
    };
  }, [activeEnvUrl, activeProjectId, activeProjectPath, refreshProjectNavBadges]);

  useEffect(() => {
    if (!result || !activeProjectId || !activeEnvUrl) return;
    if (normalizeShellUrl(result.url) !== normalizeShellUrl(activeEnvUrl)) return;
    void refreshProjectNavBadges({ bypassCooldown: true });
  }, [activeEnvUrl, activeProjectId, refreshProjectNavBadges, result]);

  useEffect(() => {
    if (!codeResult || !activeProjectId) return;
    if (codeResult.projectId !== activeProjectId) return;
    void refreshProjectNavBadges({ bypassCooldown: true });
  }, [activeProjectId, codeResult, refreshProjectNavBadges]);

  useTauriEvent(PROJECT_SIGNALS_CHANGED_EVENT, (payload) => {
    const scope = activeScopeRef.current;
    const scopedProjectId = scope.projectId;
    if (scopedProjectId == null) return;
    if (
      !matchesProjectSignalsChangedEvent(payload, {
        projectId: scopedProjectId,
        url: scope.url,
      })
    ) {
      return;
    }
    if (payload.source === "updates" && payload.updates) {
      primeProjectUpdatesSnapshot(
        queryClient,
        scopedProjectId,
        payload.url ?? scope.url,
        payload.updates,
      );
      publishUpdatesBadgeForReport(scopedProjectId, payload.updates);
      return;
    }
    void refreshProjectNavBadges({ bypassCooldown: true });
  });

  // Lifecycle changes emit site-score-changed; refresh the active-count badge.
  useTauriEvent("site-score-changed", () => {
    if (activeScopeRef.current.projectId == null) return;
    void refreshProjectNavBadges({ bypassCooldown: true });
  });

  return { refreshProjectNavBadges };
}
