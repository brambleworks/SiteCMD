import {
  normalizeStoredProjectSelectionUrl,
  persistProjectSelection,
} from "./project-selection-state";

export interface ActiveSelection {
  projectId: number | null;
  envUrl: string | null;
}

const EMPTY: ActiveSelection = { projectId: null, envUrl: null };

let snapshot: ActiveSelection = EMPTY;
const listeners = new Set<() => void>();

/** Synchronous read of the current selection key. */
export function getActiveSelection(): ActiveSelection {
  return snapshot;
}

/** Subscribe to selection changes. Returns an unsubscribe function. */
export function subscribeActiveSelection(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Persist changed selections while preserving unchanged snapshot identity. */
export function setActiveSelection(projectId: number | null, envUrl: string | null): boolean {
  const nextProjectId = projectId ?? null;
  const nextEnvUrl = normalizeStoredProjectSelectionUrl(envUrl);
  if (snapshot.projectId === nextProjectId && snapshot.envUrl === nextEnvUrl) {
    return false;
  }
  snapshot = { projectId: nextProjectId, envUrl: nextEnvUrl };
  persistProjectSelection(nextProjectId, nextEnvUrl);
  for (const listener of listeners) {
    listener();
  }
  return true;
}

/** Reset the singleton between tests. */
export function resetActiveSelectionForTest() {
  snapshot = EMPTY;
  listeners.clear();
}
