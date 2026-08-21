import type { PackageUpdate } from "@/lib/types";
import { isJsonRecord } from "@/lib/json-record";
// Shared with the live-scan and snapshot boundaries so a field added to
// PackageUpdate cannot be defaulted three different ways.
import { normalizePackageUpdate } from "@/lib/package-update-normalize";
import { createDurableEntryStore, maxNullableNumber } from "@/lib/durable-entry-store";

interface UpdateMemoryEntry {
  key: string;
  firstSeenAt: number;
  lastSeenAt: number;
  lastPendingAt: number | null;
  lastVerifiedAt: number | null;
  regressedAfterVerifiedAt: number | null;
  currentVersion: string;
  latestVersion: string;
  advisoryFixedVersion: string | null;
  updateType: string;
  isSecurity: boolean;
}

interface UpdateSnapshotEntry {
  updatedAt: number;
  updates: PackageUpdate[];
}

const STORAGE_KEY = "sitecmd_update_memory_v1";
const STORE_KEY = "update-memory";
const SNAPSHOT_STORAGE_KEY = "sitecmd_update_snapshots_v1";
const SNAPSHOT_STORE_KEY = "update-snapshots";
const MAX_ENTRIES = 300;
const MAX_SNAPSHOTS = 50;

function mergeUpdateMemoryEntry(
  durable: UpdateMemoryEntry,
  current: UpdateMemoryEntry,
): UpdateMemoryEntry {
  const lastVerifiedAt = maxNullableNumber(durable.lastVerifiedAt, current.lastVerifiedAt);
  const lastPendingAt = maxNullableNumber(durable.lastPendingAt, current.lastPendingAt);
  const regressedAfterVerifiedAt = maxNullableNumber(
    durable.regressedAfterVerifiedAt,
    current.regressedAfterVerifiedAt,
  );
  return {
    ...durable,
    ...current,
    firstSeenAt: Math.min(durable.firstSeenAt, current.firstSeenAt),
    lastSeenAt: Math.max(durable.lastSeenAt, current.lastSeenAt),
    lastPendingAt,
    lastVerifiedAt,
    regressedAfterVerifiedAt:
      regressedAfterVerifiedAt ??
      (lastVerifiedAt != null && lastPendingAt != null && lastPendingAt > lastVerifiedAt
        ? lastPendingAt
        : null),
  };
}

const memoryStore = createDurableEntryStore<UpdateMemoryEntry>({
  storageKey: STORAGE_KEY,
  storeKey: STORE_KEY,
  max: MAX_ENTRIES,
  parseEntry: parseUpdateMemoryEntry,
  mergeEntry: mergeUpdateMemoryEntry,
  recencyOf: (entry) => entry.lastSeenAt,
});

const snapshotStore = createDurableEntryStore<UpdateSnapshotEntry>({
  storageKey: SNAPSHOT_STORAGE_KEY,
  storeKey: SNAPSHOT_STORE_KEY,
  max: MAX_SNAPSHOTS,
  parseEntry: parseUpdateSnapshotEntry,
  // Snapshots are keyed by project; a later scan replaces the earlier one.
  mergeEntry: (durable, current) => (current.updatedAt >= durable.updatedAt ? current : durable),
  recencyOf: (entry) => entry.updatedAt,
});

function parseUpdateMemoryEntry(value: unknown): UpdateMemoryEntry | null {
  if (!isJsonRecord(value)) return null;
  if (
    typeof value.key !== "string" ||
    typeof value.firstSeenAt !== "number" ||
    typeof value.lastSeenAt !== "number" ||
    typeof value.currentVersion !== "string" ||
    typeof value.latestVersion !== "string" ||
    typeof value.updateType !== "string" ||
    typeof value.isSecurity !== "boolean"
  ) {
    return null;
  }
  return {
    key: value.key,
    firstSeenAt: value.firstSeenAt,
    lastSeenAt: value.lastSeenAt,
    lastPendingAt: typeof value.lastPendingAt === "number" ? value.lastPendingAt : null,
    lastVerifiedAt: typeof value.lastVerifiedAt === "number" ? value.lastVerifiedAt : null,
    regressedAfterVerifiedAt:
      typeof value.regressedAfterVerifiedAt === "number" ? value.regressedAfterVerifiedAt : null,
    currentVersion: value.currentVersion,
    latestVersion: value.latestVersion,
    advisoryFixedVersion:
      typeof value.advisoryFixedVersion === "string" ? value.advisoryFixedVersion : null,
    updateType: value.updateType,
    isSecurity: value.isSecurity,
  };
}

function parseUpdateSnapshotEntry(value: unknown): UpdateSnapshotEntry | null {
  if (
    !isJsonRecord(value) ||
    typeof value.updatedAt !== "number" ||
    !Array.isArray(value.updates)
  ) {
    return null;
  }
  const updates = value.updates.map(normalizePackageUpdate);
  if (updates.some((update) => !update)) return null;
  return {
    updatedAt: value.updatedAt,
    updates: updates as PackageUpdate[],
  };
}

function parseUpdateMemoryKeyForProject(
  projectPath: string,
  key: string,
): {
  ecosystem: PackageUpdate["ecosystem"];
  name: string;
} | null {
  const prefix = `${projectPath}::`;
  if (!key.startsWith(prefix)) return null;
  const remainder = key.slice(prefix.length);
  const [ecosystem, ...nameParts] = remainder.split("::");
  if (!ecosystem || nameParts.length === 0) return null;
  return {
    ecosystem: ecosystem as PackageUpdate["ecosystem"],
    name: nameParts.join("::"),
  };
}

