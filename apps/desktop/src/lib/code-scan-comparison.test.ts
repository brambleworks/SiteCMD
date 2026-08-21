import { describe, expect, it } from "vitest";
import {
  buildSummaryOnlyCodeScanResult,
  computeCodeScanComparison,
  getCodeScanDomainSummaries,
  getPreviousCodeScanSummary,
  sortCodeIssues,
  summarizeCodeIssueCounts,
} from "./code-scan-comparison";
import type { CodeIssue, CodeScanResult, CodeScanSummary } from "./types";
import { getCodeIssueDomain, type ClassifiableCodeIssue } from "@/lib/code-scan-domains";

function issue(overrides: Partial<CodeIssue> = {}): CodeIssue {
  const base: ClassifiableCodeIssue = {
    id: "test-issue",
    checkId: "code_scan.test-issue",
    category: "ai-safety",
    severity: "high",
    title: "Test issue",
    description: "",
    relativePath: "src/foo.ts",
    absolutePath: "/tmp/src/foo.ts",
    line: null,
    sourceExcerpt: null,
    evidence: null,
    whyNow: null,
    likelyFix: null,
    confidence: "high",
    verifyHint: null,
    ...overrides,
  };
  return { ...base, domain: getCodeIssueDomain(base) };
}

function result(overrides: Partial<CodeScanResult> = {}): CodeScanResult {
  return {
    id: 1,
    projectId: 1,
    environmentUrl: "https://example.com",
    overallScore: 80,
    issueCount: 0,
    criticalCount: 0,
    highCount: 0,
    mediumCount: 0,
    lowCount: 0,
    durationMs: 100,
    checkedAt: "2026-04-10T00:00:00Z",
    framework: null,
    domainSummaries: [],
    issues: [],
    ...overrides,
  };
}

function summary(overrides: Partial<CodeScanSummary> = {}): CodeScanSummary {
  return {
    id: 1,
    projectId: 1,
    environmentUrl: "https://example.com",
    overallScore: 80,
    issueCount: 0,
    groupedIssueCount: 0,
    criticalCount: 0,
    highCount: 0,
    durationMs: 100,
    checkedAt: "2026-04-10T00:00:00Z",
    framework: null,
    topDomain: null,
    topDomainCount: 0,
    domainSummaries: [],
    ...overrides,
  };
}

describe("summarizeCodeIssueCounts", () => {
  it("returns zero counts for an empty list", () => {
    expect(summarizeCodeIssueCounts([])).toEqual({ critical: 0, high: 0, medium: 0, low: 0 });
  });

  it("buckets issues by severity", () => {
    const issues = [
      issue({ severity: "critical" }),
      issue({ severity: "critical" }),
      issue({ severity: "high" }),
      issue({ severity: "medium" }),
      issue({ severity: "low" }),
      issue({ severity: "low" }),
      issue({ severity: "low" }),
    ];
    expect(summarizeCodeIssueCounts(issues)).toEqual({
      critical: 2,
      high: 1,
      medium: 1,
      low: 3,
    });
  });
});

describe("sortCodeIssues", () => {
  it("sorts critical before high before medium before low", () => {
    const issues = [
      issue({ id: "a", severity: "low", title: "A" }),
      issue({ id: "b", severity: "critical", title: "B" }),
      issue({ id: "c", severity: "medium", title: "C" }),
      issue({ id: "d", severity: "high", title: "D" }),
    ];
    issues.sort(sortCodeIssues);
    expect(issues.map((i) => i.id)).toEqual(["b", "d", "c", "a"]);
  });

  it("tiebreaks by title when severity matches", () => {
    const issues = [
      issue({ id: "a", severity: "high", title: "Zebra" }),
      issue({ id: "b", severity: "high", title: "Apple" }),
      issue({ id: "c", severity: "high", title: "Mango" }),
    ];
    issues.sort(sortCodeIssues);
    expect(issues.map((i) => i.title)).toEqual(["Apple", "Mango", "Zebra"]);
  });
});

