import { normalizeHttpTargetUrl } from "./app-targets";
import { parseJsonRecord } from "./json-record";

const PROJECT_SELECTION_STORAGE_KEY = "sitecmd_project_selection_v1";

function normalizeSelectionUrl(url: string | null | undefined): string | null {
  return normalizeHttpTargetUrl(url);
}

function parseProjectId(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? value : null;
}

export function readStoredProjectSelection(): { projectId: number; envUrl: string | null } | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(PROJECT_SELECTION_STORAGE_KEY);
    if (!raw) return null;
    const parsed = parseJsonRecord(raw);
    if (!parsed) return null;
    const projectId = parseProjectId(parsed.projectId);
    if (!projectId) return null;
    return {
      projectId,
      envUrl: typeof parsed.envUrl === "string" ? normalizeSelectionUrl(parsed.envUrl) : null,
    };
  } catch {
    return null;
  }
}

export function persistProjectSelection(projectId: number | null, envUrl: string | null) {
  if (typeof window === "undefined") return;
  try {
    const normalizedProjectId = parseProjectId(projectId);
    if (normalizedProjectId == null) {
      window.localStorage.removeItem(PROJECT_SELECTION_STORAGE_KEY);
      return;
    }
    window.localStorage.setItem(
      PROJECT_SELECTION_STORAGE_KEY,
      JSON.stringify({ projectId: normalizedProjectId, envUrl: normalizeSelectionUrl(envUrl) }),
    );
  } catch {
    // best effort
  }
}

export function clearStoredProjectSelection() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(PROJECT_SELECTION_STORAGE_KEY);
  } catch {
    // best effort
  }
}

export function normalizeStoredProjectSelectionUrl(url: string | null | undefined): string | null {
  return normalizeSelectionUrl(url);
}
