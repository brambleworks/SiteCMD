import { isJsonRecord } from "@/lib/json-record";
import { normalizeAppUrlForKey } from "@/lib/app-targets";

// Persist slim dashboard summaries across reloads; events remain the freshness authority.
export const SNAPSHOT_CACHE_TTL_MS = 5 * 60_000;

export interface SnapshotCacheEntry<T> {
  snapshot: T;
  cachedAt: number;
  /** True when callers must refetch detail over IPC. */
  partial?: boolean;
}

export function parseSnapshotCacheEntry<T>(value: unknown): SnapshotCacheEntry<T> | null {
  if (!isJsonRecord(value)) return null;
  if (!("snapshot" in value)) return null;
  if (typeof value.cachedAt !== "number" || !Number.isFinite(value.cachedAt)) return null;
  const entry: SnapshotCacheEntry<T> = {
    snapshot: value.snapshot as T,
    cachedAt: value.cachedAt,
  };
  if (value.partial === true) {
    entry.partial = true;
  }
  return entry;
}

export function snapshotCacheKey(projectId: number, url?: string | null) {
  return `${projectId}:${normalizeAppUrlForKey(url)}`;
}

function readSessionSnapshotCache<T>(storagePrefix: string, key: string) {
  if (typeof window === "undefined") return null;
  const storageKey = `${storagePrefix}${key}`;
  try {
    const raw = window.sessionStorage.getItem(storageKey);
    if (!raw) return null;
    const parsed = parseSnapshotCacheEntry<T>(JSON.parse(raw) as unknown);
    if (!parsed) {
      window.sessionStorage.removeItem(storageKey);
    }
    return parsed;
  } catch {
    window.sessionStorage.removeItem(storageKey);
    return null;
  }
}

function writeSessionSnapshotCache<T>(
  storagePrefix: string,
  key: string,
  entry: SnapshotCacheEntry<T>,
) {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.setItem(`${storagePrefix}${key}`, JSON.stringify(entry));
  } catch {
    // best effort only
  }
}

export function clearSessionSnapshotCache(storagePrefix: string, key: string) {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.removeItem(`${storagePrefix}${key}`);
  } catch {
    // best effort only
  }
}

// TTL-gated cross-reload cache; QueryClient owns in-session state.
export function readFreshSessionSnapshot<T>(
  storagePrefix: string,
  key: string,
): SnapshotCacheEntry<T> | null {
  const cached = readSessionSnapshotCache<T>(storagePrefix, key);
  if (!cached) return null;
  if (Date.now() - cached.cachedAt > SNAPSHOT_CACHE_TTL_MS) {
    clearSessionSnapshotCache(storagePrefix, key);
    return null;
  }
  return cached;
}

// Optional slimming keeps large snapshots within serialization and storage budgets.
export function writeSessionSnapshot<T>(
  storagePrefix: string,
  key: string,
  snapshot: T,
  cachedAt: number,
  slimForSession?: (snapshot: T) => T,
): void {
  const entry: SnapshotCacheEntry<T> = slimForSession
    ? { snapshot: slimForSession(snapshot), cachedAt, partial: true }
    : { snapshot, cachedAt };
  writeSessionSnapshotCache(storagePrefix, key, entry);
}