describe("getPreviousCodeScanSummary", () => {
  it("returns null when history is empty", () => {
    expect(getPreviousCodeScanSummary(result(), [])).toBeNull();
  });

  it("prefers the most recent scan with the same environment URL", () => {
    const current = result({ id: 3, environmentUrl: "https://example.com" });
    const history = [
      summary({ id: 3, environmentUrl: "https://example.com" }),
      summary({ id: 2, environmentUrl: "https://staging.example.com" }),
      summary({ id: 1, environmentUrl: "https://example.com" }),
    ];
    const prev = getPreviousCodeScanSummary(current, history);
    expect(prev?.id).toBe(1);
  });

  it("falls back to any previous scan when no same-target exists", () => {
    const current = result({ id: 3, environmentUrl: "https://example.com" });
    const history = [
      summary({ id: 3, environmentUrl: "https://example.com" }),
      summary({ id: 2, environmentUrl: "https://staging.example.com" }),
    ];
    const prev = getPreviousCodeScanSummary(current, history);
    expect(prev?.id).toBe(2);
  });

  it("normalizes trailing slashes when matching URLs", () => {
    const current = result({ id: 3, environmentUrl: "https://example.com/" });
    const history = [
      summary({ id: 3, environmentUrl: "https://example.com/" }),
      summary({ id: 2, environmentUrl: "https://example.com" }),
    ];
    const prev = getPreviousCodeScanSummary(current, history);
    expect(prev?.id).toBe(2);
  });

  it("skips only the exact id when the current scan is not in history", () => {
    const current = result({ id: 99, environmentUrl: "https://example.com" });
    const history = [
      summary({ id: 5, environmentUrl: "https://example.com" }),
      summary({ id: 4, environmentUrl: "https://example.com" }),
    ];
    const prev = getPreviousCodeScanSummary(current, history);
    expect(prev?.id).toBe(5);
  });
});

describe("computeCodeScanComparison", () => {
  it("computes score / issue / severity deltas", () => {
    const before = result({
      overallScore: 70,
      issueCount: 10,
      criticalCount: 2,
      highCount: 3,
      mediumCount: 3,
      lowCount: 2,
    });
    const after = result({
      overallScore: 85,
      issueCount: 4,
      criticalCount: 0,
      highCount: 1,
      mediumCount: 2,
      lowCount: 1,
    });
    const diff = computeCodeScanComparison(before, after);

    expect(diff.scoreDelta).toBe(15);
    expect(diff.issueDelta).toBe(-6);
    expect(diff.criticalDelta).toBe(-2);
    expect(diff.highDelta).toBe(-2);
    expect(diff.mediumDelta).toBe(-1);
    expect(diff.lowDelta).toBe(-1);
  });

  it("classifies issues as fixed / new / changed / unchanged by id", () => {
    const before = result({
      issues: [
        issue({ id: "fix-me", title: "Fix me", severity: "high" }),
        issue({ id: "keep", title: "Keep", severity: "medium" }),
        issue({ id: "severity-changed", title: "Change", severity: "high" }),
      ],
    });
    const after = result({
      issues: [
        issue({ id: "keep", title: "Keep", severity: "medium" }),
        issue({ id: "severity-changed", title: "Change", severity: "critical" }),
        issue({ id: "brand-new", title: "New", severity: "low" }),
      ],
    });
    const diff = computeCodeScanComparison(before, after);

    expect(diff.fixed.map((i) => i.issueId)).toEqual(["fix-me"]);
    expect(diff.newIssues.map((i) => i.issueId)).toEqual(["brand-new"]);
    expect(diff.changed.map((i) => i.issueId)).toEqual(["severity-changed"]);
    expect(diff.unchangedCount).toBe(1);
  });
});

describe("getCodeScanDomainSummaries", () => {
  it("returns the stored domainSummaries when present", () => {
    const summary = {
      domain: "ai-safety" as const,
      issueCount: 5,
      criticalCount: 1,
      highCount: 2,
      mediumCount: 1,
      lowCount: 1,
    };
    const summaries = getCodeScanDomainSummaries({ issues: [], domainSummaries: [summary] });
    expect(summaries).toEqual([summary]);
  });

  it("falls back to computing from issues when summaries are empty", () => {
    const issues = [
      issue({ id: "a", category: "ai-safety", severity: "high" }),
      issue({ id: "b", category: "ai-safety", severity: "medium" }),
      issue({ id: "c", category: "supply-chain", severity: "low" }),
    ];
    const summaries = getCodeScanDomainSummaries({ issues, domainSummaries: [] });
    const ai = summaries.find((s) => s.domain === "ai-safety");
    const supply = summaries.find((s) => s.domain === "supply-chain");
    expect(ai?.issueCount).toBe(2);
    expect(ai?.highCount).toBe(1);
    expect(ai?.mediumCount).toBe(1);
    expect(supply?.lowCount).toBe(1);
  });
});

describe("buildSummaryOnlyCodeScanResult", () => {
  it("strips the issues array but keeps domainSummaries populated", () => {
    const original = result({
      issues: [issue({ id: "a", category: "ai-safety", severity: "high" })],
    });
    const stripped = buildSummaryOnlyCodeScanResult(original);
    expect(stripped.issues).toEqual([]);
    expect(stripped.domainSummaries?.length ?? 0).toBeGreaterThan(0);
    expect(stripped.overallScore).toBe(original.overallScore);
  });
});
