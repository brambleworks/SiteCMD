import { describe, expect, it } from "vitest";
import {
  buildCategoryScores,
  getSeverityConfig,
  type ScoreTrendPoint,
} from "./DashboardComponents";
import type { ScanResult } from "@/lib/types";

function trendPoint(overrides: Partial<ScoreTrendPoint> = {}): ScoreTrendPoint {
  return {
    overall: 80,
    security: 90,
    performance: 85,
    seo: 70,
    accessibility: 60,
    compliance: 75,
    config: null,
    polish: 65,
    timestamp: "2026-04-10T00:00:00Z",
    issues: 3,
    scanType: "health",
    ...overrides,
  };
}

function result(categories: ScanResult["categories"] | undefined): ScanResult {
  return {
    url: "https://example.com",
    score: 80,
    duration_ms: 500,
    timestamp: "2026-04-10T00:00:00Z",
    scanType: "health",
    tech_stack: [],
    detected_cms: null,
    detected_framework: null,
    issues: [],
    categories: categories ?? [],
  } as unknown as ScanResult;
}

describe("buildCategoryScores - detail path", () => {
  it("uses detail.categories when present, sorted by CATEGORY_ORDER", () => {
    const detail = result([
      { category: "polish", score: 80, issuesTotal: 2 },
      { category: "security", score: 90, issuesTotal: 1 },
      { category: "performance", score: 70, issuesTotal: 3 },
      { category: "seo", score: 75, issuesTotal: 0 },
    ] as ScanResult["categories"]);

    const out = buildCategoryScores(trendPoint(), detail);
    // CATEGORY_ORDER is security, performance, seo,..., polish -- so the
    // canonical display order leads with security/performance and ends on polish.
    expect(out.map((c) => c.category)).toEqual(["security", "performance", "seo", "polish"]);
    expect(out.at(-1)?.category).toBe("polish");
  });

  it("drops categories with score=0 from detail.categories", () => {
    const detail = result([
      { category: "security", score: 90, issuesTotal: 1 },
      { category: "polish", score: 0, issuesTotal: 0 },
    ] as ScanResult["categories"]);
    const out = buildCategoryScores(trendPoint(), detail);
    expect(out.map((c) => c.category)).not.toContain("polish");
  });

  it("copies issues_total through as issues", () => {
    const detail = result([
      { category: "security", score: 80, issuesTotal: 7 },
    ] as ScanResult["categories"]);
    const out = buildCategoryScores(trendPoint(), detail);
    expect(out[0].issues).toBe(7);
  });
});

describe("buildCategoryScores - trend-only fallback", () => {
  it("uses trend values when detail is null", () => {
    const out = buildCategoryScores(trendPoint(), null);
    expect(out.length).toBe(6);
    expect(out.find((c) => c.category === "security")?.score).toBe(90);
    // issues is always 0 on the trend-only path
    expect(out.every((c) => c.issues === 0)).toBe(true);
  });

  it("skips categories with null or zero scores", () => {
    const out = buildCategoryScores(trendPoint({ seo: null, compliance: 0, polish: null }), null);
    const keys = out.map((c) => c.category);
    expect(keys).not.toContain("seo");
    expect(keys).not.toContain("compliance");
    expect(keys).not.toContain("polish");
  });

  it("falls back to trend when detail.categories is an empty array", () => {
    const out = buildCategoryScores(trendPoint(), result([]));
    expect(out).toEqual([]);
  });
});

describe("getSeverityConfig", () => {
  it("returns severity-specific text classes", () => {
    expect(getSeverityConfig("critical").color).toBe("text-severity-critical");
    expect(getSeverityConfig("high").color).toBe("text-severity-high");
    expect(getSeverityConfig("medium").color).toBe("text-severity-medium");
    expect(getSeverityConfig("low").color).toBe("text-severity-low");
  });

  it("falls back to muted-foreground for unknown severities", () => {
    expect(getSeverityConfig("nonsense").color).toBe("text-muted-foreground");
  });
});
