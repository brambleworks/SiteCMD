import { describe, expect, it } from "vitest";

import type { PackageUpdate } from "@/lib/types";
import {
  buildUpdateQueueBreakdown,
  buildUpdateQueueSummary,
  countSecurityUpdates,
} from "./update-summary";

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
    workspaceMembers: [],
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    ...overrides,
  };
}

describe("update summary", () => {
  it("uses one shared policy for total, security, and regular update buckets", () => {
    const updates = [
      update({ name: "next", isSecurity: true, advisorySeverity: "high" }),
      update({ name: "vite", isSecurity: true, advisorySeverity: "critical" }),
      update({ name: "react", updateType: "major" }),
      update({ name: "typescript", updateType: "minor" }),
      update({ name: "eslint", updateType: "patch" }),
      update({ name: "prettier", updateType: "unknown" }),
    ];

    const summary = buildUpdateQueueSummary(updates);

    expect(summary.total).toBe(6);
    expect(summary.security).toBe(2);
    expect(summary.regular).toBe(4);
    expect(summary.major).toBe(1);
    expect(summary.minor).toBe(1);
    expect(summary.patch).toBe(2);
    expect(summary.securityUpdates.map((item) => item.name)).toEqual(["next", "vite"]);
    expect(summary.regularUpdates.map((item) => item.name)).toEqual([
      "react",
      "typescript",
      "eslint",
      "prettier",
    ]);
  });

  it("keeps legacy update event breakdowns aligned with the shared summary", () => {
    const updates = [
      update({ name: "next", isSecurity: true }),
      update({ name: "react", updateType: "major" }),
    ];

    expect(countSecurityUpdates(updates)).toBe(1);
    expect(buildUpdateQueueBreakdown(updates)).toEqual({
      critical: 1,
      major: 1,
      minor: 0,
      patch: 0,
    });
  });
});
