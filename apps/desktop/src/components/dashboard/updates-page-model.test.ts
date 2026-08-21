import { describe, expect, it } from "vitest";

import type { PackageUpdate, UpdateReport } from "@/lib/types";
import { buildUpdateDisplayModel, formatWorkspaceMembers } from "./updates-page-model";

function update(overrides: Partial<PackageUpdate> = {}): PackageUpdate {
  return {
    name: "package",
    currentVersion: "1.0.0",
    latestVersion: "2.0.0",
    ecosystem: "npm",
    updateType: "patch",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "package-lock.json",
    isDev: false,
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    workspaceMembers: [],
    ...overrides,
  };
}

function report(updates: PackageUpdate[]): UpdateReport {
  return {
    packages: [],
    updates,
    ecosystemsDetected: ["npm"],
    scanDurationMs: 10,
  };
}

describe("buildUpdateDisplayModel", () => {
  it("uses the shared total update count instead of a regular-only count", () => {
    const model = buildUpdateDisplayModel(
      report([
        update({
          name: "next",
          isSecurity: true,
          advisorySeverity: "high",
          advisoryFixedVersion: "2.0.0",
        }),
        update({ name: "react", updateType: "major" }),
      ]),
      "all",
    );

    expect(model.totalCount).toBe(2);
    expect(model.securityUpdates).toHaveLength(1);
    expect(model.regularUpdates).toHaveLength(1);
    expect(model.majors).toHaveLength(1);
  });
});

describe("buildUpdateDisplayModel copyable updates", () => {
  it('includes security updates so "Copy All Commands" means all', () => {
    // `regularUpdates` deliberately excludes security updates, so sourcing the
    // copy-all button from it silently skipped the most urgent ones.
    const model = buildUpdateDisplayModel(
      report([
        update({
          name: "next",
          isSecurity: true,
          advisorySeverity: "high",
          advisoryFixedVersion: "2.0.0",
        }),
        update({ name: "react", updateType: "major" }),
      ]),
      "all",
    );

    expect(model.copyableUpdates.map((entry) => entry.name)).toEqual(["next", "react"]);
  });

  it("skips advisories that have no released fix", () => {
    const model = buildUpdateDisplayModel(
      report([
        update({ name: "next", isSecurity: true, advisoryFixedVersion: undefined }),
        update({ name: "react", updateType: "major" }),
      ]),
      "all",
    );

    // There is no command to copy for a package with no fix published.
    expect(model.copyableUpdates.map((entry) => entry.name)).toEqual(["react"]);
  });
});

describe("formatWorkspaceMembers", () => {
  it("returns null outside a workspace so non-monorepo rows stay unchanged", () => {
    expect(formatWorkspaceMembers([])).toBeNull();
  });

  it("renders the pnpm root key as a word a reader recognises", () => {
    expect(formatWorkspaceMembers(["."])).toBe("root");
  });

  it("lists every member, because the upgrade applies in each", () => {
    expect(formatWorkspaceMembers(["apps/mcp-server", "apps/example-worker"])).toBe(
      "apps/mcp-server, apps/example-worker",
    );
  });

  it("collapses a long member list instead of overflowing the row", () => {
    expect(formatWorkspaceMembers(["a", "b", "c", "d", "e"])).toBe("a, b, c +2 more");
  });
});
