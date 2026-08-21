import { beforeEach, describe, expect, it, vi } from "vitest";

const storeMock = vi.hoisted(() => {
  const durableValues = new Map<string, unknown>();

  return {
    durableValues,
    storeGet: vi.fn(async (key: string, fallback: unknown) =>
      durableValues.has(key) ? durableValues.get(key) : fallback,
    ),
    storeSet: vi.fn(async (key: string, value: unknown) => {
      durableValues.set(key, value);
    }),
    migrateFromLocalStorage: vi.fn(
      async (_localStorageKey: string, storeKey: string, fallback: unknown) =>
        durableValues.has(storeKey) ? durableValues.get(storeKey) : fallback,
    ),
  };
});

vi.mock("@/lib/store", () => ({
  storeGet: storeMock.storeGet,
  storeSet: storeMock.storeSet,
  migrateFromLocalStorage: storeMock.migrateFromLocalStorage,
}));

async function flushHydration() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("durable memory hydration", () => {
  beforeEach(() => {
    vi.resetModules();
    window.localStorage.clear();
    storeMock.durableValues.clear();
    storeMock.storeGet.mockClear();
    storeMock.storeSet.mockClear();
    storeMock.migrateFromLocalStorage.mockClear();
  });

  it("hydrates update memory from Tauri Store when localStorage is empty", async () => {
    storeMock.durableValues.set("update-memory", {
      "/tmp/app::npm::react": {
        key: "/tmp/app::npm::react",
        firstSeenAt: 1_000,
        lastSeenAt: 2_000,
        lastPendingAt: 2_000,
        lastVerifiedAt: null,
        regressedAfterVerifiedAt: null,
        currentVersion: "18.2.0",
        latestVersion: "19.0.0",
        updateType: "major",
        isSecurity: false,
      },
    });

    const { getUpdateMemory } = await import("./update-memory");
    await flushHydration();

    expect(getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "react" })).toMatchObject({
      currentVersion: "18.2.0",
      latestVersion: "19.0.0",
      updateType: "major",
    });
    expect(window.localStorage.getItem("sitecmd_update_memory_v1")).toBeNull();
  });

  it("merges early update writes with durable memory that hydrates after startup", async () => {
    storeMock.durableValues.set("update-memory", {
      "/tmp/app::npm::react": {
        key: "/tmp/app::npm::react",
        firstSeenAt: 1_000,
        lastSeenAt: 2_000,
        lastPendingAt: 2_000,
        lastVerifiedAt: 3_000,
        regressedAfterVerifiedAt: null,
        currentVersion: "18.2.0",
        latestVersion: "19.0.0",
        updateType: "major",
        isSecurity: false,
      },
    });

    const { getUpdateMemory, recordSeenUpdates } = await import("./update-memory");
    recordSeenUpdates("/tmp/app", [
      {
        ecosystem: "npm",
        name: "react",
        currentVersion: "18.2.0",
        latestVersion: "19.1.0",
        updateType: "major",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "npm",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
    ]);
    await flushHydration();

    const entry = getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "react" });
    expect(entry?.firstSeenAt).toBe(1_000);
    expect(entry?.lastVerifiedAt).toBe(3_000);
    expect(entry?.latestVersion).toBe("19.1.0");

    const localFallback = JSON.parse(
      window.localStorage.getItem("sitecmd_update_memory_v1") ?? "{}",
    );
    expect(localFallback["/tmp/app::npm::react"]).toMatchObject({
      firstSeenAt: 1_000,
      lastVerifiedAt: 3_000,
      latestVersion: "19.1.0",
    });
  });

  it("hydrates update snapshots from Tauri Store when localStorage is empty", async () => {
    storeMock.durableValues.set("update-snapshots", {
      "/tmp/app": {
        updatedAt: 1_000,
        updates: [
          {
            ecosystem: "npm",
            name: "zod",
            current_version: "3.25.0",
            latest_version: "4.0.0",
            update_type: "major",
            is_security: false,
          },
        ],
      },
    });

    const { readUpdateSnapshot } = await import("./update-memory");
    await flushHydration();

    expect(readUpdateSnapshot("/tmp/app")).toEqual([
      expect.objectContaining({
        name: "zod",
        latest_version: "4.0.0",
      }),
    ]);
  });

  it("keeps the localStorage fallback in sync when update snapshots merge after early writes", async () => {
    storeMock.durableValues.set("update-snapshots", {
      "/tmp/other": {
        updatedAt: 1_000,
        updates: [
          {
            ecosystem: "npm",
            name: "zod",
            current_version: "3.25.0",
            latest_version: "4.0.0",
            update_type: "major",
            is_security: false,
          },
        ],
      },
    });

    const { readUpdateSnapshot, writeUpdateSnapshot } = await import("./update-memory");
    writeUpdateSnapshot("/tmp/app", [
      {
        ecosystem: "npm",
        name: "react",
        currentVersion: "18.2.0",
        latestVersion: "19.1.0",
        updateType: "major",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "npm",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
    ]);
    await flushHydration();

    expect(readUpdateSnapshot("/tmp/other")).toEqual([expect.objectContaining({ name: "zod" })]);
    expect(readUpdateSnapshot("/tmp/app")).toEqual([expect.objectContaining({ name: "react" })]);
    const localFallback = JSON.parse(
      window.localStorage.getItem("sitecmd_update_snapshots_v1") ?? "{}",
    );
    expect(localFallback["/tmp/other"].updates[0].name).toBe("zod");
    expect(localFallback["/tmp/app"].updates[0].name).toBe("react");
  });
});
