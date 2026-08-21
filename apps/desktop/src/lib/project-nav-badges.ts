import { setIssuesBadgeFromSummary } from "@/lib/issues-badge";
import {
  clearUpdatesBadgeForProject,
  setEnabledIntegrations,
  setUpdatesBadge,
  type UpdatesBadge,
} from "@/lib/nav-badges";
import {
  buildProjectIssueSummaryFromWorkSummary,
  type ProjectIssueSummary,
} from "@/lib/project-issue-summary";
import type { ProjectNavBadgeSnapshot } from "@/lib/project-summary-signals";
import type { UpdateReport } from "@/lib/types";
import { buildUpdateQueueSummary } from "@/lib/update-summary";

interface ProjectNavBadgeState {
  updates: UpdatesBadge | null;
  issues: Pick<ProjectIssueSummary, "totalCount" | "criticalCount">;
  enabledIntegrations: string[];
}

export function buildUpdatesBadgeFromReport(
  projectId: number,
  report: Pick<UpdateReport, "updates"> | null | undefined,
): UpdatesBadge | null {
  const updateSummary = buildUpdateQueueSummary(report?.updates ?? []);
  if (updateSummary.total === 0) return null;
  return {
    projectId,
    total: updateSummary.total,
    critical: updateSummary.security,
  };
}

// All issue-count surfaces read this canonical persisted summary.
export function buildProjectIssueSummaryFromSnapshot(
  snapshot: ProjectNavBadgeSnapshot,
): ProjectIssueSummary {
  return buildProjectIssueSummaryFromWorkSummary(snapshot.signals.workSummary);
}

export function buildProjectNavBadgeState(
  projectId: number,
  snapshot: ProjectNavBadgeSnapshot,
): ProjectNavBadgeState {
  const issueSummary = buildProjectIssueSummaryFromSnapshot(snapshot);

  return {
    updates: buildUpdatesBadgeFromReport(projectId, snapshot.signals.updates),
    issues: {
      totalCount: issueSummary.totalCount,
      criticalCount: issueSummary.criticalCount,
    },
    enabledIntegrations: snapshot.signals.monitoring.enabledIntegrations ?? [],
  };
}

export function publishProjectNavBadges(
  projectId: number,
  snapshot: ProjectNavBadgeSnapshot,
): ProjectNavBadgeState {
  const state = buildProjectNavBadgeState(projectId, snapshot);
  if (state.updates) {
    setUpdatesBadge(state.updates);
  } else {
    clearUpdatesBadgeForProject(projectId);
  }
  setIssuesBadgeFromSummary(projectId, state.issues);
  setEnabledIntegrations(projectId, state.enabledIntegrations);
  return state;
}

export function publishUpdatesBadgeForReport(
  projectId: number,
  report: Pick<UpdateReport, "updates"> | null | undefined,
) {
  const badge = buildUpdatesBadgeFromReport(projectId, report);
  if (badge) {
    setUpdatesBadge(badge);
  } else {
    clearUpdatesBadgeForProject(projectId);
  }
  return badge;
}
