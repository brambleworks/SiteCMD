import { useEffect, useRef } from "react";
import { inspectDesktopWatchFiles } from "@/lib/commands";
import { emitAppEvent } from "@/lib/app-events";
import { sendActionableDesktopNotification } from "@/lib/actionable-notifications";
import {
  buildDesktopPromptId,
  buildDesktopWatchPromptCopy,
  normalizeDesktopPromptReason,
  queueDesktopPrompt,
} from "@/lib/desktop-prompts";
import { queuePendingVerification } from "@/lib/pending-verification";
import type { AppTarget } from "@/lib/app-targets";
import { normalizeTargetUrl } from "@/lib/app-targets";
import {
  PROJECT_SIGNALS_CHANGED_EVENT,
  type ProjectSignalsChangedEvent,
} from "@/lib/project-signal-events";
import { buildScheduledScanCompletionCopy } from "@/lib/scan-completion-copy";
import { buildFileWatchNotificationActions } from "@/lib/notification-actions";
import { getOpenTargetLabel } from "@/lib/action-language";
import { getScoreMessage } from "@/lib/types";
import { recordCompletedJob } from "@/lib/jobs";
import { parseJsonRecord, parseNumberRecord } from "@/lib/json-record";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { currentScoreIssueCount, loadCurrentScoreSnapshot } from "@/lib/current-score";
import { getActiveSelection } from "@/lib/active-selection-store";
import { formatUrlDisplay } from "@/lib/utils";
import type { ProjectRecord } from "@/hooks/useProject";
import type { useToast } from "@/hooks/useToast";

interface DesktopWatchRequest {
  projectId: number;
  projectPath: string;
  primaryUrl: string | null;
}

interface DesktopWatchSignal {
  projectId: number;
  url: string | null;
  kind: string;
  relativePath: string;
  absolutePath: string;
  modifiedMs: number;
  page: "search-console" | "updates" | "issues";
  focus?: string | null;
  title: string;
  detail: string;
}

type WorkflowCue = {
  label: string;
  sentence: string;
} | null;

type ToastApi = Pick<ReturnType<typeof useToast>, "success" | "info">;

interface UseAppShellOrchestrationOptions {
  projects: ProjectRecord[];
  projectsLoading: boolean;
  refreshProjects: (options?: { selectNewestImportedProject?: boolean }) => Promise<{
    projects: ProjectRecord[];
    newProject: ProjectRecord | null;
  }>;
  selectProject: (project: ProjectRecord) => void;
  navigateTo: (target: string) => void;
  openTrayScanConfig: () => void;
  showBackgroundedScan: () => void;
  loadHistory: (url: string, projectId?: number) => Promise<unknown> | void;
  toast: ToastApi;
  desktopPrefs: {
    backgroundMonitoring: boolean;
    desktopNotifications: boolean;
    fileWatchSuggestions: boolean;
    refreshOnFocus: boolean;
  };
  normalizeUrl: (value?: string | null) => string | null;
  loadPrimaryWorkflowCue: (
    projectId: number | null | undefined,
    url: string | null | undefined,
  ) => Promise<WorkflowCue>;
}

const DESKTOP_WATCH_CACHE_KEY = "sitecmd_desktop_watch_snapshot_v1";

function loadDesktopWatchSnapshot(): Record<string, number> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(DESKTOP_WATCH_CACHE_KEY);
    if (!raw) return {};
    return parseNumberRecord(parseJsonRecord(raw)) ?? {};
  } catch {
    return {};
  }
}

function saveDesktopWatchSnapshot(snapshot: Record<string, number>) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(DESKTOP_WATCH_CACHE_KEY, JSON.stringify(snapshot));
  } catch {
    // Desktop watch suggestions are best-effort only.
  }
}

async function loadScheduledCompletionScore(
  projectId: number,
  envUrl: string,
  fallbackScore: number,
  fallbackIssues: number,
) {
  try {
    const snapshot = await loadCurrentScoreSnapshot(projectId, envUrl);
    return {
      score: Math.round(snapshot.overall),
      issueCount: currentScoreIssueCount(snapshot),
    };
  } catch {
    return {
      score: fallbackScore,
      issueCount: fallbackIssues,
    };
  }
}

