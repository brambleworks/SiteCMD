import { describe, expect, it } from "vitest";

import {
  buildWorkflowFollowUpBanner,
  getPrimaryWorkSummaryCue,
  getWorkSummaryBadgeTarget,
  getWorkSummaryFollowUpBanner,
  readPersistedWorkSummaryCue,
} from "@/lib/work-item-presentation";
import type { ProjectWorkSummary } from "@/lib/project-summary-signals";

function buildSummary(overrides: Partial<ProjectWorkSummary> = {}): ProjectWorkSummary {
  return {
    unresolvedCount: 0,
    newCount: 0,
    workingCount: 0,
    regressedCount: 0,
    ignoredCount: 0,
    blockedCount: 0,
    launchBlockerCount: 0,
    maintenanceCount: 0,
    primaryAction: null,
    regressedAction: null,
    workingAction: null,
    blockedAction: null,
    ignoredAction: null,
    launchBlockerAction: null,
    weeklySummary: null,
    ...overrides,
  };
}

describe("work-item presentation helpers", () => {
  it("prioritizes regressed work over other statuses", () => {
    const summary = buildSummary({
      regressedCount: 2,
      workingCount: 1,
      regressedAction: {
        stableKey: "web:seo",
        projectId: 7,
        environmentUrl: "https://example.com",
        kind: "web",
        status: "regressed",
        severity: "critical",
        title: "Robots.txt regressed",
        summary: "Robots directives regressed after deploy",
        category: "seo",
        domain: null,
        packageName: null,
        target: { page: "search-console", projectId: 7, url: "https://example.com" },
        firstSeenAt: "2026-04-10T12:00:00Z",
        lastSeenAt: "2026-04-11T12:00:00Z",
        lastVerifiedAt: "2026-04-10T13:00:00Z",
        lastStatusChangedAt: "2026-04-11T12:00:00Z",
      },
    });

    const cue = getPrimaryWorkSummaryCue(summary);
    expect(cue?.key).toBe("regressed");
    expect(cue?.label).toBe("2 regressed");
    expect(cue?.sentence).toContain("Resume 2 regressed");
    expect(cue?.target?.page).toBe("search-console");
  });

  it("maps blocked badges to the representative blocked target", () => {
    const blockedTarget = {
      page: "updates",
      projectId: 9,
      url: "https://blocked.example",
    } as const;
    const summary = buildSummary({
      blockedCount: 1,
      blockedAction: {
        stableKey: "update:react",
        projectId: 9,
        environmentUrl: "https://blocked.example",
        kind: "update",
        status: "blocked",
        severity: "high",
        title: "React upgrade blocked",
        summary: "Release depends on deciding the React upgrade path",
        category: null,
        domain: null,
        packageName: "react",
        target: blockedTarget,
        firstSeenAt: "2026-04-10T12:00:00Z",
        lastSeenAt: "2026-04-11T12:00:00Z",
        lastVerifiedAt: null,
        lastStatusChangedAt: "2026-04-11T12:00:00Z",
      },
    });

    expect(getWorkSummaryBadgeTarget(summary, "blocked")).toEqual(blockedTarget);
  });

  it("reads persisted workflow cues from event detail payloads", () => {
    expect(
      readPersistedWorkSummaryCue({
        workflow_key: "blocked",
        workflow_label: "1 blocked",
        workflow_sentence: "1 blocked item needs a decision.",
      }),
    ).toEqual({
      key: "blocked",
      label: "1 blocked",
      sentence: "1 blocked item needs a decision.",
    });
  });

  it("builds a resume banner for urgent work states", () => {
    const summary = buildSummary({
      workingCount: 1,
      workingAction: {
        stableKey: "web:seo",
        projectId: 7,
        environmentUrl: "https://example.com",
        kind: "web",
        status: "working",
        severity: "high",
        title: "Meta description fix in progress",
        summary: "Resume the SEO fix and verify it.",
        category: "seo",
        domain: null,
        packageName: null,
        target: { page: "search-console", projectId: 7, url: "https://example.com" },
        firstSeenAt: "2026-04-10T12:00:00Z",
        lastSeenAt: "2026-04-11T12:00:00Z",
        lastVerifiedAt: null,
        lastStatusChangedAt: "2026-04-11T12:00:00Z",
      },
    });

    expect(getWorkSummaryFollowUpBanner(summary)).toMatchObject({
      title: "Pick up where you left off",
      actionLabel: "Open Search & SEO",
      tone: "followup",
      target: { page: "search-console", projectId: 7, url: "https://example.com" },
    });
  });

  it("marks regressed work banners as urgent", () => {
    expect(
      buildWorkflowFollowUpBanner({
        key: "regressed",
        sentence: "Resume 1 regressed item next.",
        target: { page: "issues", projectId: 7, url: "https://example.com" },
      }),
    ).toMatchObject({
      tone: "urgent",
    });
  });

  it("skips banners for blocked work states", () => {
    expect(
      buildWorkflowFollowUpBanner({
        key: "blocked",
        sentence: "1 blocked item needs a decision.",
        target: { page: "updates", projectId: 9, url: "https://blocked.example" },
      }),
    ).toBeNull();
  });
});
