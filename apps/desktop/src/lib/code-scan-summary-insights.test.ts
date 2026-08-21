import { describe, expect, it } from "vitest";
import {
  buildCodeScanDomainRows,
  buildCodeScanSummaryFromResult,
  describeCodeScanDomainTrend,
} from "./code-scan-summary-insights";
import type { CodeIssue, CodeScanDomainSummary, CodeScanResult, CodeScanSummary } from "./types";
import { getCodeIssueDomain, type ClassifiableCodeIssue } from "./code-scan-domains";
import type { CodeScanDomain } from "./code-scan-domains";

function domainSummary(
  domain: CodeScanDomain,
  issueCount: number,
  critical = 0,
  high = 0,
): CodeScanDomainSummary {
  return {
    domain,
    issueCount,
    criticalCount: critical,
    highCount: high,
    mediumCount: 0,
    lowCount: Math.max(0, issueCount - critical - high),
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

function issue(overrides: Partial<CodeIssue> = {}): CodeIssue {
  const base: ClassifiableCodeIssue = {
    id: "test",
    checkId: "code_scan.test",
    category: "ai-safety",
    severity: "high",
    title: "Issue",
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

function result(issues: CodeIssue[], overrides: Partial<CodeScanResult> = {}): CodeScanResult {
  return {
    id: 1,
    projectId: 1,
    environmentUrl: "https://example.com",
    overallScore: 80,
    issueCount: issues.length,
    criticalCount: 0,
    highCount: 0,
    mediumCount: 0,
    lowCount: 0,
    durationMs: 100,
    checkedAt: "2026-04-10T00:00:00Z",
    framework: null,
    domainSummaries: [],
    issues,
    ...overrides,
  };
}

describe("buildCodeScanSummaryFromResult", () => {
  it("picks the most severe domain as topDomain before falling back to raw count", () => {
    const r = result([
      issue({ id: "a", category: "supply-chain", severity: "low" }),
      issue({ id: "b", category: "supply-chain", severity: "low" }),
      issue({ id: "c", category: "supply-chain", severity: "low" }),
      issue({ id: "d", category: "security", severity: "critical" }),
      issue({ id: "e", category: "security", severity: "high" }),
    ]);
    const s = buildCodeScanSummaryFromResult(r);
    expect(s.topDomain).toBe("security");
    expect(s.topDomainCount).toBe(2);
  });

  it("preserves the core scan fields (id, checkedAt, overallScore, ...)", () => {
    const r = result([], {
      id: 42,
      overallScore: 77,
      checkedAt: "2026-04-11T12:34:56Z",
      framework: "Next.js",
    });
    const s = buildCodeScanSummaryFromResult(r);
    expect(s.id).toBe(42);
    expect(s.overallScore).toBe(77);
    expect(s.checkedAt).toBe("2026-04-11T12:34:56Z");
    expect(s.framework).toBe("Next.js");
  });

  it("leaves topDomain null when there are no issues", () => {
    const s = buildCodeScanSummaryFromResult(result([]));
    expect(s.topDomain).toBeNull();
    expect(s.topDomainCount).toBe(0);
  });
});

describe("describeCodeScanDomainTrend", () => {
  it("returns null label/tone when either summary is missing", () => {
    expect(describeCodeScanDomainTrend(null, null)).toEqual({ label: null, tone: null });
    expect(describeCodeScanDomainTrend(summary(), null)).toEqual({ label: null, tone: null });
  });

  it("reports regression when a domain grew", () => {
    const prev = summary({
      domainSummaries: [domainSummary("ai-safety", 2, 0, 1)],
    });
    const curr = summary({
      domainSummaries: [domainSummary("ai-safety", 6, 0, 3)],
    });
    const trend = describeCodeScanDomainTrend(curr, prev);
    expect(trend.tone).toBe("regressed");
    expect(trend.label).toContain("grew by 4");
  });

  it("reports improvement when a domain shrank", () => {
    const prev = summary({
      domainSummaries: [domainSummary("database", 5)],
    });
    const curr = summary({
      domainSummaries: [domainSummary("database", 2)],
    });
    const trend = describeCodeScanDomainTrend(curr, prev);
    expect(trend.tone).toBe("improved");
    expect(trend.label).toContain("eased by 3");
  });

  it("reports the strongest domain delta when top domain swaps with equal magnitude", () => {
    const prev = summary({
      topDomain: "database",
      domainSummaries: [domainSummary("database", 5)],
    });
    const curr = summary({
      topDomain: "security",
      domainSummaries: [domainSummary("security", 5)],
    });
    const trend = describeCodeScanDomainTrend(curr, prev);
    expect(trend.tone).toBe("improved");
    expect(trend.label).toContain("eased by 5");
  });

  it("reports stable when counts are identical", () => {
    const prev = summary({
      topDomain: "database",
      domainSummaries: [domainSummary("database", 3)],
    });
    const curr = summary({
      topDomain: "database",
      domainSummaries: [domainSummary("database", 3)],
    });
    expect(describeCodeScanDomainTrend(curr, prev).tone).toBe("stable");
  });
});

describe("buildCodeScanDomainRows", () => {
  it("returns an empty list when current summary is null", () => {
    expect(buildCodeScanDomainRows(null, null)).toEqual([]);
  });

  it("sorts domains by severity pressure before raw count and truncates to the limit", () => {
    const curr = summary({
      domainSummaries: [
        domainSummary("database", 4, 0, 2),
        domainSummary("ai-safety", 8),
        domainSummary("security", 2, 1, 1),
        domainSummary("architecture", 2),
      ],
    });
    const rows = buildCodeScanDomainRows(curr, null, 3);
    expect(rows.map((r) => r.domain)).toEqual(["database", "ai-safety", "security"]);
    expect(rows[0].score).toBe(79);
    expect(rows[1].score).toBe(97);
    expect(rows[2].score).toBe(49);
  });

  it("fills missing domains with clean 100 scores so the card can stay structurally consistent", () => {
    const curr = summary({
      domainSummaries: [domainSummary("security", 2, 1, 1)],
    });
    const rows = buildCodeScanDomainRows(curr, null);
    expect(rows).toHaveLength(7);
    expect(rows[0].domain).toBe("database");
    expect(rows[0].score).toBe(100);
    expect(rows[0].count).toBe(0);
    expect(rows[2].domain).toBe("security");
    expect(rows[2].score).toBe(49);
  });

  it("tags rows as improved / regressed / stable based on domain score delta vs previous", () => {
    const prev = summary({
      domainSummaries: [
        domainSummary("ai-safety", 6),
        domainSummary("database", 3),
        domainSummary("security", 2),
      ],
    });
    const curr = summary({
      domainSummaries: [
        domainSummary("ai-safety", 1),
        domainSummary("database", 3),
        domainSummary("security", 7),
      ],
    });
    const rows = buildCodeScanDomainRows(curr, prev);
    const bySlug = Object.fromEntries(rows.map((r) => [r.domain, r]));
    expect(bySlug["ai-safety"].tone).toBe("improved");
    expect(bySlug["ai-safety"].delta).toBe(1);
    expect(bySlug["database"].tone).toBe("stable");
    expect(bySlug["security"].tone).toBe("regressed");
    expect(bySlug["security"].delta).toBe(-2);
  });

  it("leaves delta null when there's no previous summary to compare against", () => {
    const curr = summary({
      domainSummaries: [domainSummary("ai-safety", 3)],
    });
    const rows = buildCodeScanDomainRows(curr, null);
    expect(rows[0].delta).toBeNull();
    expect(rows[0].tone).toBe("stable");
  });
});
