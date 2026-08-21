import { describe, expect, it } from "vitest";
import {
  filterActiveCodeIssues,
  filterActiveWebIssues,
  INACTIVE_ISSUE_STATUSES,
  isInactiveIssueStatus,
} from "./active-issue-filter";

describe("isInactiveIssueStatus", () => {
  it("matches exactly the statuses the score excludes (blocked, ignored, verified)", () => {
    expect(INACTIVE_ISSUE_STATUSES).toEqual(["blocked", "ignored", "verified"]);
    expect(isInactiveIssueStatus("blocked")).toBe(true);
    expect(isInactiveIssueStatus("ignored")).toBe(true);
    expect(isInactiveIssueStatus("verified")).toBe(true);
    expect(isInactiveIssueStatus("new")).toBe(false);
    expect(isInactiveIssueStatus("snoozed")).toBe(false);
    expect(isInactiveIssueStatus("working")).toBe(false);
  });
});

describe("filterActiveWebIssues", () => {
  it("drops web issues whose check_id is inactive (e.g. blocked)", () => {
    const issues = [{ checkId: "seo.title" }, { checkId: "security.csp" }];
    const inactive = new Set(["security.csp"]);

    const result = filterActiveWebIssues(issues, inactive);

    expect(result.map((i) => i.checkId)).toEqual(["seo.title"]);
  });

  it("returns all issues when nothing is inactive", () => {
    const issues = [{ checkId: "seo.title" }];
    expect(filterActiveWebIssues(issues, new Set())).toEqual(issues);
  });

  it("returns the input array identity when the filter removes nothing", () => {
    // Memoized consumers key on array identity; a fresh copy on every no-op
    // refresh would cascade into rankUnified recomputes.
    const issues = [{ checkId: "seo.title" }, { checkId: "security.csp" }];
    expect(filterActiveWebIssues(issues, new Set())).toBe(issues);
    expect(filterActiveWebIssues(issues, new Set(["not-present"]))).toBe(issues);
  });
});

describe("filterActiveCodeIssues", () => {
  it("drops the blocked code issue (the reported preact case) by checkId and keeps the rest", () => {
    const issues = [
      { checkId: "code_scan.dep-typosquat-preact" },
      { checkId: "code_scan.sql-injection-1" },
    ];
    const inactive = new Set(["code_scan.dep-typosquat-preact"]);

    const result = filterActiveCodeIssues(issues, inactive);

    expect(result.map((i) => i.checkId)).toEqual(["code_scan.sql-injection-1"]);
  });

  it("returns all issues when nothing is inactive", () => {
    const issues = [{ checkId: "code_scan.x" }];
    expect(filterActiveCodeIssues(issues, new Set())).toEqual(issues);
  });

  it("returns the input array identity when the filter removes nothing", () => {
    const issues = [{ checkId: "code_scan.x" }, { checkId: "code_scan.y" }];
    expect(filterActiveCodeIssues(issues, new Set())).toBe(issues);
    expect(filterActiveCodeIssues(issues, new Set(["not-present"]))).toBe(issues);
  });
});
