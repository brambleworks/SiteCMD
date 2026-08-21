import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/tauri-invoke", () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import {
  buildSeoCategoryScore,
  getSingleFocusedSeoIssueId,
  inferSeoFocus,
  matchesSeoFocus,
} from "./SearchConsolePage";
import {
  buildGscObservations,
  buildSearchTrendSummary,
  buildSeoCoverageGroups,
} from "./search-console-page-model";
import type { CheckResult } from "@/lib/types";

function webIssue(overrides: Partial<CheckResult> = {}): CheckResult {
  return {
    checkId: "seo.meta_description",
    category: "seo",
    title: "Missing meta description",
    description: "",
    status: "fail",
    severity: "medium",
    fixPrompt: null,
    manualFix: null,
    rawData: null,
    confidence: "high",
    ...overrides,
  };
}

describe("matchesSeoFocus", () => {
  it("returns false when focus is null/empty", () => {
    expect(matchesSeoFocus(webIssue(), null)).toBe(false);
    expect(matchesSeoFocus(webIssue(), "")).toBe(false);
  });

  it("matches robots focus for robots.txt checks", () => {
    expect(matchesSeoFocus(webIssue({ checkId: "seo.robots_txt" }), "seo.robots")).toBe(true);
  });

  it("matches sitemap focus", () => {
    expect(
      matchesSeoFocus(
        webIssue({ checkId: "seo.sitemap", title: "Sitemap missing" }),
        "seo.sitemap",
      ),
    ).toBe(true);
  });

  it("titles focus matches 'title' substring", () => {
    expect(
      matchesSeoFocus(webIssue({ checkId: "seo.title", title: "Short title" }), "seo.titles"),
    ).toBe(true);
  });

  it("descriptions focus matches meta-description variants", () => {
    expect(matchesSeoFocus(webIssue({ checkId: "seo.meta_description" }), "seo.descriptions")).toBe(
      true,
    );
    expect(matchesSeoFocus(webIssue({ checkId: "seo.meta-description" }), "seo.descriptions")).toBe(
      true,
    );
    expect(matchesSeoFocus(webIssue({ checkId: "seo.description" }), "seo.descriptions")).toBe(
      true,
    );
  });

  it("canonical focus", () => {
    expect(
      matchesSeoFocus(
        webIssue({ checkId: "seo.canonical", title: "Canonical tag missing" }),
        "seo.canonical",
      ),
    ).toBe(true);
  });

  it("structured_data matches structured/schema/json_ld variants", () => {
    expect(matchesSeoFocus(webIssue({ checkId: "seo.schema" }), "seo.structured_data")).toBe(true);
    expect(matchesSeoFocus(webIssue({ checkId: "seo.json_ld" }), "seo.structured_data")).toBe(true);
    expect(matchesSeoFocus(webIssue({ checkId: "seo.json-ld" }), "seo.structured_data")).toBe(true);
  });

  it("noindex focus matches indexability checks", () => {
    expect(matchesSeoFocus(webIssue({ checkId: "seo.indexability" }), "seo.noindex")).toBe(true);
  });

  it("falls through to raw focus substring when no patterns defined", () => {
    expect(matchesSeoFocus(webIssue({ checkId: "something-weird" }), "weird")).toBe(true);
  });
});

describe("getSingleFocusedSeoIssueId", () => {
  it("returns the only matching issue when focus narrows to one result", () => {
    expect(
      getSingleFocusedSeoIssueId(
        [
          webIssue({ checkId: "seo.robots_txt", title: "Robots blocked" }),
          webIssue({ checkId: "seo.title", title: "Title missing" }),
        ],
        "seo.robots",
      ),
    ).toBe("seo.robots_txt");
  });

  it("returns null when focus matches zero or multiple issues", () => {
    expect(
      getSingleFocusedSeoIssueId(
        [webIssue({ checkId: "seo.title", title: "Title missing" })],
        "seo.robots",
      ),
    ).toBeNull();

    expect(
      getSingleFocusedSeoIssueId(
        [
          webIssue({ checkId: "seo.title", title: "Title missing" }),
          webIssue({ checkId: "seo.page_title", title: "Page title duplicated" }),
        ],
        "seo.titles",
      ),
    ).toBeNull();
  });
});

