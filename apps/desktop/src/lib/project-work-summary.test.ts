import { describe, expect, it } from "vitest";

import {
  EMPTY_PROJECT_WORK_SUMMARY,
  getProjectWorkSummaryIssueTotal,
  getProjectWorkSummaryOrEmpty,
  hasProjectWorkSummaryActivity,
} from "./project-work-summary";

describe("project work summary helpers", () => {
  it("provides one immutable empty work summary shape", () => {
    expect(getProjectWorkSummaryOrEmpty(null)).toBe(EMPTY_PROJECT_WORK_SUMMARY);
    expect(hasProjectWorkSummaryActivity(undefined)).toBe(false);
  });

  it("detects activity across every work-summary count", () => {
    const countKeys = [
      "unresolvedCount",
      "newCount",
      "workingCount",
      "regressedCount",
      "ignoredCount",
      "blockedCount",
      "launchBlockerCount",
      "maintenanceCount",
    ] as const;

    for (const key of countKeys) {
      expect(
        hasProjectWorkSummaryActivity({
          ...EMPTY_PROJECT_WORK_SUMMARY,
          [key]: 1,
        }),
      ).toBe(true);
    }
  });

  it("deduplicates launch blockers from actionable issue totals", () => {
    expect(
      getProjectWorkSummaryIssueTotal({
        unresolvedCount: 12,
        blockedCount: 2,
        launchBlockerCount: 3,
      }),
    ).toBe(11);
  });
});
