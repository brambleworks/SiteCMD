import type { ProjectWorkSummary } from "@/lib/project-summary-types";

export const EMPTY_PROJECT_WORK_SUMMARY: ProjectWorkSummary = Object.freeze({
  issueCount: 0,
  issueWebCount: 0,
  issueCodeCount: 0,
  issueCriticalCount: 0,
  issueHighCount: 0,
  issueMediumCount: 0,
  issueLowCount: 0,
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
});

export function getProjectWorkSummaryOrEmpty(
  summary: ProjectWorkSummary | null | undefined,
): ProjectWorkSummary {
  return summary ?? EMPTY_PROJECT_WORK_SUMMARY;
}

export function hasProjectWorkSummaryActivity(
  summary: ProjectWorkSummary | null | undefined,
): boolean {
  const resolved = getProjectWorkSummaryOrEmpty(summary);
  return (
    resolved.unresolvedCount > 0 ||
    resolved.newCount > 0 ||
    resolved.workingCount > 0 ||
    resolved.regressedCount > 0 ||
    resolved.ignoredCount > 0 ||
    resolved.blockedCount > 0 ||
    resolved.launchBlockerCount > 0 ||
    resolved.maintenanceCount > 0
  );
}

export function getProjectWorkSummaryIssueTotal(
  summary:
    | Pick<ProjectWorkSummary, "unresolvedCount" | "blockedCount" | "launchBlockerCount">
    | null
    | undefined,
): number {
  if (!summary) return 0;
  const launchBlockerCount = summary.launchBlockerCount ?? 0;
  return Math.max(0, summary.unresolvedCount + summary.blockedCount - launchBlockerCount);
}