describe("inferSeoFocus", () => {
  it("finds the first focus that matches", () => {
    // default webIssue title mentions 'description' so override title on
    // each case to avoid cross-matching seo.descriptions first.
    expect(inferSeoFocus(webIssue({ checkId: "seo.robots_txt", title: "Robots blocked" }))).toBe(
      "seo.robots",
    );
    expect(inferSeoFocus(webIssue({ checkId: "seo.sitemap", title: "Sitemap missing" }))).toBe(
      "seo.sitemap",
    );
    expect(
      inferSeoFocus(webIssue({ checkId: "seo.canonical", title: "Missing canonical tag" })),
    ).toBe("seo.canonical");
  });

  it("returns null when nothing matches", () => {
    expect(inferSeoFocus(webIssue({ checkId: "unrelated", title: "nothing" }))).toBeNull();
  });
});

describe("buildSeoCategoryScore", () => {
  it("returns null when no SEO checks are present", () => {
    expect(buildSeoCategoryScore([webIssue({ category: "security" })])).toBeNull();
  });

  it("recomputes the SEO category score from current check states", () => {
    expect(
      buildSeoCategoryScore([
        webIssue({ checkId: "seo.robots", severity: "critical", status: "fail" }),
        webIssue({ checkId: "seo.title", severity: "high", status: "warn" }),
        webIssue({ checkId: "seo.canonical", severity: "medium", status: "pass" }),
      ]),
    ).toEqual({
      category: "seo",
      score: 73,
      issuesTotal: 2,
      issuesCritical: 1,
      issuesHigh: 1,
      issuesMedium: 0,
      issuesLow: 0,
      issuesPassed: 1,
    });
  });
});

describe("search visibility model helpers", () => {
  it("groups SEO checks into coverage areas with pass/fail counts", () => {
    const groups = buildSeoCoverageGroups(
      [
        webIssue({
          checkId: "seo.sitemap",
          title: "Sitemap is missing",
          description: "Search engines need a sitemap.",
          severity: "high",
        }),
      ],
      [
        webIssue({
          checkId: "seo.title",
          title: "Every page has a unique title",
          description: "Title tags are present.",
          status: "pass",
        }),
      ],
    );

    expect(groups.find((group) => group.id === "discovery")).toMatchObject({
      status: "needs-work",
      total: 1,
    });
    expect(groups.find((group) => group.id === "metadata")).toMatchObject({
      status: "covered",
      total: 1,
    });
  });

  it("summarizes search trend direction from Search Console daily points", () => {
    expect(
      buildSearchTrendSummary({
        total_clicks: 12,
        total_impressions: 200,
        average_ctr: 0.06,
        average_position: 8,
        top_queries: [],
        top_pages: [],
        devices: [],
        daily: [
          { date: "2026-04-01", clicks: 1, impressions: 20, ctr: 0.05, position: 9 },
          { date: "2026-04-02", clicks: 1, impressions: 20, ctr: 0.05, position: 9 },
          { date: "2026-04-03", clicks: 4, impressions: 70, ctr: 0.057, position: 8 },
          { date: "2026-04-04", clicks: 6, impressions: 90, ctr: 0.067, position: 7 },
        ],
      }),
    ).toMatchObject({
      tone: "up",
      deltaLabel: "+8 clicks",
    });
  });

  it("builds GSC observations from low-CTR queries, near-ranking queries, and low-CTR pages", () => {
    const observations = buildGscObservations({
      total_clicks: 20,
      total_impressions: 1000,
      average_ctr: 0.02,
      average_position: 9,
      devices: [],
      daily: [],
      top_pages: [{ page: "/pricing", clicks: 2, impressions: 200, ctr: 0.01, position: 5 }],
      top_queries: [
        { query: "smart home course", clicks: 2, impressions: 400, ctr: 0.005, position: 7 },
        { query: "tiny house plans", clicks: 5, impressions: 80, ctr: 0.0625, position: 6 },
      ],
    });

    const ids = observations.map((o) => o.id);
    expect(ids).toContain("query-ctr:smart home course");
    expect(ids).toContain("query-position:tiny house plans");
    expect(ids).toContain("page-ctr:/pricing");
  });

  it("wraps query strings in guillemets so embedded double-quotes do not break the label", () => {
    const observations = buildGscObservations({
      total_clicks: 0,
      total_impressions: 0,
      average_ctr: 0,
      average_position: 0,
      devices: [],
      daily: [],
      top_pages: [],
      top_queries: [
        { query: `"dreame-hold" wifi`, clicks: 1, impressions: 500, ctr: 0.002, position: 7 },
      ],
    });

    const labels = observations.map((o) => o.label).join(" ");
    expect(labels).not.toContain(`""dreame-hold"`);
    expect(labels).toContain("«dreame-hold wifi»");
  });

  it("returns empty when gscData is null", () => {
    expect(buildGscObservations(null)).toEqual([]);
    expect(buildGscObservations(undefined)).toEqual([]);
  });
});
