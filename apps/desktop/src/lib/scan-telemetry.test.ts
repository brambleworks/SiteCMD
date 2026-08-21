import { describe, expect, it } from "vitest";
import {
  buildCodeScanTelemetryMeta,
  buildScanFailureTelemetryMeta,
  buildWebScanTelemetryMeta,
  durationBucket,
} from "./scan-telemetry";
import type { CheckResult, CodeIssue, CodeScanResult, ScanResult } from "./types";

function webIssue(overrides: Partial<CheckResult> = {}): CheckResult {
  return {
    checkId: "security.headers",
    category: "security",
    title: "Missing header",
    description: "A response header is missing.",
    status: "fail",
    severity: "high",
    fixPrompt: null,
    manualFix: null,
    rawData: null,
    confidence: "high",
    ...overrides,
  };
}

function codeIssue(overrides: Partial<CodeIssue> = {}): CodeIssue {
  return {
    id: "raw-sql:src/db.ts",
    checkId: "code_scan.raw-sql",
    category: "database",
    domain: "database",
    severity: "critical",
    title: "Unsafe query",
    description: "Raw SQL accepts user input.",
    relativePath: "src/db.ts",
    absolutePath: "/Users/dev/project/src/db.ts",
    line: 12,
    sourceExcerpt: null,
    evidence: null,
    whyNow: null,
    likelyFix: null,
    confidence: "high",
    verifyHint: null,
    ...overrides,
  };
}

describe("scan telemetry", () => {
  it.each([
    [500, "under-1s"],
    [2_000, "1s-3s"],
    [9_999, "3s-10s"],
    [45_000, "30s-60s"],
    [300_000, "over-3m"],
    [null, "unknown"],
  ])("buckets duration %s as %s", (durationMs, expected) => {
    expect(durationBucket(durationMs)).toBe(expected);
  });

  it("builds web scan telemetry without scan URLs or raw issue details", () => {
    const result: ScanResult = {
      url: "https://example.com/private?token=abc",
      mode: "live",
      scanType: "health",
      overallScore: 72,
      categories: [],
      durationMs: 12_000,
      timestamp: "2026-05-16T12:00:00.000Z",
      detectedStack: null,
      issues: [
        webIssue({ confidence: "confirmed" }),
        webIssue({ category: "seo", severity: "medium", confidence: "needs_review" }),
      ],
    };

    expect(buildWebScanTelemetryMeta(result, "health", "succeeded")).toEqual({
      scanMode: "web",
      scanType: "health",
      scanOutcome: "succeeded",
      durationBucket: "10s-30s",
      totalIssues: 2,
      criticalIssues: 0,
      highIssues: 1,
      mediumIssues: 1,
      lowIssues: 0,
      confirmedIssues: 1,
      highConfidenceIssues: 0,
      needsReviewIssues: 1,
      securityIssues: 1,
      seoIssues: 1,
    });
  });

  it("builds code scan telemetry from counts and safe domain buckets", () => {
    const result: CodeScanResult = {
      id: 1,
      projectId: 1,
      environmentUrl: "https://example.com",
      overallScore: 55,
      issueCount: 2,
      criticalCount: 1,
      highCount: 1,
      mediumCount: 0,
      lowCount: 0,
      durationMs: 65_000,
      checkedAt: "2026-05-16T12:00:00.000Z",
      framework: null,
      domainSummaries: [],
      issues: [
        codeIssue({ confidence: "confirmed" }),
        codeIssue({ domain: "architecture", severity: "high", confidence: "high" }),
      ],
    };

    expect(buildCodeScanTelemetryMeta(result, "succeeded")).toMatchObject({
      scanMode: "code",
      scanType: "code",
      scanOutcome: "succeeded",
      durationBucket: "1m-3m",
      totalIssues: 2,
      criticalIssues: 1,
      highIssues: 1,
      confirmedIssues: 1,
      highConfidenceIssues: 1,
      databaseIssues: 1,
      architectureIssues: 1,
    });
  });

  it("keeps failure telemetry generic", () => {
    expect(
      buildScanFailureTelemetryMeta({ scanMode: "web", scanType: "health", pageCount: 3 }),
    ).toEqual({
      scanMode: "web",
      scanType: "health",
      scanOutcome: "failed",
      pageCount: 3,
    });
  });
});
