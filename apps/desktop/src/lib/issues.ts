import { emitAppEvent } from "@/lib/app-events";
import {
  blockIssue as blockIssueCmd,
  dismissIntegrationHint as dismissIntegrationHintCmd,
  getIssuesForPage,
  getIssueState as getIssueStateCmd,
  getPagesWithIssues,
  ignoreIssue as ignoreIssueCmd,
  reopenIssue as reopenIssueCmd,
  snoozeIssue as snoozeIssueCmd,
  verifyIssue as verifyIssueCmd,
} from "@/lib/commands";
import { createSeverityCounts, isSeverity, type SeverityCounts } from "./severity";
import type { IssueVerificationOutcome } from "@/generated/ipc-bindings";
import type { IssueGroup, IssueStatus, PageSummary } from "./types";

// Lifecycle event for status-filtered surfaces; score changes use a separate event.
export const ISSUE_LIFECYCLE_CHANGED_EVENT = "issue-lifecycle-changed";

function emitIssueLifecycleChanged(projectId: number, checkId: string, status: IssueStatus): void {
  // Best-effort; the Issues list also refreshes on site-score-changed.
  emitAppEvent(ISSUE_LIFECYCLE_CHANGED_EVENT, { projectId, checkId, status });
}

type CheckStatusLike = string | null | undefined;
type CheckLike = { status: CheckStatusLike };
type SeverityLike = { severity?: string | null };

export function isActionableCheckStatus(status: CheckStatusLike): boolean {
  return status === "fail" || status === "warn";
}

export function isPassingCheckStatus(status: CheckStatusLike): boolean {
  return status === "pass";
}

export function formatCheckStatus(status: CheckStatusLike): string {
  switch (status) {
    case "pass":
      return "Pass";
    case "fail":
      return "Fail";
    case "warn":
      return "Warn";
    case "skipped":
      return "Skipped";
    default:
      return status ?? "";
  }
}

export function isActionableCheckResult<T extends CheckLike>(issue: T): issue is T {
  return isActionableCheckStatus(issue.status);
}

export function isPassingCheckResult<T extends CheckLike>(issue: T): issue is T {
  return isPassingCheckStatus(issue.status);
}

export function filterActionableCheckResults<T extends CheckLike>(issues: readonly T[]): T[] {
  return issues.filter(isActionableCheckResult);
}

export function countActionableCheckResults(issues: readonly CheckLike[]): number {
  return issues.filter(isActionableCheckResult).length;
}

export function countPassingCheckResults(issues: readonly CheckLike[]): number {
  return issues.filter(isPassingCheckResult).length;
}

export function summarizeIssueSeverities(issues: readonly SeverityLike[]): SeverityCounts {
  const counts = createSeverityCounts();
  for (const issue of issues) {
    if (isSeverity(issue.severity)) counts[issue.severity] += 1;
  }
  return counts;
}

export function summarizeActionableCheckSeverities(
  issues: readonly (CheckLike & SeverityLike)[],
): SeverityCounts {
  return summarizeIssueSeverities(issues.filter(isActionableCheckResult));
}

export async function ignoreIssue(
  projectId: number,
  envUrl: string,
  checkId: string,
): Promise<void> {
  await ignoreIssueCmd({ projectId, envUrl, checkId });
  emitIssueLifecycleChanged(projectId, checkId, "ignored");
}

export async function verifyIssue(
  projectId: number,
  envUrl: string,
  checkId: string,
): Promise<IssueVerificationOutcome> {
  const outcome = await verifyIssueCmd({ projectId, envUrl, checkId });
  if (outcome.status === "verified") {
    emitIssueLifecycleChanged(projectId, checkId, "verified");
  }
  return outcome;
}

export async function blockIssue(
  projectId: number,
  envUrl: string,
  checkId: string,
  reason: string,
): Promise<void> {
  await blockIssueCmd({ projectId, envUrl, checkId, reason });
  emitIssueLifecycleChanged(projectId, checkId, "blocked");
}

export async function snoozeIssue(
  projectId: number,
  envUrl: string,
  checkId: string,
  snoozeUntil: number,
): Promise<void> {
  await snoozeIssueCmd({ projectId, envUrl, checkId, snoozeUntil });
  emitIssueLifecycleChanged(projectId, checkId, "snoozed");
}

export async function reopenIssue(
  projectId: number,
  envUrl: string,
  checkId: string,
): Promise<void> {
  await reopenIssueCmd({ projectId, envUrl, checkId });
  emitIssueLifecycleChanged(projectId, checkId, "new");
}

/** Return the issue-state overlay tuple, or null when none is set. */
export async function getIssueState(
  projectId: number,
  envUrl: string,
  checkId: string,
): Promise<[string, number | null, string | null, string | null] | null> {
  return getIssueStateCmd({ projectId, envUrl, checkId });
}

export async function getIssuePages(projectId: number, envUrl: string): Promise<PageSummary[]> {
  return getPagesWithIssues({ projectId, envUrl });
}

export async function getPageIssues(
  projectId: number,
  envUrl: string,
  pageUrl: string,
): Promise<IssueGroup[]> {
  return getIssuesForPage({ projectId, envUrl, pageUrl });
}

export async function dismissIntegrationHint(
  projectId: number,
  checkId: string,
  integrationType: string,
): Promise<void> {
  await dismissIntegrationHintCmd({ projectId, checkId, integrationType });
  // Refetch groups so backend-filtered integration suggestions reflect dismissal.
  emitAppEvent("integration-hint-dismissed", { projectId, checkId, integration: integrationType });
}
