import { describe, expect, it } from "vitest";

import {
  buildProjectIssueSummary,
  getProjectIssueTotalFromWorkSummary,
} from "./project-issue-summary";

describe("project issue summary", () => {
  it("uses one shared total across the actionable web and code queue", () => {
    const summary = buildProjectIssueSummary({
      webIssues: [{ severity: "high" }, { severity: "critical" }],
      codeIssues: [],
      codeSummaryFallback: {
        issueCount: 4,
        criticalCount: 1,
        highCount: 2,
        mode: "summary",
      },
    });

    expect(summary).toEqual({
      webCount: 2,
      codeCount: 4,
      totalCount: 6,
      criticalCount: 2,
      severityCounts: { critical: 2, high: 3, medium: 1, low: 0 },
    });
  });

  it("does not let page banners inflate actionable issue totals", () => {
    const summary = buildProjectIssueSummary({
      webIssues: Array.from({ length: 37 }, () => ({ severity: "high" })),
      codeIssues: [],
      codeSummaryFallback: {
        issueCount: 12,
        criticalCount: 0,
        highCount: 5,
        mode: "summary",
      },
    });

    expect(summary).toEqual({
      webCount: 37,
      codeCount: 12,
      totalCount: 49,
      criticalCount: 0,
      severityCounts: { critical: 0, high: 42, medium: 7, low: 0 },
    });
  });

  it("collapses duplicate web and code findings into grouped actionable counts", () => {
    const summary = buildProjectIssueSummary({
      webIssues: [
        { severity: "high", checkId: "security.hsts" },
        { severity: "high", checkId: "security.hsts" },
      ],
      codeIssues: [
        {
          severity: "high",
          checkId: "code_scan.public-endpoint-rate-limit",
          category: "security",
          title: "Public-facing route has no clear rate limiting",
        },
        {
          severity: "high",
          checkId: "code_scan.public-endpoint-rate-limit",
          category: "security",
          title: "Public-facing route has no clear rate limiting",
        },
        {
          severity: "critical",
          checkId: "code_scan.supply-chain-typosquat",
          category: "dependencies",
          title: "Declared dependency name looks suspiciously close to a popular library",
        },
        {
          severity: "critical",
          checkId: "code_scan.supply-chain-typosquat",
          category: "dependencies",
          title: "Declared dependency name looks suspiciously close to a popular library",
        },
      ],
    });

    expect(summary).toEqual({
      webCount: 1,
      codeCount: 2,
      totalCount: 3,
      criticalCount: 1,
      severityCounts: { critical: 1, high: 2, medium: 0, low: 0 },
    });
  });

  it("prefers loaded grouped code detail over a noisier summary fallback", () => {
    const summary = buildProjectIssueSummary({
      webIssues: [],
      codeIssues: [
        {
          severity: "high",
          checkId: "code_scan.public-endpoint-rate-limit",
          category: "security",
          title: "Public-facing route has no clear rate limiting",
        },
        {
          severity: "high",
          checkId: "code_scan.public-endpoint-rate-limit",
          category: "security",
          title: "Public-facing route has no clear rate limiting",
        },
      ],
      codeSummaryFallback: {
        issueCount: 8,
        criticalCount: 2,
        highCount: 5,
        mode: "summary",
      },
    });

    expect(summary).toEqual({
      webCount: 0,
      codeCount: 1,
      totalCount: 1,
      criticalCount: 0,
      severityCounts: { critical: 0, high: 1, medium: 0, low: 0 },
    });
  });

  it("uses scanner-provided polish severities in summary counts", () => {
    const summary = buildProjectIssueSummary({
      webIssues: [
        {
          severity: "low",
          checkId: "polish.ai-buzzword-dictionary",
          category: "polish",
          title: "High Marketing Buzzword Density",
        },
        {
          severity: "medium",
          checkId: "polish.div-soup-ratio",
          category: "polish",
          title: "High Div Element Density",
        },
      ],
      codeIssues: [],
    });

    expect(summary.criticalCount).toBe(0);
    expect(summary.severityCounts).toEqual({ critical: 0, high: 0, medium: 1, low: 1 });
  });

  it("clamps a code summary fallback so severity counts never sum past the active count", () => {
    const summary = buildProjectIssueSummary({
      webIssues: Array.from({ length: 9 }, () => ({ severity: "low" })),
      codeIssues: [],
      codeSummaryFallback: {
        issueCount: 2,
        criticalCount: 2,
        highCount: 26,
        mode: "summary",
      },
    });

    const severityTotal =
      summary.severityCounts.critical +
      summary.severityCounts.high +
      summary.severityCounts.medium +
      summary.severityCounts.low;
    expect(severityTotal).toBe(summary.totalCount);
    expect(summary).toEqual({
      webCount: 9,
      codeCount: 2,
      totalCount: 11,
      criticalCount: 2,
      // critical fills the budget of 2 first, leaving no room for the raw highs.
      severityCounts: { critical: 2, high: 0, medium: 0, low: 9 },
    });
  });

  it("excludes launch blockers from work-summary issue totals", () => {
    expect(
      getProjectIssueTotalFromWorkSummary({
        unresolvedCount: 12,
        blockedCount: 2,
        launchBlockerCount: 3,
      }),
    ).toBe(11);
  });
});
