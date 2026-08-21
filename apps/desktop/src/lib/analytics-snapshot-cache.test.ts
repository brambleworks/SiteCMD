import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import {
  __resetAnalyticsSnapshotCacheForTests,
  buildAnalyticsSnapshotKey,
  clearAnalyticsSnapshots,
  readAnalyticsSnapshot,
  writeAnalyticsSnapshot,
} from "./analytics-snapshot-cache";

const LOCAL_KEY = "sitecmd_analytics_snapshots_v1";

describe("analytics-snapshot-cache", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __resetAnalyticsSnapshotCacheForTests();
  });

  afterEach(() => {
    window.localStorage.clear();
    __resetAnalyticsSnapshotCacheForTests();
  });

  it("buildAnalyticsSnapshotKey joins projectId, period, and optional variant", () => {
    expect(buildAnalyticsSnapshotKey(7, "28d")).toBe("7:28d");
    expect(buildAnalyticsSnapshotKey(7, "28d", "traffic")).toBe("7:28d:traffic");
  });

  it("round-trips a write and read", () => {
    const key = buildAnalyticsSnapshotKey(1, "28d");
    writeAnalyticsSnapshot(key, { total_clicks: 42 }, 1_700_000_000_000);
    expect(readAnalyticsSnapshot(key, 1_700_000_000_000)).toEqual({ total_clicks: 42 });
  });

  it("returns null for keys that have never been written", () => {
    expect(readAnalyticsSnapshot(buildAnalyticsSnapshotKey(99, "28d"))).toBeNull();
  });

  it("treats snapshots older than 24h as missing so callers force a refetch", () => {
    const key = buildAnalyticsSnapshotKey(1, "28d");
    const writtenAt = 1_700_000_000_000;
    writeAnalyticsSnapshot(key, { total_clicks: 42 }, writtenAt);

    const dayAndOneMs = 24 * 60 * 60 * 1000 + 1;
    expect(readAnalyticsSnapshot(key, writtenAt + dayAndOneMs)).toBeNull();
    expect(readAnalyticsSnapshot(key, writtenAt + dayAndOneMs - 2)).toEqual({ total_clicks: 42 });
  });

  it("persists to localStorage so a fresh module load can hydrate synchronously", () => {
    const key = buildAnalyticsSnapshotKey(1, "28d");
    writeAnalyticsSnapshot(key, { total_clicks: 42 }, 1_700_000_000_000);

    __resetAnalyticsSnapshotCacheForTests();

    expect(readAnalyticsSnapshot(key, 1_700_000_000_000)).toEqual({ total_clicks: 42 });
    const raw = window.localStorage.getItem(LOCAL_KEY);
    expect(raw).not.toBeNull();
  });

  it("caps the snapshot store at 50 entries, dropping the oldest first", () => {
    for (let i = 0; i < 60; i++) {
      writeAnalyticsSnapshot(buildAnalyticsSnapshotKey(i, "28d"), { id: i }, 1_700_000_000_000 + i);
    }
    __resetAnalyticsSnapshotCacheForTests();
    const persisted = JSON.parse(window.localStorage.getItem(LOCAL_KEY) ?? "{}") as Record<
      string,
      unknown
    >;
    expect(Object.keys(persisted)).toHaveLength(50);
    // Oldest 10 (ids 0..9) were dropped; newest 50 (ids 10..59) survive.
    expect(persisted["0:28d"]).toBeUndefined();
    expect(persisted["9:28d"]).toBeUndefined();
    expect(persisted["10:28d"]).toBeDefined();
    expect(persisted["59:28d"]).toBeDefined();
  });

  it("ignores malformed localStorage entries on hydration", () => {
    window.localStorage.setItem(LOCAL_KEY, "not-json");
    __resetAnalyticsSnapshotCacheForTests();
    expect(readAnalyticsSnapshot(buildAnalyticsSnapshotKey(1, "28d"))).toBeNull();
  });

  it("ignores entries that are missing fetchedAt", () => {
    window.localStorage.setItem(
      LOCAL_KEY,
      JSON.stringify({ "1:28d": { data: { total_clicks: 42 } } }),
    );
    __resetAnalyticsSnapshotCacheForTests();
    expect(readAnalyticsSnapshot(buildAnalyticsSnapshotKey(1, "28d"))).toBeNull();
  });

  it("clearAnalyticsSnapshots wipes everything", () => {
    writeAnalyticsSnapshot(buildAnalyticsSnapshotKey(1, "28d"), { ok: true });
    clearAnalyticsSnapshots();
    expect(readAnalyticsSnapshot(buildAnalyticsSnapshotKey(1, "28d"))).toBeNull();
    expect(window.localStorage.getItem(LOCAL_KEY)).toBeNull();
  });
});
