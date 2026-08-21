import { describe, expect, it } from "vitest";
import type { CodeScanSummary, ScanResult, SiteEvent } from "@/lib/types";
import { buildDashboardActivity, buildDashboardActivityFromEvents } from "./activity";

function webScan(timestamp: string, overallScore = 82): ScanResult {
  return {
    url: "https://example.com",
    mode: "live",
    scanType: "health",
    overallScore: overallScore,
    categories: [],
    issues: [],
    detectedStack: null,
    durationMs: 1000,
    timestamp,
  };
}

function codeScan(
  checkedAt: string,
  issueCount = 3,
  criticalCount = 0,
  overallScore = 79,
): CodeScanSummary {
  return {
    id: 99,
    projectId: 7,
    environmentUrl: "https://example.com",
    overallScore,
    issueCount,
    groupedIssueCount: issueCount,
    criticalCount,
    highCount: criticalCount,
    durationMs: 1200,
    checkedAt,
    framework: "Next.js",
    topDomain: null,
    topDomainCount: 0,
    domainSummaries: [],
  };
}

describe("buildDashboardActivity", () => {
  it("sorts activity by real timestamps instead of the original section order", () => {
    const items = buildDashboardActivity({
      latestDeploy: null,
      commitsSinceLastScan: [],
      latestWebScan: webScan("2026-04-20T16:08:00Z"),
      webIssueCount: 4,
      latestCodeScan: null,
      updatesCheckedAt: "2026-04-20T16:35:00Z",
      updateBreakdown: { critical: 0, major: 1, minor: 1, patch: 0 },
    });

    expect(items[0]).toMatchObject({
      label: "Update Check",
      value: "0 Critical, 1 Major, 1 Minor, 0 Patch",
    });
    expect(items[1]).toMatchObject({
      label: "Web Scan",
      value: "4 issues found",
    });
  });

  it("keeps raw scan artifact scores out of dashboard activity labels", () => {
    const items = buildDashboardActivity({
      latestDeploy: null,
      commitsSinceLastScan: [],
      latestWebScan: webScan("2026-04-20T16:08:00Z", 41),
      webIssueCount: 1,
      latestCodeScan: codeScan("2026-04-20T17:09:30Z", 2, 0, 99),
      updatesCheckedAt: null,
      updateBreakdown: { critical: 0, major: 0, minor: 0, patch: 0 },
    });

    expect(items.map((item) => item.value)).toEqual(["2 issues found", "1 issue found"]);
    expect(items.map((item) => item.value).join(" ")).not.toContain("score");
  });

  it("collapses matching web + code scans into a single Full Scan activity item", () => {
    const items = buildDashboardActivity({
      latestDeploy: null,
      commitsSinceLastScan: [],
      latestWebScan: webScan("2026-04-20T16:08:00Z"),
      webIssueCount: 12,
      latestCodeScan: codeScan("2026-04-20T16:09:30Z", 5, 1, 76),
      updatesCheckedAt: null,
      updateBreakdown: { critical: 0, major: 0, minor: 0, patch: 0 },
    });

    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      label: "Full Scan",
      value: "12 web issues · 5 code issues",
      target: "issues",
    });
  });

  it("labels update activity as an explicit check and shows the outcome", () => {
    const items = buildDashboardActivity({
      latestDeploy: null,
      commitsSinceLastScan: [],
      latestWebScan: null,
      webIssueCount: 0,
      latestCodeScan: null,
      updatesCheckedAt: "2026-04-20T16:35:00Z",
      updateBreakdown: { critical: 0, major: 0, minor: 0, patch: 0 },
    });

    expect(items[0]).toMatchObject({
      label: "Update Check",
      value: "0 Critical, 0 Major, 0 Minor, 0 Patch",
      valueColor: "green",
    });
  });
});

