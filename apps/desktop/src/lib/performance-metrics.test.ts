import { beforeEach, describe, expect, it } from "vitest";

import {
  buildPerformanceSnapshotText,
  clearPerformanceSnapshot,
  finishPerformanceTimer,
  readPerformanceSnapshot,
  recordPerformanceMetric,
  startPerformanceTimer,
} from "./performance-metrics";

describe("performance-metrics", () => {
  beforeEach(() => {
    clearPerformanceSnapshot();
  });

  it("records and summarizes metric samples", () => {
    recordPerformanceMetric("app.cold_start_ms", 820, { source: "test" });
    recordPerformanceMetric("app.cold_start_ms", 910, { source: "test" });

    const coldStart = readPerformanceSnapshot().find(
      (metric) => metric.key === "app.cold_start_ms",
    );

    expect(coldStart).toMatchObject({
      count: 2,
      firstDurationMs: 820,
      latestDurationMs: 910,
      budgetMs: 2500,
      withinBudget: true,
    });
    expect(coldStart?.latestMeta).toEqual({ source: "test" });
  });

  it("records a timer when it is finished", () => {
    const timer = startPerformanceTimer("issues.initial_ready_ms", { projectId: 7 });
    finishPerformanceTimer(timer, { issueCount: 12 });

    const issuesMetric = readPerformanceSnapshot().find(
      (metric) => metric.key === "issues.initial_ready_ms",
    );

    expect(issuesMetric?.count).toBe(1);
    expect(issuesMetric?.latestMeta).toEqual({ projectId: 7, issueCount: 12 });
  });

  it("renders pending metrics in the snapshot text until samples exist", () => {
    const snapshot = buildPerformanceSnapshotText();

    expect(snapshot).toContain("Cold app start: pending");
    expect(snapshot).toContain("Issues page render: pending");
  });

  it("ignores non-finite persisted durations and meta values", () => {
    window.localStorage.setItem(
      "sitecmd_performance_metrics_v1",
      `{
        "metrics": {
          "app.cold_start_ms": [
            {
              "durationMs": 1e999,
              "recordedAt": "2026-04-14T12:00:00.000Z",
              "meta": { "source": "bad" }
            },
            {
              "durationMs": -5,
              "recordedAt": "2026-04-14T12:01:00.000Z",
              "meta": { "source": "negative" }
            },
            {
              "durationMs": 1200.4,
              "recordedAt": "2026-04-14T12:02:00.000Z",
              "meta": {
                "source": "good",
                "badNumber": 1e999,
                "enabled": true
              }
            }
          ]
        }
      }`,
    );

    const coldStart = readPerformanceSnapshot().find(
      (metric) => metric.key === "app.cold_start_ms",
    );

    expect(coldStart).toMatchObject({
      count: 1,
      firstDurationMs: 1200,
      latestDurationMs: 1200,
      averageDurationMs: 1200,
    });
    expect(coldStart?.latestMeta).toEqual({ source: "good", enabled: true });
    expect(buildPerformanceSnapshotText()).not.toContain("Infinity");
  });
});
