import { describe, expect, it } from "vitest";

import { normalizePackageUpdate, normalizeUpdateReport } from "./package-update-normalize";

/** A snapshot row as an older build wrote it: no `workspaceMembers` key. */
function legacyUpdateJson(overrides: Record<string, unknown> = {}) {
  return {
    name: "react",
    currentVersion: "19.2.7",
    latestVersion: "19.2.8",
    ecosystem: "npm",
    updateType: "patch",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "pnpm-lock.yaml",
    isDev: false,
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    ...overrides,
  };
}

describe("normalizePackageUpdate", () => {
  it("fills in fields a persisted row predates", () => {
    const update = normalizePackageUpdate(legacyUpdateJson());
    expect(update?.workspaceMembers).toEqual([]);
    expect(update?.advisoryFixedVersion).toBeUndefined();
  });

  it("keeps verified advisory remediation separate from the registry version", () => {
    const update = normalizePackageUpdate(
      legacyUpdateJson({
        isSecurity: true,
        latestVersion: "5.0.0",
        advisoryFixedVersion: "4.17.21",
      }),
    );
    expect(update?.latestVersion).toBe("5.0.0");
    expect(update?.advisoryFixedVersion).toBe("4.17.21");
  });

  it("retires the legacy no-fix sentinel without inventing remediation", () => {
    const update = normalizePackageUpdate(
      legacyUpdateJson({
        isSecurity: true,
        currentVersion: "4.17.20",
        latestVersion: "no fix available",
      }),
    );
    expect(update?.latestVersion).toBe("4.17.20");
    expect(update?.advisoryFixedVersion).toBeUndefined();
  });

  it("keeps member attribution when the row has it", () => {
    const update = normalizePackageUpdate(
      legacyUpdateJson({ workspaceMembers: ["apps/mcp-server", "apps/example-worker"] }),
    );
    expect(update?.workspaceMembers).toEqual(["apps/mcp-server", "apps/example-worker"]);
  });

  it("drops non-string members rather than rendering them", () => {
    const update = normalizePackageUpdate(
      legacyUpdateJson({ workspaceMembers: ["apps/desktop", 7, null, { a: 1 }] }),
    );
    expect(update?.workspaceMembers).toEqual(["apps/desktop"]);
  });

  it("treats a non-array members value as no members", () => {
    const update = normalizePackageUpdate(legacyUpdateJson({ workspaceMembers: "apps/desktop" }));
    expect(update?.workspaceMembers).toEqual([]);
  });

  it("rejects a row with no identity, so nothing renders half-blank", () => {
    expect(normalizePackageUpdate(legacyUpdateJson({ name: undefined }))).toBeNull();
    expect(normalizePackageUpdate(legacyUpdateJson({ latestVersion: 42 }))).toBeNull();
    expect(normalizePackageUpdate(null)).toBeNull();
    expect(normalizePackageUpdate("react")).toBeNull();
  });

  it("defaults optional metadata instead of leaking undefined", () => {
    const update = normalizePackageUpdate({
      name: "react",
      currentVersion: "1.0.0",
      latestVersion: "2.0.0",
      ecosystem: "npm",
      updateType: "major",
      isSecurity: false,
    });
    expect(update).toMatchObject({
      source: "unknown",
      isDev: false,
      isDeprecated: false,
      deprecationMessage: null,
      currentVersionDeprecated: false,
      isStale: false,
      lastPublished: null,
      workspaceMembers: [],
    });
  });
});

describe("normalizeUpdateReport", () => {
  it("normalizes installed packages as well as updates", () => {
    const report = normalizeUpdateReport({
      packages: [{ name: "react", version: "19.2.7", ecosystem: "npm", isDev: false }],
      updates: [legacyUpdateJson()],
      ecosystemsDetected: ["npm"],
      scanDurationMs: 120,
    });
    expect(report.packages[0].workspaceMembers).toEqual([]);
    expect(report.updates[0].workspaceMembers).toEqual([]);
    expect(report.scanDurationMs).toBe(120);
  });

  it("survives a partial payload instead of throwing downstream", () => {
    const report = normalizeUpdateReport({ updates: null, packages: undefined });
    expect(report.packages).toEqual([]);
    expect(report.updates).toEqual([]);
    expect(report.ecosystemsDetected).toEqual([]);
    expect(report.scanDurationMs).toBe(0);
  });

  it("survives a non-object payload", () => {
    expect(normalizeUpdateReport(null).updates).toEqual([]);
    expect(normalizeUpdateReport("nope").packages).toEqual([]);
  });

  it("drops unusable rows but keeps the good ones", () => {
    const report = normalizeUpdateReport({
      updates: [legacyUpdateJson(), { name: "broken" }, null],
    });
    expect(report.updates.map((update) => update.name)).toEqual(["react"]);
  });

  it("does not invent packages, so an empty scan stays observably empty", () => {
    // packages.length is what tells the page a scan actually observed
    // dependencies; inventing entries here would read as "all up to date".
    expect(normalizeUpdateReport({ updates: [legacyUpdateJson()] }).packages).toEqual([]);
  });
});
