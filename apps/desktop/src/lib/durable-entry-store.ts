import { storeSet, migrateFromLocalStorage } from "@/lib/store";
import { parseRecordMap } from "@/lib/json-record";

export function maxNullableNumber(a: number | null, b: number | null): number | null {
  if (a == null) return b;
  if (b == null) return a;
  return Math.max(a, b);
}

export interface DurableEntryStore<E> {
  /** The current store, from the in-memory cache or localStorage. */
  load: () => Record<string, E>;
  /** Cap to the newest `max` entries, then write through cache + localStorage + Tauri store. */
  persist: (store: Record<string, E>) => void;
  /** Test-only: drop the cache so the next load re-reads. */
  reset: () => void;
}

interface DurableEntryStoreConfig<E> {
  /** localStorage key (the fast, synchronous read tier). */
  storageKey: string;
  /** Tauri store key (the durable tier that survives a localStorage clear). */
  storeKey: string;
  /** Newest-N cap applied on every persist. */
  max: number;
  /** Validate one stored entry; return null to drop it. */
  parseEntry: (value: unknown) => E | null;
  /** Fold a freshly-observed entry into the durable one under the same key. */
  mergeEntry: (durable: E, current: E) => E;
  /** Recency key for the newest-N cap (higher = kept). */
  recencyOf: (entry: E) => number;
}

/** Capped localStorage and Tauri store with caller-defined validation and merging. */
export function createDurableEntryStore<E>(
  config: DurableEntryStoreConfig<E>,
): DurableEntryStore<E> {
  const { storageKey, storeKey, max, parseEntry, mergeEntry, recencyOf } = config;

  let cached: Record<string, E> | null = null;
  let dirty = false;

  const parseStore = (value: unknown): Record<string, E> | null =>
    parseRecordMap(value, parseEntry);

  function writeLocalStorage(store: Record<string, E>): void {
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem(storageKey, JSON.stringify(store));
    } catch {
      // Best effort - the durable store still has the value.
    }
  }

  function mergeStores(durable: Record<string, E>, current: Record<string, E>): Record<string, E> {
    const merged = { ...durable };
    for (const [key, currentEntry] of Object.entries(current)) {
      merged[key] = merged[key] ? mergeEntry(merged[key], currentEntry) : currentEntry;
    }
    return merged;
  }

  // Merge writes that land while the durable store is hydrating.
  migrateFromLocalStorage<Record<string, E>>(storageKey, storeKey, {}, parseStore)
    .then((store) => {
      if (dirty && cached) {
        cached = mergeStores(store, cached);
        writeLocalStorage(cached);
        storeSet(storeKey, cached).catch(() => {});
        return;
      }
      cached = store;
    })
    .catch(() => {});

  function load(): Record<string, E> {
    if (cached) return cached;
    if (typeof window === "undefined") return {};
    try {
      const raw = window.localStorage.getItem(storageKey);
      if (!raw) return {};
      const parsed = parseStore(JSON.parse(raw) as unknown);
      if (parsed) {
        cached = parsed;
        return cached;
      }
    } catch {
      // Best effort.
    }
    return {};
  }

  function persist(store: Record<string, E>): void {
    if (typeof window === "undefined") return;
    const capped = Object.entries(store)
      .sort(([, a], [, b]) => recencyOf(b) - recencyOf(a))
      .slice(0, max);
    const next = Object.fromEntries(capped);
    dirty = true;
    cached = next;
    writeLocalStorage(next);
    storeSet(storeKey, next).catch(() => {});
  }

  function reset(): void {
    cached = null;
    dirty = false;
  }

  return { load, persist, reset };
}