function dashboardEvent(overrides: Partial<SiteEvent> = {}): SiteEvent {
  return {
    id: 1,
    projectId: 7,
    eventType: "scan",
    severity: "info",
    occurredAtMs: Date.parse("2026-04-20T16:08:00Z"),
    title: "SiteCMD Score: 81/100",
    summary: "3 issues (1 critical, 1 high)",
    detail: JSON.stringify({
      scan_id: 41,
      scan_type: "health",
      overall_score: 81,
      url: "https://example.com",
    }),
    source: "internal",
    sourceId: "scan_41",
    metadata: null,
    affectedCheckIds: null,
    ...overrides,
  };
}

describe("buildDashboardActivityFromEvents", () => {
  it("collapses adjacent web and code scan events into a single Full Scan item", () => {
    const items = buildDashboardActivityFromEvents([
      dashboardEvent({
        id: 2,
        occurredAtMs: Date.parse("2026-04-20T16:09:00Z"),
        title: "SiteCMD Score: 77/100",
        summary: "4 code issues (1 critical, 1 high)",
        detail: JSON.stringify({
          code_scan_id: 42,
          scan_type: "code",
          overall_score: 77,
          issues_total: 4,
          url: "https://example.com",
        }),
      }),
      dashboardEvent({
        detail: JSON.stringify({
          scan_id: 41,
          scan_type: "health",
          overall_score: 81,
          issues_total: 3,
          url: "https://example.com",
        }),
      }),
    ]);

    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({
      label: "Full Scan",
      value: "3 web issues · 4 code issues",
      target: "events",
    });
  });

  it("limits the dashboard feed to five newest events", () => {
    const items = buildDashboardActivityFromEvents([
      dashboardEvent({
        id: 10,
        occurredAtMs: Date.parse("2026-04-20T16:35:00Z"),
        eventType: "update",
        title: "3 Updates Applied",
        summary: "Updated three packages.",
        detail: null,
      }),
      dashboardEvent({
        id: 9,
        occurredAtMs: Date.parse("2026-04-20T15:40:00Z"),
        eventType: "deploy",
        title: "Deploy passed",
        summary: "main deployed successfully.",
        detail: null,
        source: "git",
      }),
      dashboardEvent({
        id: 8,
        occurredAtMs: Date.parse("2026-04-20T15:20:00Z"),
        eventType: "search",
        title: "Search clicks dropped",
        summary: "Clicks are down.",
        detail: null,
      }),
      dashboardEvent({
        id: 7,
        occurredAtMs: Date.parse("2026-04-20T15:00:00Z"),
        eventType: "uptime",
        title: "Uptime recovered",
        summary: "The site is back up.",
        detail: null,
        source: "uptimerobot",
      }),
      dashboardEvent({
        id: 6,
        occurredAtMs: Date.parse("2026-04-20T14:40:00Z"),
        eventType: "analytics",
        title: "Traffic spike",
        summary: "Visitors are up 20%.",
        detail: null,
        source: "plausible",
      }),
      dashboardEvent({
        id: 5,
        occurredAtMs: Date.parse("2026-04-20T14:20:00Z"),
        eventType: "security",
        title: "Security regression",
        summary: "A new blocker appeared.",
        detail: null,
      }),
    ]);

    expect(items).toHaveLength(5);
    expect(items.map((item) => item.label)).toEqual([
      "3 Updates Applied",
      "Deploy passed",
      "Search clicks dropped",
      "Uptime recovered",
      "Traffic spike",
    ]);
  });

  it("formats update events with compact severity buckets when detail has update counts", () => {
    const items = buildDashboardActivityFromEvents([
      dashboardEvent({
        id: 11,
        occurredAtMs: Date.parse("2026-04-20T16:35:00Z"),
        eventType: "update",
        title: "3 Updates Applied",
        summary: "react, vite, and lucide-react were updated.",
        detail: JSON.stringify({
          critical_updates: 1,
          major_updates: 1,
          minor_updates: 0,
          patch_updates: 2,
        }),
      }),
    ]);

    expect(items[0]).toMatchObject({
      label: "3 Updates Applied",
      value: "1 Critical, 1 Major, 0 Minor, 2 Patch",
    });
  });
});
