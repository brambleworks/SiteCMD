import { useMemo, useSyncExternalStore } from "react";

export interface UpdatesBadge {
  /** Active project the counts belong to */
  projectId: number;
  /** Total available package updates */
  total: number;
  /** Updates flagged as security/critical */
  critical: number;
}

interface NavBadgesSnapshot {
  updatesByProject: Record<number, UpdatesBadge>;
  /** Enabled integration-type names per project, driving progressive nav pages. */
  enabledIntegrationsByProject: Record<number, string[]>;
}

let snapshot: NavBadgesSnapshot = { updatesByProject: {}, enabledIntegrationsByProject: {} };
const listeners = new Set<() => void>();

function publish() {
  snapshot = { ...snapshot };
  for (const fn of listeners) fn();
}

export function setUpdatesBadge(next: UpdatesBadge | null) {
  if (next === null) {
    if (Object.keys(snapshot.updatesByProject).length === 0) return;
    snapshot.updatesByProject = {};
    publish();
    return;
  }

  const current = snapshot.updatesByProject[next.projectId] ?? null;
  if (
    current?.projectId === next.projectId &&
    current?.total === next.total &&
    current?.critical === next.critical
  ) {
    return;
  }

  snapshot.updatesByProject = {
    ...snapshot.updatesByProject,
    [next.projectId]: next,
  };
  publish();
}

export function clearUpdatesBadgeForProject(projectId: number) {
  if (!(projectId in snapshot.updatesByProject)) return;
  const next = { ...snapshot.updatesByProject };
  delete next[projectId];
  snapshot.updatesByProject = next;
  publish();
}

function sameMembers(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const set = new Set(a);
  return b.every((value) => set.has(value));
}

/** Publish connected integrations only when membership changes. */
export function setEnabledIntegrations(projectId: number, integrations: string[]) {
  const current = snapshot.enabledIntegrationsByProject[projectId];
  if (current == null) {
    if (integrations.length === 0) return;
  } else if (sameMembers(current, integrations)) {
    return;
  }
  snapshot.enabledIntegrationsByProject = {
    ...snapshot.enabledIntegrationsByProject,
    [projectId]: integrations,
  };
  publish();
}

function subscribe(fn: () => void) {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

function getSnapshot() {
  return snapshot;
}

export function useNavBadges(projectId?: number): {
  updates: UpdatesBadge | null;
} {
  const state = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return {
    updates: projectId != null ? (state.updatesByProject[projectId] ?? null) : null,
  };
}

/** Connected integration-type names for a project, as a set for membership checks. */
export function useNavIntegrations(projectId?: number): ReadonlySet<string> {
  const state = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const list = projectId != null ? state.enabledIntegrationsByProject[projectId] : undefined;
  return useMemo(() => new Set(list ?? []), [list]);
}
