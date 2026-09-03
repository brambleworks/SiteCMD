import { describe, expect, it, beforeEach } from "vitest";
import {
  invalidateLatestCodeScanSnapshot,
  mergePrimedCodeScanForAccess,
  primeLatestCodeScanSnapshot,
} from "./project-summary-code-scan";
import type { ProjectSignalSnapshot } from "./project-summary-types";
import type { CodeScanResult } from "./types";

const MAX_PRIMED_CODE_SCAN_SNAPSHOTS = 5;

function result(projectId: number, id: number, checkedAt: string): CodeScanResult {
  // issueCount 0 with an empty issues array signals a fully detailed
  // (zero-issue) payload, so the primed cache keeps `result` rather than
  // collapsing it to a summary-only entry.
  return {
    id,
    projectId,
    environmentUrl: "https://example.com",
    overallScore: 80,
    issueCount: 0,
    criticalCount: 0,
    highCount: 0,
    mediumCount: 0,
    lowCount: 0,
    durationMs: 100,
    checkedAt,
    framework: null,
    domainSummaries: [],
    issues: [],
  };
}

function snapshot(
  projectId: number,
  overrides: Partial<ProjectSignalSnapshot> = {},
): ProjectSignalSnapshot {
  return {
    projectId,
    environmentUrl: "https://example.com",
    firstScanBannerDismissed: false,
    codeScanSummary: null,
    previousCodeScanSummary: null,
    codeScanDetail: null,
    monitoring: {
      enabledIntegrations: [],
      integrationFailureCount: 0,
      staleIntegrationCount: 0,
      searchRegression: null,
    },
    monitoringRefreshedAt: null,
    updates: null,
    updatesRefreshedAt: null,
    targets: {
      securityIssueId: null,
      securityFocus: null,
    },
    workSummary: {
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
    },
    ...overrides,
  };
}

// Reads a project's primed detail without consuming a persisted summary, so
// the primed report always wins when it is still cached.
function readPrimedDetail(projectId: number): CodeScanResult | null {
  return mergePrimedCodeScanForAccess(snapshot(projectId), true).codeScanDetail;
}

describe("primed code scan snapshot cache", () => {
  beforeEach(() => {
    // The primed cache is a module-level singleton; clear every id a test
    // might have touched so runs stay isolated.
    for (let projectId = 1; projectId <= 20; projectId += 1) {
      invalidateLatestCodeScanSnapshot(projectId);
    }
  });

  it("bounds the number of cached reports", () => {
    const total = MAX_PRIMED_CODE_SCAN_SNAPSHOTS + 3;
    for (let projectId = 1; projectId <= total; projectId += 1) {
      primeLatestCodeScanSnapshot(result(projectId, projectId, "2026-04-10T00:00:00Z"));
    }

    const cachedCount = Array.from({ length: total }, (_, index) => index + 1).filter(
      (projectId) => readPrimedDetail(projectId) != null,
    ).length;

    expect(cachedCount).toBe(MAX_PRIMED_CODE_SCAN_SNAPSHOTS);
  });

  it("evicts the least recently used entry, accounting for reads", () => {
    for (let projectId = 1; projectId <= MAX_PRIMED_CODE_SCAN_SNAPSHOTS; projectId += 1) {
      primeLatestCodeScanSnapshot(result(projectId, projectId, "2026-04-10T00:00:00Z"));
    }

    // Reading project 1 marks it most recently used, so it should survive
    // the next eviction even though it was primed first.
    expect(readPrimedDetail(1)?.id).toBe(1);

    const overflowProjectId = MAX_PRIMED_CODE_SCAN_SNAPSHOTS + 1;
    primeLatestCodeScanSnapshot(
      result(overflowProjectId, overflowProjectId, "2026-04-11T00:00:00Z"),
    );

    // Project 2 was the next-oldest entry after the read touched project 1,
    // so it is the one evicted, not project 1.
    expect(readPrimedDetail(2)).toBeNull();
    expect(readPrimedDetail(1)?.id).toBe(1);
    expect(readPrimedDetail(overflowProjectId)?.id).toBe(overflowProjectId);
  });

  it("drops a project's cached report when the project is deleted", () => {
    primeLatestCodeScanSnapshot(result(9, 9, "2026-04-10T00:00:00Z"));
    expect(readPrimedDetail(9)?.id).toBe(9);

    invalidateLatestCodeScanSnapshot(9);

    expect(readPrimedDetail(9)).toBeNull();
  });

  it("drops the cached report once persisted scan data supersedes it", () => {
    primeLatestCodeScanSnapshot(result(3, 5, "2026-04-10T00:00:00Z"));
    expect(readPrimedDetail(3)?.id).toBe(5);

    // A read whose persisted summary is fresher than the primed report
    // should both prefer the persisted data and evict the stale cache entry.
    const fresherSnapshot = snapshot(3, {
      codeScanSummary: {
        id: 12,
        projectId: 3,
        environmentUrl: "https://example.com",
        overallScore: 90,
        issueCount: 1,
        groupedIssueCount: 1,
        criticalCount: 0,
        highCount: 0,
        durationMs: 100,
        checkedAt: "2026-04-15T00:00:00Z",
        framework: null,
        topDomain: null,
        topDomainCount: 0,
        domainSummaries: [],
      },
      codeScanDetail: null,
    });
    const merged = mergePrimedCodeScanForAccess(fresherSnapshot, true);
    expect(merged.codeScanDetail).toBeNull();
    expect(merged.codeScanSummary?.id).toBe(12);

    // A later read with an older persisted summary would normally lose to a
    // still-cached primed report; it stays null here because supersession
    // already evicted the entry above, proving it was actually dropped.
    const olderSnapshot = snapshot(3, {
      codeScanSummary: {
        id: 1,
        projectId: 3,
        environmentUrl: "https://example.com",
        overallScore: 70,
        issueCount: 1,
        groupedIssueCount: 1,
        criticalCount: 0,
        highCount: 0,
        durationMs: 100,
        checkedAt: "2026-04-05T00:00:00Z",
        framework: null,
        topDomain: null,
        topDomainCount: 0,
        domainSummaries: [],
      },
      codeScanDetail: null,
    });
    expect(mergePrimedCodeScanForAccess(olderSnapshot, true).codeScanDetail).toBeNull();
  });

  it("behaves identically on a hit, a miss, and after eviction", () => {
    const missSnapshot = snapshot(4);
    expect(mergePrimedCodeScanForAccess(missSnapshot, true)).toEqual(missSnapshot);

    primeLatestCodeScanSnapshot(result(4, 7, "2026-04-10T00:00:00Z"));
    const hit = mergePrimedCodeScanForAccess(snapshot(4), true);
    expect(hit.codeScanDetail?.id).toBe(7);

    invalidateLatestCodeScanSnapshot(4);
    expect(mergePrimedCodeScanForAccess(snapshot(4), true)).toEqual(missSnapshot);
  });
});
