import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import { createDurableEntryStore, maxNullableNumber } from "./durable-entry-store";

interface Row {
  key: string;
  seenAt: number;
}

const parseRow = (value: unknown): Row | null => {
  if (typeof value !== "object" || value === null) return null;
  const v = value as Record<string, unknown>;
  return typeof v.key === "string" && typeof v.seenAt === "number"
    ? { key: v.key, seenAt: v.seenAt }
    : null;
};

function newStore(max = 3) {
  return createDurableEntryStore<Row>({
    storageKey: "durable-test",
    storeKey: "durable-test",
    max,
    parseEntry: parseRow,
    mergeEntry: (durable, current) => ({
      ...durable,
      ...current,
      seenAt: Math.max(durable.seenAt, current.seenAt),
    }),
    recencyOf: (row) => row.seenAt,
  });
}

describe("maxNullableNumber", () => {
  it("keeps the larger value, treating null as absent", () => {
    expect(maxNullableNumber(null, 5)).toBe(5);
    expect(maxNullableNumber(5, null)).toBe(5);
    expect(maxNullableNumber(null, null)).toBeNull();
    expect(maxNullableNumber(3, 7)).toBe(7);
  });
});

describe("createDurableEntryStore", () => {
  beforeEach(() => localStorage.clear());

  it("persists and loads entries", () => {
    const store = newStore();
    store.persist({ a: { key: "a", seenAt: 1 } });
    expect(store.load()).toEqual({ a: { key: "a", seenAt: 1 } });
  });

  it("caps to the newest `max` entries by recency", () => {
    const store = newStore(2);
    store.persist({
      a: { key: "a", seenAt: 1 },
      b: { key: "b", seenAt: 3 },
      c: { key: "c", seenAt: 2 },
    });
    // The oldest (a) is dropped; the two newest survive.
    expect(Object.keys(store.load()).sort()).toEqual(["b", "c"]);
  });

  it("reads back through localStorage after a cache reset", () => {
    const store = newStore();
    store.persist({ a: { key: "a", seenAt: 1 } });
    store.reset();
    expect(store.load()).toEqual({ a: { key: "a", seenAt: 1 } });
  });
});
