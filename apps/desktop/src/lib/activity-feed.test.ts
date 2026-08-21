import { describe, expect, it } from "vitest";

import {
  buildUpdateBreakdownFromEventDetail,
  formatUpdateBreakdown,
  normalizeActivityFeedEvents,
} from "./activity-feed";
import type { SiteEvent } from "./types";

function siteEvent(overrides: Partial<SiteEvent>): SiteEvent {
  return {
    id: 1,
    projectId: 7,
    eventType: "scan",
    severity: "info",
    occurredAtMs: Date.parse("2026-05-11T12:00:00.000Z"),
    title: "SiteCMD Score: 95/100",
    summary: "",
    detail: null,
    parsedDetail: null,
    source: "internal",
    sourceId: null,
    metadata: null,
    affectedCheckIds: null,
    ...overrides,
  };
}

describe("activity-feed", () => {
  it("normalizes persisted update bucket counts before formatting", () => {
    const breakdown = buildUpdateBreakdownFromEventDetail({
      critical_updates: 1.6,
      major_updates: -5,
      minor_updates: Number.POSITIVE_INFINITY,
      patch_updates: 2,
    });

    expect(breakdown).toEqual({
      critical: 2,
      major: 0,
      minor: 0,
      patch: 2,
    });
    expect(formatUpdateBreakdown(breakdown!)).toBe("2 Critical, 0 Major, 0 Minor, 2 Patch");
  });

  it("summarizes paired full scans by issues instead of split source scores", () => {
    const items = normalizeActivityFeedEvents([
      siteEvent({
        id: 1,
        title: "SiteCMD Score: 82/100",
        parsedDetail: { scan_type: "health", issues_total: 1 },
      }),
      siteEvent({
        id: 2,
        title: "SiteCMD Score: 77/100",
        parsedDetail: { scan_type: "code", issues_total: 4 },
      }),
    ]);

    expect(items).toHaveLength(1);
    expect(items[0]?.title).toBe("Full Scan");
    expect(items[0]?.summary).toBe("1 web issue · 4 code issues");
  });
});
