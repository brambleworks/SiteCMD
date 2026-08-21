import { describe, expect, it } from "vitest";
import type { PackageUpdate } from "@/lib/types";
import { buildUpdateAgentIssue } from "./update-dossier-model";

function makeUpdate(overrides: Partial<PackageUpdate> = {}): PackageUpdate {
  return {
    name: "lodash",
    currentVersion: "4.17.20",
    latestVersion: "4.17.21",
    ecosystem: "npm",
    updateType: "patch",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "package.json",
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

describe("buildUpdateAgentIssue", () => {
  it("returns null for minor and patch updates, which never become work items", () => {
    expect(buildUpdateAgentIssue(makeUpdate({ updateType: "patch" }), [])).toBeNull();
    expect(buildUpdateAgentIssue(makeUpdate({ updateType: "minor" }), [])).toBeNull();
    expect(buildUpdateAgentIssue(makeUpdate({ updateType: "unknown" }), [])).toBeNull();
  });

  it("maps a security update to the shared vulnerability group check_id", () => {
    const update = makeUpdate({
      isSecurity: true,
      advisorySeverity: "high",
      advisoryFixedVersion: "4.17.21",
    });
    const issue = buildUpdateAgentIssue(update, [update]);

    expect(issue).not.toBeNull();
    expect(issue!.checkId).toBe("dependencies.vulnerability");
    expect(issue!.title).toBe("Vulnerability in lodash 4.17.20 (npm)");
    expect(issue!.severity).toBe("high");
    expect(issue!.manualFix).toBe("npm install lodash@4.17.21");
  });

  it("covers every vulnerable package in the brief, not just the selected one", () => {
    const selected = makeUpdate({
      isSecurity: true,
      advisorySeverity: "high",
      advisoryFixedVersion: "4.17.21",
    });
    const otherVulnerable = makeUpdate({
      name: "minimist",
      currentVersion: "1.2.5",
      latestVersion: "1.2.8",
      isSecurity: true,
      advisorySeverity: "critical",
      advisoryFixedVersion: "1.2.8",
    });
    const unrelatedMajor = makeUpdate({ name: "react", updateType: "major" });
    const issue = buildUpdateAgentIssue(selected, [selected, otherVulnerable, unrelatedMajor]);

    expect(issue!.title).toBe("Vulnerabilities in 2 dependencies");
    expect(issue!.description).toContain("- lodash 4.17.20 -> 4.17.21 (npm)");
    expect(issue!.description).toContain("- minimist 1.2.5 -> 1.2.8 (npm)");
    expect(issue!.description).not.toContain("react");
    expect(issue!.description).toContain("verifies this issue as a group");
    // Highest advisory severity across the group wins.
    expect(issue!.severity).toBe("critical");
    expect(issue!.manualFix).toBe("npm install lodash@4.17.21\nnpm install minimist@1.2.8");
    expect(issue!.evidence).toEqual([
      expect.objectContaining({ package: "lodash", advisory_severity: "high" }),
      expect.objectContaining({ package: "minimist", advisory_severity: "critical" }),
    ]);
  });

  it("keeps no-fix advisories in the shared verification group", () => {
    const selected = makeUpdate({ isSecurity: true, advisoryFixedVersion: "4.17.21" });
    const withoutFix = makeUpdate({
      name: "minimist",
      currentVersion: "1.2.5",
      latestVersion: "1.2.8",
      isSecurity: true,
    });

    const issue = buildUpdateAgentIssue(selected, [selected, withoutFix]);

    expect(issue!.title).toBe("Vulnerabilities in 2 dependencies");
    expect(issue!.description).toContain("minimist 1.2.5 (no fixed release)");
    expect(issue!.manualFix).toContain("npm install lodash@4.17.21");
    expect(issue!.manualFix).not.toContain("npm install minimist");
    expect(issue!.manualFix).toContain("For minimist, determine reachability");
  });

  it("defaults a missing or unrecognized advisory severity to high", () => {
    const update = makeUpdate({
      isSecurity: true,
      advisorySeverity: null,
      advisoryFixedVersion: "4.17.21",
    });
    expect(buildUpdateAgentIssue(update, [update])!.severity).toBe("high");

    const odd = makeUpdate({
      isSecurity: true,
      advisorySeverity: "weird-scale",
      advisoryFixedVersion: "4.17.21",
    });
    expect(buildUpdateAgentIssue(odd, [odd])!.severity).toBe("high");
  });

  it("maps a major update to the outdated-major group and excludes security majors", () => {
    const selected = makeUpdate({ name: "react", updateType: "major", latestVersion: "19.0.0" });
    const securityMajor = makeUpdate({
      name: "express",
      updateType: "major",
      isSecurity: true,
    });
    const issue = buildUpdateAgentIssue(selected, [selected, securityMajor]);

    expect(issue!.checkId).toBe("dependencies.outdated-major");
    expect(issue!.title).toBe("react has a major update (4.17.20 -> 19.0.0)");
    expect(issue!.severity).toBe("low");
    expect(issue!.description).not.toContain("express");
  });

  it("includes the selected package even when allUpdates omits it", () => {
    const selected = makeUpdate({ isSecurity: true, advisoryFixedVersion: "4.17.21" });
    const issue = buildUpdateAgentIssue(selected, []);

    expect(issue!.description).toContain("- lodash 4.17.20 -> 4.17.21 (npm)");
  });

  it("builds mitigation guidance for an advisory without a fixed release", () => {
    const selected = makeUpdate({ isSecurity: true, advisorySeverity: "critical" });
    const issue = buildUpdateAgentIssue(selected, [selected]);

    expect(issue).not.toBeNull();
    expect(issue!.description).toContain("lodash 4.17.20 (no fixed release)");
    expect(issue!.manualFix).toContain("remove, replace, or isolate");
    expect(issue!.manualFix).not.toContain("npm install");
    expect(issue!.evidence).toEqual([expect.objectContaining({ advisory_fixed_version: null })]);
  });
});