export function buildUpdateMemoryKey(
  projectPath: string,
  update: Pick<PackageUpdate, "ecosystem" | "name">,
): string {
  return `${projectPath}::${update.ecosystem}::${update.name}`;
}

export function __resetUpdateMemoryForTests() {
  memoryStore.reset();
  snapshotStore.reset();
}

export function recordSeenUpdates(projectPath: string, updates: PackageUpdate[]) {
  const store = memoryStore.load();
  const now = Date.now();
  for (const update of updates) {
    const key = buildUpdateMemoryKey(projectPath, update);
    const existing = store[key];
    const regressedAfterVerifiedAt =
      existing?.lastVerifiedAt &&
      (!existing.lastPendingAt || existing.lastPendingAt < existing.lastVerifiedAt)
        ? now
        : (existing?.regressedAfterVerifiedAt ?? null);
    store[key] = {
      key,
      firstSeenAt: existing?.firstSeenAt ?? now,
      lastSeenAt: now,
      lastPendingAt: now,
      lastVerifiedAt: existing?.lastVerifiedAt ?? null,
      regressedAfterVerifiedAt,
      currentVersion: update.currentVersion,
      latestVersion: update.latestVersion,
      advisoryFixedVersion: update.advisoryFixedVersion ?? null,
      updateType: update.updateType,
      isSecurity: update.isSecurity,
    };
  }
  memoryStore.persist(store);
}

export function markUpdateVerified(projectPath: string, update: PackageUpdate) {
  const store = memoryStore.load();
  const now = Date.now();
  const key = buildUpdateMemoryKey(projectPath, update);
  const existing = store[key];
  store[key] = {
    key,
    firstSeenAt: existing?.firstSeenAt ?? now,
    lastSeenAt: existing?.lastSeenAt ?? now,
    lastPendingAt: existing?.lastPendingAt ?? null,
    lastVerifiedAt: now,
    regressedAfterVerifiedAt: existing?.regressedAfterVerifiedAt ?? null,
    currentVersion: update.currentVersion,
    latestVersion: update.latestVersion,
    advisoryFixedVersion: update.advisoryFixedVersion ?? null,
    updateType: update.updateType,
    isSecurity: update.isSecurity,
  };
  memoryStore.persist(store);
}

export function markUpdateStillPending(projectPath: string, update: PackageUpdate) {
  const store = memoryStore.load();
  const now = Date.now();
  const key = buildUpdateMemoryKey(projectPath, update);
  const existing = store[key];
  store[key] = {
    key,
    firstSeenAt: existing?.firstSeenAt ?? now,
    lastSeenAt: now,
    lastPendingAt: now,
    lastVerifiedAt: existing?.lastVerifiedAt ?? null,
    regressedAfterVerifiedAt: existing?.regressedAfterVerifiedAt ?? null,
    currentVersion: update.currentVersion,
    latestVersion: update.latestVersion,
    advisoryFixedVersion: update.advisoryFixedVersion ?? null,
    updateType: update.updateType,
    isSecurity: update.isSecurity,
  };
  memoryStore.persist(store);
}

export function getUpdateMemory(
  projectPath: string,
  update: Pick<PackageUpdate, "ecosystem" | "name">,
): UpdateMemoryEntry | null {
  const store = memoryStore.load();
  return store[buildUpdateMemoryKey(projectPath, update)] ?? null;
}

export function readUpdateSnapshot(projectPath: string): PackageUpdate[] | null {
  const store = snapshotStore.load();
  const snapshot = store[projectPath];
  return snapshot ? [...snapshot.updates] : null;
}

export function writeUpdateSnapshot(projectPath: string, updates: PackageUpdate[]) {
  const store = snapshotStore.load();
  store[projectPath] = {
    updatedAt: Date.now(),
    updates: [...updates],
  };
  snapshotStore.persist(store);
}

export function getRecentPendingProjectUpdates(
  projectPath: string,
  options?: {
    maxAgeMs?: number;
    limit?: number;
  },
): PackageUpdate[] {
  const maxAgeMs = options?.maxAgeMs ?? 1000 * 60 * 60 * 2;
  const limit = options?.limit ?? 20;
  const now = Date.now();

  return Object.values(memoryStore.load())
    .filter(
      (entry) =>
        entry.key.startsWith(`${projectPath}::`) &&
        entry.lastPendingAt != null &&
        now - entry.lastPendingAt <= maxAgeMs &&
        (!entry.lastVerifiedAt || entry.lastPendingAt >= entry.lastVerifiedAt),
    )
    .sort((a, b) => (b.lastPendingAt ?? 0) - (a.lastPendingAt ?? 0))
    .slice(0, limit)
    .flatMap((entry) => {
      const parsedKey = parseUpdateMemoryKeyForProject(projectPath, entry.key);
      if (!parsedKey) return [];
      return [
        {
          name: parsedKey.name,
          ecosystem: parsedKey.ecosystem,
          currentVersion: entry.currentVersion,
          latestVersion: entry.latestVersion,
          updateType: entry.updateType as PackageUpdate["updateType"],
          isSecurity: entry.isSecurity,
          advisorySeverity: null,
          advisoryUrl: null,
          ...(entry.advisoryFixedVersion
            ? { advisoryFixedVersion: entry.advisoryFixedVersion }
            : {}),
          source: "",
          isDev: false,
          isDeprecated: false,
          deprecationMessage: null,
          currentVersionDeprecated: false,
          isStale: false,
          lastPublished: null,
          // Memory entries predate member attribution and only ever hold a
          // package name; the live scan is what fills this in.
          workspaceMembers: [],
        } satisfies PackageUpdate,
      ];
    });
}
