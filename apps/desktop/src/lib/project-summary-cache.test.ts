import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  parseSnapshotCacheEntry,
  readFreshSessionSnapshot,
  snapshotCacheKey,
  writeSessionSnapshot,
} from "./project-summary-cache";

describe("project-summary-cache", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-05-07T12:00:00.000Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("parses only cache entries with a snapshot and finite cachedAt", () => {
    expect(parseSnapshotCacheEntry(null)).toBeNull();
    expect(parseSnapshotCacheEntry({ cachedAt: Date.now() })).toBeNull();
    expect(parseSnapshotCacheEntry({ snapshot: { ok: true }, cachedAt: "now" })).toBeNull();
    expect(parseSnapshotCacheEntry({ snapshot: { ok: true }, cachedAt: Date.now() })).toEqual({
      snapshot: { ok: true },
      cachedAt: Date.now(),
    });
  });

  it("hydrates valid session cache entries", () => {
    const key = snapshotCacheKey(1, "https://example.com/");
    window.sessionStorage.setItem(
      `test:${key}`,
      JSON.stringify({ snapshot: { score: 92 }, cachedAt: Date.now() }),
    );

    expect(readFreshSessionSnapshot<{ score: number }>("test:", key)?.snapshot).toEqual({
      score: 92,
    });
  });

  it("drops malformed session cache entries instead of hydrating them", () => {
    const key = snapshotCacheKey(1, "https://example.com/");
    window.sessionStorage.setItem(`test:${key}`, JSON.stringify({ cachedAt: Date.now() }));

    expect(readFreshSessionSnapshot("test:", key)).toBeNull();
    expect(window.sessionStorage.getItem(`test:${key}`)).toBeNull();
  });

  it("keeps entries fresh across page switches instead of expiring after 30s", () => {
    // Event invalidation, not ordinary navigation, controls freshness.
    const key = snapshotCacheKey(1, "https://example.com/");
    writeSessionSnapshot("test:", key, { score: 92 }, Date.now());
    vi.advanceTimersByTime(31_000);

    expect(readFreshSessionSnapshot<{ score: number }>("test:", key)?.snapshot).toEqual({
      score: 92,
    });
  });

  it("expires old cache entries from session storage", () => {
    const key = snapshotCacheKey(1, "https://example.com/");
    writeSessionSnapshot("test:", key, { score: 92 }, Date.now());
    vi.advanceTimersByTime(5 * 60_000 + 1_000);

    expect(readFreshSessionSnapshot("test:", key)).toBeNull();
    expect(window.sessionStorage.getItem(`test:${key}`)).toBeNull();
  });

  it("persists a slimmed, partial-flagged copy to session storage when a slimmer is provided", () => {
    type Snapshot = { score: number; detail: string | null };
    const key = snapshotCacheKey(1, "https://example.com/");
    writeSessionSnapshot<Snapshot>(
      "test:",
      key,
      { score: 92, detail: "heavy-detail-payload" },
      Date.now(),
      (snapshot) => ({ ...snapshot, detail: null }),
    );

    // The session tier only holds the slimmed copy, flagged partial.
    const raw = window.sessionStorage.getItem(`test:${key}`);
    expect(raw).not.toContain("heavy-detail-payload");
    const sessionEntry = parseSnapshotCacheEntry<Snapshot>(JSON.parse(raw!) as unknown);
    expect(sessionEntry?.snapshot).toEqual({ score: 92, detail: null });
    expect(sessionEntry?.partial).toBe(true);
  });

  it("restores slimmed session entries as partial so authoritative reads can refetch", () => {
    type Snapshot = { score: number; detail: string | null };
    const key = snapshotCacheKey(1, "https://example.com/");
    writeSessionSnapshot<Snapshot>(
      "test:",
      key,
      { score: 92, detail: "heavy-detail-payload" },
      Date.now(),
      (snapshot) => ({ ...snapshot, detail: null }),
    );

    const restored = readFreshSessionSnapshot<Snapshot>("test:", key);
    expect(restored?.snapshot).toEqual({ score: 92, detail: null });
    expect(restored?.partial).toBe(true);
  });
});
