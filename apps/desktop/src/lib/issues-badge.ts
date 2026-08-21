import { useSyncExternalStore } from "react";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";

interface IssuesBadge {
  projectId: number;
  total: number;
  critical: number;
}

let badgesByProject: Record<number, IssuesBadge> = {};
const listeners = new Set<() => void>();

function notify() {
  badgesByProject = { ...badgesByProject };
  for (const fn of listeners) fn();
}

/** Publish the shared issue count to the sidebar badge. */
export function setIssuesBadgeFromSummary(
  projectId: number,
  summary: Pick<ProjectIssueSummary, "totalCount" | "criticalCount">,
) {
  setIssuesBadge({
    projectId,
    total: summary.totalCount,
    critical: summary.criticalCount,
  });
}

function setIssuesBadge(next: IssuesBadge) {
  const current = badgesByProject[next.projectId] ?? null;
  if (
    current &&
    current.total === next.total &&
    current.critical === next.critical &&
    current.projectId === next.projectId
  ) {
    return;
  }
  badgesByProject[next.projectId] = next;
  notify();
}

export function clearIssuesBadge() {
  if (Object.keys(badgesByProject).length === 0) return;
  badgesByProject = {};
  notify();
}

export function clearIssuesBadgeForProject(projectId: number) {
  if (!(projectId in badgesByProject)) return;
  const next = { ...badgesByProject };
  delete next[projectId];
  badgesByProject = next;
  notify();
}

function subscribe(fn: () => void) {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

function getSnapshot() {
  return badgesByProject;
}

export function useIssuesBadge(projectId?: number) {
  const state = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return projectId != null ? (state[projectId] ?? null) : null;
}
