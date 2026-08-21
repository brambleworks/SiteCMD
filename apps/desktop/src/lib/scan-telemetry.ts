import { getIssueConfidence } from "@/lib/issue-confidence";
import type { CheckResult, CodeIssue, CodeScanResult, ScanResult } from "@/lib/types";

type PrimitiveTelemetryValue = string | number | boolean | null;
type ScanTelemetryMeta = Record<string, PrimitiveTelemetryValue>;

const CATEGORY_KEY: Record<string, string> = {
  accessibility: "accessibilityIssues",
  "ai-safety": "aiSafetyIssues",
  architecture: "architectureIssues",
  compliance: "complianceIssues",
  config: "configIssues",
  database: "databaseIssues",
  dependencies: "dependencyIssues",
  performance: "performanceIssues",
  polish: "polishIssues",
  reliability: "reliabilityIssues",
  security: "securityIssues",
  seo: "seoIssues",
  "supply-chain": "dependencyIssues",
};

export function durationBucket(durationMs: number | null | undefined): string {
  if (typeof durationMs !== "number" || !Number.isFinite(durationMs) || durationMs < 0) {
    return "unknown";
  }
  if (durationMs < 1_000) return "under-1s";
  if (durationMs < 3_000) return "1s-3s";
  if (durationMs < 10_000) return "3s-10s";
  if (durationMs < 30_000) return "10s-30s";
  if (durationMs < 60_000) return "30s-60s";
  if (durationMs < 180_000) return "1m-3m";
  return "over-3m";
}

export function buildWebScanTelemetryMeta(
  result: ScanResult,
  scanType: string,
  scanOutcome: "succeeded" | "failed",
): ScanTelemetryMeta {
  return {
    scanMode: "web",
    scanType,
    scanOutcome,
    durationBucket: durationBucket(result.durationMs),
    totalIssues: result.issues.length,
    ...countIssueTelemetry(result.issues, (issue) => issue.category),
  };
}

export function buildCodeScanTelemetryMeta(
  result: CodeScanResult,
  scanOutcome: "succeeded" | "failed",
): ScanTelemetryMeta {
  return {
    scanMode: "code",
    scanType: "code",
    scanOutcome,
    durationBucket: durationBucket(result.durationMs),
    totalIssues: result.issueCount,
    criticalIssues: result.criticalCount,
    highIssues: result.highCount,
    mediumIssues: result.mediumCount,
    lowIssues: result.lowCount,
    ...countIssueTelemetry(result.issues, (issue) => issue.domain ?? issue.category),
  };
}

export function buildScanFailureTelemetryMeta(input: {
  scanMode: "web" | "multi" | "code";
  scanType: string;
  pageCount?: number;
}): ScanTelemetryMeta {
  return {
    scanMode: input.scanMode,
    scanType: input.scanType,
    scanOutcome: "failed",
    ...(typeof input.pageCount === "number" ? { pageCount: input.pageCount } : {}),
  };
}

function countIssueTelemetry<TIssue extends CheckResult | CodeIssue>(
  issues: TIssue[],
  categoryForIssue: (issue: TIssue) => string | null | undefined,
): ScanTelemetryMeta {
  const meta: ScanTelemetryMeta = {
    criticalIssues: 0,
    highIssues: 0,
    mediumIssues: 0,
    lowIssues: 0,
    confirmedIssues: 0,
    highConfidenceIssues: 0,
    needsReviewIssues: 0,
  };

  for (const issue of issues) {
    increment(meta, `${issue.severity}Issues`);
    const confidence = getIssueConfidence(issue);
    if (confidence === "confirmed") {
      increment(meta, "confirmedIssues");
    } else if (confidence === "needs_review") {
      increment(meta, "needsReviewIssues");
    } else {
      increment(meta, "highConfidenceIssues");
    }

    const category = categoryForIssue(issue);
    if (category) {
      const key = CATEGORY_KEY[category];
      if (key) increment(meta, key);
    }
  }

  return meta;
}

function increment(meta: ScanTelemetryMeta, key: string) {
  const current = meta[key];
  meta[key] = typeof current === "number" ? current + 1 : 1;
}
