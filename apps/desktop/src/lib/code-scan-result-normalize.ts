import type { CodeIssue, CodeScanDomainSummary, CodeScanResult } from "@/lib/types";

type RawCodeScanResult = Partial<CodeScanResult> & {
  project_id?: number;
  environment_url?: string | null;
  overall_score?: number;
  issue_count?: number;
  critical_count?: number;
  high_count?: number;
  medium_count?: number;
  low_count?: number;
  duration_ms?: number;
  checked_at?: string;
  domain_summaries?: CodeScanDomainSummary[] | null;
  issues?: CodeIssue[] | null;
};

function numberField(
  result: RawCodeScanResult,
  camelKey: keyof CodeScanResult,
  snakeKey: keyof RawCodeScanResult,
  fallback = 0,
) {
  const camelValue = result[camelKey];
  if (typeof camelValue === "number" && Number.isFinite(camelValue)) return camelValue;

  const snakeValue = result[snakeKey];
  if (typeof snakeValue === "number" && Number.isFinite(snakeValue)) return snakeValue;

  return fallback;
}

function stringField(
  result: RawCodeScanResult,
  camelKey: keyof CodeScanResult,
  snakeKey: keyof RawCodeScanResult,
  fallback = "",
) {
  const camelValue = result[camelKey];
  if (typeof camelValue === "string") return camelValue;

  const snakeValue = result[snakeKey];
  if (typeof snakeValue === "string") return snakeValue;

  return fallback;
}

function nullableStringField(
  result: RawCodeScanResult,
  camelKey: keyof CodeScanResult,
  snakeKey: keyof RawCodeScanResult,
) {
  const camelValue = result[camelKey];
  if (typeof camelValue === "string" || camelValue === null) return camelValue;

  const snakeValue = result[snakeKey];
  if (typeof snakeValue === "string" || snakeValue === null) return snakeValue;

  return null;
}

export function normalizeCodeScanResult(result: RawCodeScanResult): CodeScanResult {
  const issues = Array.isArray(result.issues) ? result.issues : [];
  const domainSummaries = Array.isArray(result.domainSummaries)
    ? result.domainSummaries
    : Array.isArray(result.domain_summaries)
      ? result.domain_summaries
      : [];
  const issueCount = numberField(result, "issueCount", "issue_count", issues.length);

  return {
    id: numberField(result, "id", "id"),
    projectId: numberField(result, "projectId", "project_id"),
    environmentUrl: nullableStringField(result, "environmentUrl", "environment_url"),
    overallScore: numberField(result, "overallScore", "overall_score"),
    issueCount,
    criticalCount: numberField(result, "criticalCount", "critical_count"),
    highCount: numberField(result, "highCount", "high_count"),
    mediumCount: numberField(result, "mediumCount", "medium_count"),
    lowCount: numberField(result, "lowCount", "low_count"),
    durationMs: numberField(result, "durationMs", "duration_ms"),
    checkedAt: stringField(result, "checkedAt", "checked_at", new Date().toISOString()),
    framework: nullableStringField(result, "framework", "framework"),
    domainSummaries,
    issues,
  };
}
