import { describe, expect, it } from "vitest";

import type { PackageUpdate } from "./types";
import {
  buildUpdateCampaignCopy,
  formatPackageUpdateSummary,
  getPackageUpdateTargetVersion,
} from "./update-priority";

function update(overrides: Partial<PackageUpdate> = {}): PackageUpdate {
  return {
    name: "lodash",
    currentVersion: "4.17.20",
    latestVersion: "4.17.21",
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

describe("update target copy", () => {
  it("uses only an OSV-verified release for security remediation", () => {
    const withoutFix = update({ isSecurity: true, advisorySeverity: "high" });
    const withFix = update({ ...withoutFix, advisoryFixedVersion: "4.17.21" });

    expect(getPackageUpdateTargetVersion(withoutFix)).toBeNull();
    expect(formatPackageUpdateSummary(withoutFix)).toBe(
      "lodash 4.17.20 (no fixed release) • security (high)",
    );
    expect(formatPackageUpdateSummary(withFix)).toBe("lodash 4.17.20 -> 4.17.21 • security (high)");
  });
});

describe("update-priority campaign copy", () => {
  it("builds fix-oriented copy for vulnerable dependency campaigns", () => {
    expect(
      buildUpdateCampaignCopy({
        totalCount: 3,
        securityCount: 2,
        leadLabel: "axios",
        leadSummary: "axios 1.6.0 -> 1.7.0 • security (critical)",
        leadSourceLabel: "package.json",
        mode: "fix",
      }),
    ).toEqual({
      title: "2 vulnerable packages and 1 other update still open",
      detail:
        "Start with axios 1.6.0 -> 1.7.0 • security (critical) in package.json; 2 more package updates are already listed in Updates.",
    });
  });

  it("builds verify-oriented copy for grouped package re-checks", () => {
    expect(
      buildUpdateCampaignCopy({
        totalCount: 3,
        leadLabel: "react 18.2.0 -> 19.0.0",
        leadSummary: "react 18.2.0 -> 19.0.0",
        mode: "verify",
      }),
    ).toEqual({
      title: "3 package updates still need verification",
      detail:
        "Start in Updates with react 18.2.0 -> 19.0.0; 2 more dependency changes still need a quick check.",
    });
  });

  it("builds resume-oriented copy for regressed package work", () => {
    expect(
      buildUpdateCampaignCopy({
        totalCount: 2,
        leadLabel: "next",
        leadSummary: "next 14.2.0 -> 15.0.0",
        mode: "resume",
      }),
    ).toEqual({
      title: "2 package updates came back",
      detail: "Start in Updates with next 14.2.0 -> 15.0.0; 1 more update also needs another look.",
    });
  });
});
