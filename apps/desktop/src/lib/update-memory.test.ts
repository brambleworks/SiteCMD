import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import {
  __resetUpdateMemoryForTests,
  buildUpdateMemoryKey,
  getRecentPendingProjectUpdates,
  getUpdateMemory,
  markUpdateVerified,
  readUpdateSnapshot,
  recordSeenUpdates,
  writeUpdateSnapshot,
} from "./update-memory";
import type { PackageUpdate } from "./types";

function update(overrides: Partial<PackageUpdate> = {}): PackageUpdate {
  return {
    ecosystem: "npm",
    name: "react",
    currentVersion: "18.2.0",
    latestVersion: "19.0.0",
    updateType: "major",
    isSecurity: false,
    ...overrides,
  } as PackageUpdate;
}

describe("buildUpdateMemoryKey", () => {
  it("joins path, ecosystem, and name", () => {
    expect(buildUpdateMemoryKey("/tmp/app", update({ ecosystem: "npm", name: "react" }))).toBe(
      "/tmp/app::npm::react",
    );
  });

  it("keeps project paths disjoint", () => {
    const a = buildUpdateMemoryKey("/tmp/a", update());
    const b = buildUpdateMemoryKey("/tmp/b", update());
    expect(a).not.toBe(b);
  });

  it("keeps ecosystems disjoint (npm react vs python react)", () => {
    const npm = buildUpdateMemoryKey("/tmp/app", update({ ecosystem: "npm", name: "react" }));
    const python = buildUpdateMemoryKey("/tmp/app", update({ ecosystem: "python", name: "react" }));
    expect(npm).not.toBe(python);
  });
});

describe("update-memory lifecycle", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __resetUpdateMemoryForTests();
  });

  it("recordSeenUpdates makes getUpdateMemory return a populated entry", () => {
    recordSeenUpdates("/tmp/app", [update({ name: "lodash" })]);
    const entry = getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "lodash" });
    expect(entry).not.toBeNull();
    expect(entry?.currentVersion).toBe("18.2.0");
    expect(entry?.latestVersion).toBe("19.0.0");
    expect(entry?.updateType).toBe("major");
    expect(entry?.lastPendingAt).not.toBeNull();
    expect(entry?.lastVerifiedAt).toBeNull();
  });

  it("returns null for an unknown (project, ecosystem, name) tuple", () => {
    expect(getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "never-seen" })).toBeNull();
  });

  it("markUpdateVerified stamps lastVerifiedAt while preserving firstSeenAt", () => {
    recordSeenUpdates("/tmp/app", [update({ name: "next" })]);
    const before = getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "next" });
    markUpdateVerified("/tmp/app", update({ name: "next" }));
    const after = getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "next" });
    expect(after?.lastVerifiedAt).not.toBeNull();
    expect(after?.firstSeenAt).toBe(before?.firstSeenAt);
  });

  it("recording the update again after verification flags a regression", () => {
    let now = 1_000;
    const spy = vi.spyOn(Date, "now").mockImplementation(() => now);

    recordSeenUpdates("/tmp/app", [update({ name: "zod" })]);
    now += 1_000;
    markUpdateVerified("/tmp/app", update({ name: "zod" }));
    const afterVerify = getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "zod" });
    expect(afterVerify?.regressedAfterVerifiedAt).toBeNull();

    now += 1_000;
    recordSeenUpdates("/tmp/app", [update({ name: "zod" })]);
    const afterRegress = getUpdateMemory("/tmp/app", { ecosystem: "npm", name: "zod" });
    expect(afterRegress?.regressedAfterVerifiedAt).not.toBeNull();

    spy.mockRestore();
  });

  it("separate projects keep separate memory entries", () => {
    recordSeenUpdates("/tmp/app-a", [update({ name: "shared" })]);
    recordSeenUpdates("/tmp/app-b", [update({ name: "shared" })]);
    markUpdateVerified("/tmp/app-a", update({ name: "shared" }));

    const a = getUpdateMemory("/tmp/app-a", { ecosystem: "npm", name: "shared" });
    const b = getUpdateMemory("/tmp/app-b", { ecosystem: "npm", name: "shared" });
    expect(a?.lastVerifiedAt).not.toBeNull();
    expect(b?.lastVerifiedAt).toBeNull();
  });

  it("persists and reloads the last known project update snapshot", () => {
    const updates = [
      update({
        name: "@tailwindcss/vite",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
      }),
      update({
        name: "tailwindcss",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
      }),
    ];

    writeUpdateSnapshot("/tmp/app", updates);

    expect(readUpdateSnapshot("/tmp/app")).toEqual(updates);
    expect(readUpdateSnapshot("/tmp/other-app")).toBeNull();
  });

  it("reconstructs recent pending project updates from memory for follow-up history", () => {
    let now = 10_000;
    const spy = vi.spyOn(Date, "now").mockImplementation(() => now);

    recordSeenUpdates("/tmp/app", [
      update({
        name: "@tailwindcss/vite",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
      }),
      update({
        name: "tailwindcss",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
      }),
    ]);

    now += 1_000;
    recordSeenUpdates("/tmp/other-app", [update({ name: "react" })]);

    const reconstructed = getRecentPendingProjectUpdates("/tmp/app");

    expect(reconstructed).toHaveLength(2);
    expect(reconstructed.map((candidate) => candidate.name).sort()).toEqual([
      "@tailwindcss/vite",
      "tailwindcss",
    ]);

    spy.mockRestore();
  });

  it("preserves a verified security target when reconstructing pending updates", () => {
    recordSeenUpdates("/tmp/app", [
      update({
        name: "lodash",
        currentVersion: "4.17.20",
        latestVersion: "4.17.21",
        updateType: "patch",
        isSecurity: true,
        advisoryFixedVersion: "4.17.21",
      }),
    ]);

    expect(getRecentPendingProjectUpdates("/tmp/app")[0]?.advisoryFixedVersion).toBe("4.17.21");
  });

  it("does not reconstruct recently verified updates as still pending", () => {
    let now = 100_000;
    const spy = vi.spyOn(Date, "now").mockImplementation(() => now);

    recordSeenUpdates("/tmp/app", [update({ name: "zod" })]);
    now += 1_000;
    markUpdateVerified("/tmp/app", update({ name: "zod" }));

    expect(getRecentPendingProjectUpdates("/tmp/app")).toEqual([]);

    spy.mockRestore();
  });
});
