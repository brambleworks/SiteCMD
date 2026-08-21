import { describe, expect, it } from "vitest";

import { buildProjectNavBadgeState, buildUpdatesBadgeFromReport } from "./project-nav-badges";

function canonicalWorkSummary(
  overrides: Partial<{
    issueCount: number;
    issueWebCount: number;
    issueCodeCount: number;
    issueCriticalCount: number;
    issueHighCount: number;
    issueMediumCount: number;
    issueLowCount: number;
  }>,
) {
  return {
    issueCount: 0,
    issueWebCount: 0,
    issueCodeCount: 0,
    issueCriticalCount: 0,
    issueHighCount: 0,
    issueMediumCount: 0,
    issueLowCount: 0,
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

describe("project nav badges", () => {
  it("builds sidebar badges from the shared dashboard snapshot", () => {
    const state = buildProjectNavBadgeState(7, {
      projectId: 7,
      environmentUrl: "https://example.com",
      aggregatedFailedIssues: [
        { checkId: "visible-critical", severity: "critical" } as never,
        { checkId: "dismissed-high", severity: "high" } as never,
      ],
      inactiveCheckIds: ["dismissed-high"],
      signals: {
        projectId: 7,
        environmentUrl: "https://example.com",
        firstScanBannerDismissed: false,
        updates: {
          updates: [{ isSecurity: true } as never, { isSecurity: false } as never],
        } as never,
        updatesRefreshedAt: null,
        codeScanSummary: {
          issueCount: 3,
          criticalCount: 1,
          highCount: 2,
          topDomainCount: 2,
        } as never,
        previousCodeScanSummary: null,
        codeScanDetail: null,
        monitoring: {
          // Drives the progressive sidebar pages; must pass straight through.
          enabledIntegrations: ["plausible", "github"],
          integrationFailureCount: 0,
          staleIntegrationCount: 0,
          searchRegression: null,
        },
        monitoringRefreshedAt: null,
        targets: {
          securityIssueId: null,
          securityFocus: null,
        },
        workSummary: canonicalWorkSummary({
          issueCount: 4,
          issueWebCount: 1,
          issueCodeCount: 3,
          issueCriticalCount: 2,
          issueHighCount: 2,
        }),
      },
    });

    expect(state).toEqual({
      updates: {
        projectId: 7,
        total: 2,
        critical: 1,
      },
      issues: {
        totalCount: 4,
        criticalCount: 2,
      },
      enabledIntegrations: ["plausible", "github"],
    });
  });

  it("uses canonical grouped counts so the sidebar matches the Issues list", () => {
    const state = buildProjectNavBadgeState(7, {
      projectId: 7,
      environmentUrl: "https://example.com",
      aggregatedFailedIssues: [],
      inactiveCheckIds: [],
      signals: {
        projectId: 7,
        environmentUrl: "https://example.com",
        firstScanBannerDismissed: false,
        updates: null,
        updatesRefreshedAt: null,
        codeScanSummary: {
          issueCount: 59,
          groupedIssueCount: 37,
          criticalCount: 3,
          highCount: 12,
          topDomainCount: 8,
        } as never,
        previousCodeScanSummary: null,
        codeScanDetail: null,
        monitoring: {
          enabledIntegrations: [],
          integrationFailureCount: 0,
          staleIntegrationCount: 0,
          searchRegression: null,
        },
        monitoringRefreshedAt: null,
        targets: {
          securityIssueId: null,
          securityFocus: null,
        },
        workSummary: canonicalWorkSummary({
          issueCount: 37,
          issueCodeCount: 37,
          issueCriticalCount: 3,
          issueHighCount: 12,
          issueMediumCount: 22,
        }),
      },
    });

    expect(state.issues.totalCount).toBe(37);
  });

  it("uses the lifecycle-filtered backend projection even when scan detail contains blocked rows", () => {
    const state = buildProjectNavBadgeState(7, {
      projectId: 7,
      environmentUrl: "https://example.com",
      aggregatedFailedIssues: [],
      // The blocked code issue's canonical check_id. Blocking writes this to
      // project_issue_states; the sidebar must drop it just like the list does.
      inactiveCheckIds: ["code_scan.blocked-issue"],
      signals: {
        projectId: 7,
        environmentUrl: "https://example.com",
        firstScanBannerDismissed: false,
        updates: null,
        updatesRefreshedAt: null,
        codeScanSummary: {
          issueCount: 2,
          groupedIssueCount: 2,
          criticalCount: 1,
          highCount: 1,
          topDomainCount: 1,
        } as never,
        previousCodeScanSummary: null,
        codeScanDetail: {
          issues: [
            {
              checkId: "code_scan.active-issue",
              severity: "critical",
              domain: "security",
              title: "Active issue",
            } as never,
            {
              checkId: "code_scan.blocked-issue",
              severity: "high",
              domain: "security",
              title: "Blocked issue",
            } as never,
          ],
        } as never,
        monitoring: {
          enabledIntegrations: [],
          integrationFailureCount: 0,
          staleIntegrationCount: 0,
          searchRegression: null,
        },
        monitoringRefreshedAt: null,
        targets: {
          securityIssueId: null,
          securityFocus: null,
        },
        workSummary: canonicalWorkSummary({
          issueCount: 1,
          issueCodeCount: 1,
          issueCriticalCount: 1,
        }),
      },
    });

    // Only the active code issue is counted; the blocked one is dropped even
    // though it is still present in the scan detail.
    expect(state.issues.totalCount).toBe(1);
    expect(state.issues.criticalCount).toBe(1);
  });

  it("returns no updates badge when the report queue is empty", () => {
    expect(buildUpdatesBadgeFromReport(7, { updates: [] } as never)).toBeNull();
  });
});
