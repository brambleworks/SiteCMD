import { normalizeAppUrlForKey, type AppTargetPage } from "@/lib/app-targets";

export interface PendingVerificationEntry {
  id: string;
  projectId: number;
  url: string;
  itemId: string;
  label: string;
  reason: string;
  page: AppTargetPage;
  focus?: string | null;
  filePath?: string | null;
  createdAt: number;
  updatedAt: number;
}

const STORAGE_KEY = "sitecmd_pending_verification_v1";
const EMPTY_PENDING_VERIFICATIONS: PendingVerificationEntry[] = [];

export function buildPendingVerificationId(
  projectId: number,
  url: string,
  itemId: string,
  page?: AppTargetPage,
): string {
  const normalized = normalizeAppUrlForKey(url);
  // Legacy deep-link ids (no page segment) stay stable when no page is given.
  return page === undefined
    ? `${projectId}:${normalized}:${itemId}`
    : `${projectId}:${normalized}:${page}:${itemId}`;
}

// Compatibility shim that clears the retired localStorage verification queue.
export function queuePendingVerification(
  _entry: Omit<PendingVerificationEntry, "id" | "createdAt" | "updatedAt">,
) {
  clearLegacyPendingVerificationStorage();
}

export function queuePendingVerificationMany(
  entriesToQueue: Array<Omit<PendingVerificationEntry, "id" | "createdAt" | "updatedAt">>,
) {
  clearLegacyPendingVerificationStorage();
  for (const entry of entriesToQueue) {
    queuePendingVerification(entry);
  }
}

export function resolvePendingVerification(_id: string) {
  clearLegacyPendingVerificationStorage();
}

export function clearPendingVerification(_filter?: { projectId?: number; url?: string }) {
  clearLegacyPendingVerificationStorage();
}

export function usePendingVerificationCenter(): PendingVerificationEntry[] {
  return EMPTY_PENDING_VERIFICATIONS;
}

function clearLegacyPendingVerificationStorage() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Best-effort cleanup for the retired queue.
  }
}