export function useAppShellOrchestration({
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
}: UseAppShellOrchestrationOptions) {
  // Long-lived tray and CLI listeners read current projects without resubscribing.
  const projectsRef = useRef(projects);
  useEffect(() => {
    projectsRef.current = projects;
  }, [projects]);

  useTauriEvent("sitecmd-cli-imported", async (payload) => {
    try {
      const { projects: updated } = await refreshProjects();
      const importedProject = updated.find((project) => project.id === payload.project_id) ?? null;
      if (importedProject) {
        selectProject(importedProject);
        navigateTo("sites");
        const primaryUrl = importedProject.environments[0]?.url ?? payload.url;
        if (payload.imported_scan && primaryUrl) {
          await loadHistory(primaryUrl, importedProject.id);
        }
      }
      toast.success(
        payload.imported_scan ? "Imported project and latest scan" : "Imported project",
        payload.imported_scan
          ? `${payload.name} is now linked and its latest CLI scan is available in SiteCMD.`
          : `${payload.name} is now linked to SiteCMD.`,
      );
    } catch {
      await refreshProjects();
    }
  });

  useTauriEvent("tray-open-overview", () => {
    navigateTo("sites");
  });

  useTauriEvent("tray-scan-now", () => {
    openTrayScanConfig();
  });

  useTauriEvent("tray-show-scan", () => {
    showBackgroundedScan();
  });

  useTauriEvent("scheduled-scan-complete", async (payload) => {
    const hostname = formatUrlDisplay(payload.url);
    const scheduledCompletionScore = await loadScheduledCompletionScore(
      payload.projectId,
      payload.url,
      payload.score,
      payload.issues,
    );
    const scoreMessage = getScoreMessage(scheduledCompletionScore.score);
    const scheduledProject = projectsRef.current.find(
      (project) => project.id === payload.projectId,
    );
    const workflowCue = await loadPrimaryWorkflowCue(payload.projectId, payload.url);
    const copy = buildScheduledScanCompletionCopy({
      scanType: payload.scanType,
      score: scheduledCompletionScore.score,
      issueCount: scheduledCompletionScore.issueCount,
      host: hostname,
      scoreMessage,
      topDomain: payload.topDomain,
      topDomainCount: payload.topDomainCount,
      domainTrendLabel: payload.domainTrendLabel,
      workflowCue: workflowCue
        ? {
            label: workflowCue.label,
            sentence: workflowCue.sentence,
          }
        : null,
    });

    toast.success(copy.title, copy.body);

    recordCompletedJob({
      id: `scheduled-scan:${payload.scanType ?? "health"}:${payload.projectId}:${payload.timestamp ?? Date.now()}`,
      type: "scan",
      label: copy.jobLabel,
      scopeLabel: scheduledProject
        ? `${scheduledProject.name} • ${hostname}`
        : hostname || "Scheduled scan",
      detail: copy.jobDetail,
      target: {
        page: "issues",
        projectId: payload.projectId,
        url: payload.url,
        scanId: payload.scanId ?? null,
        scanKind: payload.scanType === "code" ? "code" : "site",
      },
    });

    // Read the live selection at fire time (not a fire-registration closure):
    // the user may have switched projects during the score fetch above.
    const activeSelection = getActiveSelection();
    const activeEnvUrl = activeSelection.envUrl;
    const activeProjectMatches = activeSelection.projectId === payload.projectId;
    const activeUrlMatches = normalizeUrl(activeEnvUrl) === normalizeUrl(payload.url);
    const shouldReloadHistory =
      payload.scanType === "code"
        ? activeProjectMatches && Boolean(activeEnvUrl)
        : activeProjectMatches && activeUrlMatches;

    if (shouldReloadHistory && activeEnvUrl) {
      await loadHistory(
        payload.scanType === "code" ? activeEnvUrl : payload.url,
        payload.projectId,
      );
    }
    await refreshProjects();
  });

  useEffect(() => {
    let cancelled = false;
    let syncInFlight = false;

    const syncImportedProjects = async () => {
      if (syncInFlight || projectsLoading) return;
      syncInFlight = true;
      try {
        const { newProject } = await refreshProjects({ selectNewestImportedProject: true });
        if (!newProject || cancelled) return;

        navigateTo("sites");
        const primaryUrl = newProject.environments[0]?.url ?? null;
        if (primaryUrl) {
          await loadHistory(primaryUrl, newProject.id);
        }

        toast.success("Imported project", `${newProject.name} is now available in SiteCMD.`);
      } finally {
        syncInFlight = false;
      }
    };

    // Focus and visibility recover CLI imports whose best-effort event was missed.
    const onFocus = () => {
      void syncImportedProjects();
    };
    const onVisibility = () => {
      if (document.visibilityState === "visible") {
        void syncImportedProjects();
      }
    };

    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      cancelled = true;
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [loadHistory, navigateTo, projectsLoading, refreshProjects, toast]);

  useEffect(() => {
    if (!desktopPrefs.backgroundMonitoring || !desktopPrefs.fileWatchSuggestions) {
      return;
    }

    let cancelled = false;

    const checkDesktopSignals = async (allowNotify: boolean) => {
      const requests: DesktopWatchRequest[] = projectsRef.current
        .filter((project) => Boolean(project.path))
        .map((project) => ({
          projectId: project.id,
          projectPath: project.path,
          primaryUrl: project.environments[0]?.url ?? null,
        }));
      if (requests.length === 0) return;

      try {
        const snapshot = loadDesktopWatchSnapshot();
        const signals = (await inspectDesktopWatchFiles({
          requests,
        })) as DesktopWatchSignal[];
        if (cancelled) return;
        const changedProjects = new Map<string, ProjectSignalsChangedEvent>();

        for (const signal of signals) {
          const key = `${signal.projectId}:${signal.relativePath}`;
          const previousModified = snapshot[key];
          snapshot[key] = signal.modifiedMs;
          if (!previousModified || signal.modifiedMs <= previousModified) {
            continue;
          }

          const promptUrl =
            signal.url ??
            projectsRef.current.find((project) => project.id === signal.projectId)?.environments[0]
              ?.url ??
            null;
          const watchReason = normalizeDesktopPromptReason(signal.kind, signal.page);
          const promptId = promptUrl
            ? buildDesktopPromptId(signal.projectId, promptUrl, watchReason, signal.relativePath)
            : null;
          const verifyTarget: AppTarget = {
            page: signal.page as AppTarget["page"],
            projectId: signal.projectId,
            url: promptUrl,
            focus: signal.focus ?? null,
            promptId,
            reason: watchReason,
            filePath: signal.absolutePath,
          };
          const nextActionLabel = promptUrl ? getOpenTargetLabel(verifyTarget) : null;
          const promptCopy = buildDesktopWatchPromptCopy({
            title: signal.title,
            detail: signal.detail,
            page: signal.page,
            reason: watchReason,
            focus: signal.focus ?? null,
            relativePath: signal.relativePath,
            nextActionLabel,
          });

          queueDesktopPrompt({
            projectId: signal.projectId,
            url: promptUrl ?? "",
            page: signal.page,
            focus: signal.focus ?? null,
            title: promptCopy.title,
            detail: promptCopy.detail,
            relativePath: signal.relativePath,
            absolutePath: signal.absolutePath,
            kind: watchReason,
          });
          if (promptUrl) {
            queuePendingVerification({
              projectId: signal.projectId,
              url: promptUrl,
              itemId: promptId ?? `${watchReason}:${signal.relativePath}`,
              label: signal.title,
              reason: `Watched file changed: ${signal.relativePath}`,
              page: signal.page,
              focus: signal.focus ?? null,
              filePath: signal.absolutePath,
            });
          }
          if (promptUrl) {
            const projectKey = `${signal.projectId}:${normalizeTargetUrl(promptUrl)}`;
            changedProjects.set(projectKey, {
              projectId: signal.projectId,
              url: normalizeTargetUrl(promptUrl),
              source: "desktop-watch",
            });
          }
          toast.info(promptCopy.title, promptCopy.detail);
          if (
            allowNotify &&
            desktopPrefs.desktopNotifications &&
            typeof document !== "undefined" &&
            document.visibilityState !== "visible"
          ) {
            void sendActionableDesktopNotification({
              id: `watch:${signal.projectId}:${signal.relativePath}:${signal.modifiedMs}`,
              title: promptCopy.title,
              body: promptCopy.detail,
              clickTarget: { ...verifyTarget },
              actions: buildFileWatchNotificationActions({
                filePath: signal.absolutePath,
                verifyTarget,
              }),
            }).catch(() => {});
          }
        }
        saveDesktopWatchSnapshot(snapshot);
        for (const payload of changedProjects.values()) {
          emitAppEvent(PROJECT_SIGNALS_CHANGED_EVENT, payload);
        }
      } catch {
        // Desktop watch suggestions are best-effort.
      }
    };

    void checkDesktopSignals(false);
    const interval = window.setInterval(() => {
      void checkDesktopSignals(true);
    }, 45000);
    const onFocus = () => {
      if (desktopPrefs.refreshOnFocus) {
        void checkDesktopSignals(false);
      }
    };
    const onVisibility = () => {
      if (desktopPrefs.refreshOnFocus && document.visibilityState === "visible") {
        void checkDesktopSignals(false);
      }
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [desktopPrefs, toast]);
}
