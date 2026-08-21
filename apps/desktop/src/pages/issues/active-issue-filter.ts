import type { IssueStatus } from "@/lib/types";

// Mirror scorer exclusions surfaced by the dossier; regressed issues remain active.
export const INACTIVE_ISSUE_STATUSES: readonly IssueStatus[] = ["blocked", "ignored", "verified"];

const INACTIVE_ISSUE_STATUS_SET: ReadonlySet<string> = new Set(INACTIVE_ISSUE_STATUSES);

export function isInactiveIssueStatus(status: string): boolean {
  return INACTIVE_ISSUE_STATUS_SET.has(status);
}

// Preserve input identity when no inactive check ids are removed.
export function filterActiveWebIssues<T extends { checkId: string }>(
  issues: T[],
  inactiveCheckIds: ReadonlySet<string>,
): T[] {
  if (inactiveCheckIds.size === 0) return issues;
  const filtered = issues.filter((issue) => !inactiveCheckIds.has(issue.checkId));
  return filtered.length === issues.length ? issues : filtered;
}

export function filterActiveCodeIssues<T extends { checkId: string }>(
  issues: T[],
  inactiveCheckIds: ReadonlySet<string>,
): T[] {
  if (inactiveCheckIds.size === 0) return issues;
  const filtered = issues.filter((issue) => !inactiveCheckIds.has(issue.checkId));
  return filtered.length === issues.length ? issues : filtered;
}
