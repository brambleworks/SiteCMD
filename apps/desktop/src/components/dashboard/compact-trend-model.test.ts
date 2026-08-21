import { describe, expect, it } from "vitest";
import { buildIssuesTrendModel, buildUpdatesTrendModel } from "./compact-trend-model";
import type { SiteEvent } from "@/lib/types";

describe("compact trend models", () => {
  it("combines web and code issue history for the Issues trend", () => {
    const model = buildIssuesTrendModel({
      webTrend: [
        {
          overall: 80,
          security: null,
          performance: null,
          seo: null,
          accessibility: null,
          compliance: null,
          config: null,
          timestamp: "2026-05-01T00:00:00Z",
          issues: 8,
          scanType: "health",
        },
      ],
      codeTrend: [
        {
          score: 70,
          timestamp: "2026-05-02T00:00:00Z",
          issueCount: 4,
          criticalCount: 1,
          highCount: 2,
        },
      ],
      currentIssueCount: 10,
      criticalCount: 1,
    });

    expect(model.currentValue).toBe("10");
    expect(model.series).toEqual([8, 12, 10]);
    expect(model.tone).toBe("improving");
    expect(model.deltaLabel).toBe("-2 since last checked");
  });

  it("builds an Updates trend from update event counts plus the current report", () => {
    const event = {
      eventType: "update",
      occurredAtMs: Date.parse("2026-05-01T00:00:00Z"),
      detail: JSON.stringify({ remaining_updates: 5 }),
    } as SiteEvent;
    const model = buildUpdatesTrendModel({
      events: [event],
      updates: [
        {
          name: "react",
          currentVersion: "18.0.0",
          latestVersion: "19.0.0",
          ecosystem: "npm",
          updateType: "major",
          isSecurity: false,
          advisorySeverity: null,
          advisoryUrl: null,
          source: "package.json",
          isDev: false,
          isDeprecated: false,
          deprecationMessage: null,
          currentVersionDeprecated: false,
          isStale: false,
          lastPublished: null,
          workspaceMembers: [],
        },
      ],
    });

    expect(model.currentValue).toBe("1");
    expect(model.series).toEqual([5, 1]);
    expect(model.tone).toBe("improving");
  });

  it("shows a flat Updates trend when the latest event matches the current report", () => {
    const event = {
      eventType: "update",
      occurredAtMs: Date.parse("2026-05-01T00:00:00Z"),
      detail: JSON.stringify({ remaining_updates: 1 }),
    } as SiteEvent;
    const model = buildUpdatesTrendModel({
      events: [event],
      updates: [
        {
          name: "react",
          currentVersion: "18.0.0",
          latestVersion: "19.0.0",
          ecosystem: "npm",
          updateType: "major",
          isSecurity: false,
          advisorySeverity: null,
          advisoryUrl: null,
          source: "package.json",
          isDev: false,
          isDeprecated: false,
          deprecationMessage: null,
          currentVersionDeprecated: false,
          isStale: false,
          lastPublished: null,
          workspaceMembers: [],
        },
      ],
    });

    expect(model.series).toEqual([1, 1]);
    expect(model.deltaLabel).toBe("No change since last checked");
    expect(model.tone).toBe("stable");
  });
});
