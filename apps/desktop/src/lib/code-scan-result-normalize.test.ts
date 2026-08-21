import { describe, expect, it } from "vitest";

import { normalizeCodeScanResult } from "@/lib/code-scan-result-normalize";

describe("normalizeCodeScanResult", () => {
  it("keeps a complete camelCase code scan result intact", () => {
    const result = normalizeCodeScanResult({
      id: 12,
      projectId: 7,
      environmentUrl: "https://example.com",
      overallScore: 81,
      issueCount: 1,
      criticalCount: 0,
      highCount: 1,
      mediumCount: 0,
      lowCount: 0,
      durationMs: 1200,
      checkedAt: "2026-05-04T12:00:00Z",
      framework: "Next.js",
      domainSummaries: [],
      issues: [],
    });

    expect(result).toMatchObject({
      id: 12,
      projectId: 7,
      environmentUrl: "https://example.com",
      overallScore: 81,
      issueCount: 1,
      durationMs: 1200,
      checkedAt: "2026-05-04T12:00:00Z",
      issues: [],
    });
  });

  it("accepts summary-like or snake_case results without throwing on missing issues", () => {
    const result = normalizeCodeScanResult({
      id: 48,
      project_id: 1,
      environment_url: "https://example.com",
      overall_score: 36,
      issue_count: 25,
      critical_count: 0,
      high_count: 19,
      medium_count: 6,
      low_count: 0,
      duration_ms: 44834,
      checked_at: "2026-05-04T17:59:58.616744+00:00",
      framework: "Drupal",
      domain_summaries: [],
    });

    expect(result.projectId).toBe(1);
    expect(result.overallScore).toBe(36);
    expect(result.issueCount).toBe(25);
    expect(result.durationMs).toBe(44834);
    expect(result.issues).toEqual([]);
    expect(result.domainSummaries).toEqual([]);
  });
});
