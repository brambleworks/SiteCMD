import { storeSet } from "@/lib/store";
import { coerceJsonRecord, isJsonRecord } from "@/lib/json-record";
import { MS_PER_DAY } from "@/lib/format";

interface AnalyticsSnapshotEntry {
  data: unknown;
  fetchedAt: number;
}

type AnalyticsSnapshotStore = Record<string, AnalyticsSnapshotEntry>;

const LOCAL_STORAGE_KEY = "sitecmd_analytics_snapshots_v1";
const TAURI_STORE_KEY = "analytics-snapshots";
const MAX_AGE_MS = MS_PER_DAY; // 24h - older than this and we force a refetch
const MAX_ENTRIES = 50;

let cache: AnalyticsSnapshotStore | null = null;

function hydrate(): AnalyticsSnapshotStore {
  if (cache) return cache;
  if (typeof window === "undefined") {
    cache = {};
    return cache;
  }
  try {
    const raw = window.localStorage.getItem(LOCAL_STORAGE_KEY);
    if (!raw) {
      cache = {};
      return cache;
    }
    const parsed = coerceJsonRecord(raw);
    cache = parsed ? sanitize(parsed) : {};
  } catch {
    cache = {};
  }
  return cache;
}

function sanitize(record: Record<string, unknown>): AnalyticsSnapshotStore {
  const next: AnalyticsSnapshotStore = {};
  for (const [key, entry] of Object.entries(record)) {
    if (!isJsonRecord(entry)) continue;
    const fetchedAt = typeof entry.fetchedAt === "number" ? entry.fetchedAt : null;
    if (fetchedAt == null || !Number.isFinite(fetchedAt)) continue;
    if ("data" in entry) {
      next[key] = { data: entry.data, fetchedAt };
    }
  }
  return next;
}

function persist(): void {
  if (typeof window === "undefined" || !cache) return;
  try {
    window.localStorage.setItem(LOCAL_STORAGE_KEY, JSON.stringify(cache));
  } catch {
    // best-effort; Tauri store still gets a copy below
  }
  void storeSet(TAURI_STORE_KEY, cache);
}

export function buildAnalyticsSnapshotKey(
  projectId: number,
  period: string,
  variant?: string,
): string {
  return variant ? `${projectId}:${period}:${variant}` : `${projectId}:${period}`;
}

export function readAnalyticsSnapshot<T>(key: string, now: number = Date.now()): T | null {
  const store = hydrate();
  const entry = store[key];
  if (!entry) return null;
  if (now - entry.fetchedAt > MAX_AGE_MS) return null;
  return entry.data as T;
}

export function writeAnalyticsSnapshot<T>(key: string, data: T, now: number = Date.now()): void {
  const store = hydrate();
  store[key] = { data, fetchedAt: now };
  if (Object.keys(store).length > MAX_ENTRIES) {
    const ranked = Object.entries(store).sort((a, b) => b[1].fetchedAt - a[1].fetchedAt);
    cache = Object.fromEntries(ranked.slice(0, MAX_ENTRIES));
  }
  persist();
}

export function clearAnalyticsSnapshots(): void {
  cache = {};
  if (typeof window !== "undefined") {
    try {
      window.localStorage.removeItem(LOCAL_STORAGE_KEY);
    } catch {
      // best-effort
    }
  }
  void storeSet(TAURI_STORE_KEY, {});
}

/** Reset memory so tests can force rehydration from localStorage. */
export function __resetAnalyticsSnapshotCacheForTests(): void {
  cache = null;
}
