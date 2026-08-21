import type { QueryClient } from "@tanstack/react-query";
import { safeListen } from "@/lib/tauri-events";
import { ISSUE_LIFECYCLE_CHANGED_EVENT } from "@/lib/issues";
import type { ProjectSignalsChangedEvent } from "@/lib/project-signal-events";
import { clearProjectSignalSessionCache } from "@/lib/project-summary-signals";
import { queryKeys } from "./query-keys";

// Canonical Tauri event to query-invalidation registry.
/** Query family with project-scoped and full-sweep keys. */
interface ProjectScopedFamily {
  readonly all: readonly unknown[];
  readonly projectScope: (projectId: number) => readonly unknown[];
}

interface QueryInvalidationRule {
  /** Tauri event name. */
  readonly event: string;
  /** App-wide or cross-project keys. */
  readonly keys: readonly (readonly unknown[])[];
  /** Project keys, with automatic full sweep when the payload lacks a project. */
  readonly projectScoped?: readonly ProjectScopedFamily[];
  /** Extra keys derived from the payload. */
  readonly additionalKeys?: (payload: unknown) => readonly (readonly unknown[])[];
  /** Clear state outside TanStack Query. */
  readonly onEvent?: (payload: unknown) => void;
}

/** Read a project id or require a full family sweep. */
function projectIdFromPayload(payload: unknown): number | null {
  const event = payload as { projectId?: unknown } | null;
  return typeof event?.projectId === "number" ? event.projectId : null;
}

function clearProjectSummarySession(payload: unknown) {
  const event = payload as { projectId?: unknown } | null;
  if (typeof event?.projectId === "number") clearProjectSignalSessionCache(event.projectId);
}

function integrationSignalKeys(payload: unknown): readonly (readonly unknown[])[] {
  const signal = payload as Partial<ProjectSignalsChangedEvent> | null;
  if (signal?.source !== "integration" || typeof signal.projectId !== "number") return [];
  return [
    queryKeys.integrations.forProject(signal.projectId),
    queryKeys.analytics.forProject(signal.projectId),
  ];
}

export const QUERY_INVALIDATION_RULES: readonly QueryInvalidationRule[] = [
  // Refresh canonical details after every requested collector settles.
  {
    event: "scan-execution-completed",
    keys: [
      queryKeys.scanExecution.all,
      queryKeys.searchScan.all,
      queryKeys.sites.all,
      queryKeys.settings.databaseInfo(),
    ],
    projectScoped: [
      queryKeys.codeScanAudit,
      queryKeys.currentScore,
      queryKeys.workItems,
      queryKeys.pageIssues,
      queryKeys.issuePages,
      queryKeys.issueMemory,
      queryKeys.events,
      queryKeys.projectSummary,
      queryKeys.reports,
      queryKeys.alerts,
      queryKeys.deploys,
    ],
    onEvent: clearProjectSummarySession,
  },
  // Issue lifecycle changes affect active work items and score-derived views.
  {
    event: ISSUE_LIFECYCLE_CHANGED_EVENT,
    keys: [queryKeys.sites.all],
    projectScoped: [
      queryKeys.workItems,
      queryKeys.pageIssues,
      queryKeys.issuePages,
      queryKeys.issueMemory,
      queryKeys.resolvedIssues,
      queryKeys.currentScore,
      queryKeys.projectSummary,
      queryKeys.reports,
    ],
    onEvent: clearProjectSummarySession,
  },
  {
    event: "site-score-changed",
    keys: [queryKeys.sites.all],
    projectScoped: [
      queryKeys.workItems,
      queryKeys.pageIssues,
      queryKeys.issuePages,
      queryKeys.issueMemory,
      queryKeys.resolvedIssues,
      queryKeys.currentScore,
      queryKeys.projectSummary,
      queryKeys.reports,
      queryKeys.alerts,
      queryKeys.deploys,
    ],
    onEvent: clearProjectSummarySession,
  },
  {
    event: "project-signals-changed",
    keys: [queryKeys.sites.all],
    projectScoped: [
      queryKeys.projectSummary,
      queryKeys.updates,
      queryKeys.events,
      queryKeys.issueMemory,
      queryKeys.reports,
      queryKeys.deploys,
    ],
    // Refresh hidden credential changes only for integration signals.
    additionalKeys: integrationSignalKeys,
    onEvent: clearProjectSummarySession,
  },
  {
    event: "google-integration-updated",
    keys: [],
    projectScoped: [queryKeys.integrations, queryKeys.analytics, queryKeys.projectSummary],
    onEvent: clearProjectSummarySession,
  },
  // Unit payload forces a full family sweep.
  {
    event: "fix-attempt-updated",
    keys: [],
    projectScoped: [
      queryKeys.workItems,
      queryKeys.pageIssues,
      queryKeys.currentScore,
      queryKeys.projectSummary,
      queryKeys.events,
    ],
  },
  { event: "alerts-changed", keys: [], projectScoped: [queryKeys.alerts] },
  // Timeline rows written outside scans.
  {
    event: "events-recorded",
    keys: [],
    projectScoped: [queryKeys.events, queryKeys.projectSummary],
    onEvent: clearProjectSummarySession,
  },
  {
    event: "integration-hint-dismissed",
    keys: [],
    projectScoped: [queryKeys.pageIssues, queryKeys.workItems],
  },
  {
    event: "catalog-updated",
    keys: [queryKeys.settings.catalogStatus()],
    projectScoped: [],
  },
  {
    event: "catalog-refresh-completed",
    keys: [queryKeys.settings.catalogStatus()],
    projectScoped: [],
  },
];

/** Install invalidation listeners and return their disposer. */
export function installQueryEventInvalidation(queryClient: QueryClient): () => void {
  const unlisteners = QUERY_INVALIDATION_RULES.map((rule) =>
    safeListen<unknown>(rule.event, (event) => {
      rule.onEvent?.(event.payload);
      const projectId = projectIdFromPayload(event.payload);
      const scopedKeys = (rule.projectScoped ?? []).map((family) =>
        projectId != null ? family.projectScope(projectId) : family.all,
      );
      const additionalKeys = rule.additionalKeys?.(event.payload) ?? [];
      for (const queryKey of [...rule.keys, ...scopedKeys, ...additionalKeys]) {
        void queryClient.invalidateQueries({ queryKey });
      }
    }),
  );

  return () => {
    for (const unlisten of unlisteners) {
      void unlisten.then((fn) => fn());
    }
  };
}
