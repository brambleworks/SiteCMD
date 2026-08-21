import { normalizeAppUrlForKey } from "@/lib/app-targets";
import type { UpdateReport } from "@/lib/types";

export const PROJECT_SIGNALS_CHANGED_EVENT = "project-signals-changed";

export interface ProjectSignalsChangedEvent {
  projectId: number;
  url: string | null;
  source: "desktop-watch" | "issues" | "updates" | "integration";
  updates?: UpdateReport | null;
}

function normalizeUrl(url?: string | null): string {
  return normalizeAppUrlForKey(url);
}

export function matchesProjectSignalsChangedEvent(
  payload: ProjectSignalsChangedEvent,
  target: { projectId: number; url?: string | null },
): boolean {
  if (payload.projectId !== target.projectId) return false;
  if (!payload.url || !target.url) return true;
  return normalizeUrl(payload.url) === normalizeUrl(target.url);
}
