import { describe, expect, it, vi, beforeEach } from "vitest";

const { rawInvokeMock } = vi.hoisted(() => ({
  rawInvokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => rawInvokeMock(...args),
}));

import {
  getDashboardSnapshot,
  getDashboardReferenceSignals,
  getProjectSignalSnapshot,
  invalidateLatestCodeScanSnapshot,
  peekDashboardSnapshot,
  peekDashboardReferenceSignals,
  primeLatestCodeScanSnapshot,
  primeProjectUpdatesSnapshot,
  shouldPreferPrimed,
} from "./project-summary-signals";
import type {
  DashboardReferenceSignals,
  DashboardSnapshot,
  LatestCodeScanSnapshot,
  ProjectSignalSnapshot,
} from "./project-summary-signals";
import { snapshotCacheKey } from "./project-summary-cache";
import { createTestQueryClient } from "@/test-utils/query-client";
import { queryKeys } from "@/lib/query/query-keys";
import type {
  CodeScanResult,
  CodeScanSummary,
  PackageUpdate,
  ScanResult,
  UpdateReport,
} from "./types";

function summary(id: number, checkedAt: string): CodeScanSummary {
  return {
    id,
    projectId: 1,
    environmentUrl: "https://example.com",
    overallScore: 80,
    issueCount: 5,
    groupedIssueCount: 5,
    criticalCount: 0,
    highCount: 1,
    durationMs: 100,
    checkedAt,
    framework: null,
    topDomain: null,
    topDomainCount: 0,
    domainSummaries: [],
  };
}

function result(id: number, checkedAt: string): CodeScanResult {
  return {
    id,
    projectId: 1,
    environmentUrl: "https://example.com",
    overallScore: 80,
    issueCount: 5,
    criticalCount: 0,
    highCount: 1,
    mediumCount: 1,
    lowCount: 3,
    durationMs: 100,
    checkedAt,
    framework: null,
    domainSummaries: [],
    issues: [],
  };
}

function snapshot(overrides: Partial<ProjectSignalSnapshot> = {}): ProjectSignalSnapshot {
  return {
    projectId: 1,
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

function packageUpdate(name: string): PackageUpdate {
  return {
    name,
    currentVersion: "1.0.0",
    latestVersion: "2.0.0",
    ecosystem: "npm",
    updateType: "major",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "package-lock.json",
    isDev: false,
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    workspaceMembers: [],
  };
}

function updateReport(packageNames: string[]): UpdateReport {
  return {
    packages: [],
    updates: packageNames.map(packageUpdate),
    ecosystemsDetected: ["npm"],
    scanDurationMs: 100,
  };
}

function dashboardSnapshot(overrides: Partial<ProjectSignalSnapshot> = {}): DashboardSnapshot {
  return {
    projectId: 1,
    environmentUrl: "https://example.com",
    trend: [],
    codeTrend: [],
    latestScanId: null,
    latestDetail: null,
    previousDetail: null,
    aggregatedCheckCounts: { passed: 0, total: 0, failed: 0 },
    aggregatedFailedIssues: [],
    commitsSinceLastScan: [],
    issueLinks: [],
    inactiveCheckIds: [],
    signals: snapshot(overrides),
    workQueue: {
      resumeNow: [],
      verifyNow: [],
      fixNext: [],
      maintenance: [],
    },
  };
}

function webScanResult(marker: string): ScanResult {
  return {
    url: "https://example.com",
    mode: "live",
    scanType: "health",
    overallScore: 88,
    categories: [],
    issues: [
      {
        checkId: "seo.title",
        category: "seo",
        title: marker,
        description: "",
        status: "fail",
        severity: "high",
        fixPrompt: null,
        manualFix: null,
        rawData: { marker },
        confidence: "high",
      },
    ],
    detectedStack: null,
    durationMs: 1200,
    timestamp: "2026-05-19T12:00:00Z",
  };
}

function primed(
  summaryOverrides: Partial<CodeScanSummary> = {},
  resultOverrides: Partial<CodeScanResult> = {},
): LatestCodeScanSnapshot {
  const base = summary(1, "2026-04-10T00:00:00Z");
  const baseResult = result(1, "2026-04-10T00:00:00Z");
  return {
    summary: { ...base, ...summaryOverrides },
    result: { ...baseResult, ...resultOverrides },
  };
}

describe("shouldPreferPrimed", () => {
  let queryClient: ReturnType<typeof createTestQueryClient>;

  beforeEach(() => {
    rawInvokeMock.mockReset();
    invalidateLatestCodeScanSnapshot(1);
    queryClient = createTestQueryClient();
    window.sessionStorage.clear();
  });

  it("returns false when primed is undefined", () => {
    expect(shouldPreferPrimed(undefined, snapshot())).toBe(false);
  });

  it("returns false when primed has no result", () => {
    const empty: LatestCodeScanSnapshot = { summary: null, result: null };
    expect(shouldPreferPrimed(empty, snapshot())).toBe(false);
  });

  it("can prefer a summary-only primed scan without full issue detail", () => {
    const summaryOnly: LatestCodeScanSnapshot = {
      summary: summary(10, "2026-04-18T00:00:00Z"),
      result: null,
    };
    const snap = snapshot({
      codeScanSummary: summary(5, "2026-04-10T00:00:00Z"),
    });

    expect(shouldPreferPrimed(summaryOnly, snap)).toBe(true);
  });

  it("prefers primed when snapshot has no codeScanSummary AND no codeScanDetail (fresh scan path)", () => {
    // Snapshot is empty (no scans yet in DB); primed was just loaded from a
    // freshly-run scan. Primed should win.
    expect(shouldPreferPrimed(primed(), snapshot())).toBe(true);
  });

  it("prefers primed when snapshot has only a codeScanSummary that's older", () => {
    const snap = snapshot({
      codeScanSummary: summary(5, "2026-04-05T00:00:00Z"),
    });
    expect(shouldPreferPrimed(primed({}, {}), snap)).toBe(true);
  });

  it("prefers snapshot when the DB summary is fresher than the primed result", () => {
    const snap = snapshot({
      codeScanSummary: summary(9, "2026-04-15T00:00:00Z"),
    });
    expect(shouldPreferPrimed(primed(), snap)).toBe(false);
  });

  it("uses codeScanDetail as tiebreak when codeScanSummary is absent", () => {
    const snap = snapshot({
      codeScanDetail: result(9, "2026-04-15T00:00:00Z"),
    });
    expect(shouldPreferPrimed(primed(), snap)).toBe(false);
  });

  it("falls back to id comparison when both timestamps are unparseable", () => {
    const snap = snapshot({
      codeScanSummary: summary(3, "not-a-date"),
    });
    const primedOlder = primed({ id: 1 }, { id: 1, checkedAt: "also-bad" });
    const primedNewer = primed({ id: 10 }, { id: 10, checkedAt: "also-bad" });
    expect(shouldPreferPrimed(primedOlder, snap)).toBe(false);
    expect(shouldPreferPrimed(primedNewer, snap)).toBe(true);
  });

  it("passes includeCodeScanDetail through to the backend snapshot command", async () => {
    rawInvokeMock.mockResolvedValue(snapshot());

    await getProjectSignalSnapshot(7, "https://example.com", {
      forceRefresh: true,
      includeCodeScanDetail: false,
    });

    expect(rawInvokeMock).toHaveBeenCalledWith("get_project_signal_snapshot", {
      projectId: 7,
      url: "https://example.com",
      forceRefresh: true,
      includeCodeScanDetail: false,
    });
  });

  it("does not expose summary-only primed code scans as empty issue detail", async () => {
    rawInvokeMock.mockResolvedValue(
      snapshot({
        codeScanSummary: summary(4, "2026-04-10T00:00:00Z"),
        codeScanDetail: result(4, "2026-04-10T00:00:00Z"),
      }),
    );
    primeLatestCodeScanSnapshot({
      ...result(9, "2026-04-18T00:00:00Z"),
      issueCount: 5,
      issues: [],
      domainSummaries: [
        {
          domain: "security",
          issueCount: 5,
          criticalCount: 1,
          highCount: 1,
          mediumCount: 3,
          lowCount: 0,
        },
      ],
    });

    const snap = await getProjectSignalSnapshot(1, "https://example.com", {
      forceRefresh: true,
      includeCodeScanDetail: true,
    });

    expect(snap.codeScanSummary?.id).toBe(9);
    expect(snap.codeScanDetail).toBeNull();
  });

  it("primes cached dashboard snapshots with the fresh Updates report", async () => {
    rawInvokeMock.mockResolvedValue(
      dashboardSnapshot({
        updates: updateReport(["old-package"]),
        updatesRefreshedAt: "2026-05-19T12:00:00Z",
      }),
    );

    await getDashboardSnapshot(queryClient, 141, "https://updates.example");
    primeProjectUpdatesSnapshot(
      queryClient,
      141,
      "https://updates.example",
      updateReport(["next", "vite"]),
    );

    const cached = peekDashboardSnapshot(queryClient, 141, "https://updates.example");

    expect(cached?.signals.updates?.updates.map((update) => update.name)).toEqual(["next", "vite"]);
  });

  it("treats an invalidated dashboard snapshot as a cache miss", async () => {
    const oldSnapshot = { ...dashboardSnapshot(), latestScanId: 41 };
    const freshSnapshot = { ...dashboardSnapshot(), latestScanId: 42 };
    rawInvokeMock.mockResolvedValueOnce(oldSnapshot).mockResolvedValueOnce(freshSnapshot);

    expect(
      (await getDashboardSnapshot(queryClient, 141, "https://cache.example")).latestScanId,
    ).toBe(41);
    await queryClient.invalidateQueries({
      queryKey: queryKeys.projectSummary.all,
      refetchType: "none",
    });

    const refreshed = await getDashboardSnapshot(queryClient, 141, "https://cache.example");

    expect(refreshed.latestScanId).toBe(42);
    expect(rawInvokeMock).toHaveBeenCalledTimes(2);
    expect(
      window.sessionStorage.getItem(
        `sitecmd:dashboard-snapshot:${snapshotCacheKey(141, "https://cache.example")}`,
      ),
    ).toContain('"latestScanId":42');
  });

  it("strips heavy detail fields from the sessionStorage snapshot tier", async () => {
    const webMarker = "HEAVY_WEB_DETAIL_MARKER";
    const codeMarker = "HEAVY_CODE_DETAIL_MARKER";
    const fullSnapshot: DashboardSnapshot = {
      ...dashboardSnapshot({
        codeScanSummary: summary(4, "2026-04-10T00:00:00Z"),
        codeScanDetail: {
          ...result(4, "2026-04-10T00:00:00Z"),
          issues: [
            {
              id: "raw-sql:src/a.ts",
              checkId: "code_scan.raw-sql",
              category: "security",
              domain: "security",
              severity: "high",
              title: codeMarker,
              description: "",
              relativePath: "src/a.ts",
              absolutePath: "/tmp/src/a.ts",
              line: 1,
              sourceExcerpt: null,
              evidence: null,
              whyNow: null,
              likelyFix: null,
              confidence: "high",
              verifyHint: null,
            },
          ],
        },
      }),
      latestScanId: 12,
      latestDetail: webScanResult(webMarker),
      previousDetail: webScanResult(webMarker),
    };
    rawInvokeMock.mockResolvedValue(fullSnapshot);

    const fetched = await getDashboardSnapshot(queryClient, 931, "https://slim.example");

    // Callers and the in-memory tier keep the full payload.
    expect(fetched.latestDetail?.issues[0]?.title).toBe(webMarker);
    expect(rawInvokeMock).toHaveBeenCalledTimes(1);
    const cached = await getDashboardSnapshot(queryClient, 931, "https://slim.example");
    expect(cached.latestDetail?.issues[0]?.title).toBe(webMarker);
    expect(cached.signals.codeScanDetail?.issues[0]?.title).toBe(codeMarker);
    expect(rawInvokeMock).toHaveBeenCalledTimes(1);

    // The sessionStorage tier must never carry the heavy detail payloads.
    const raw = window.sessionStorage.getItem(
      `sitecmd:dashboard-snapshot:${snapshotCacheKey(931, "https://slim.example")}`,
    );
    expect(raw).toBeTruthy();
    expect(raw).not.toContain(webMarker);
    expect(raw).not.toContain(codeMarker);
    const persisted = JSON.parse(raw!) as {
      snapshot: DashboardSnapshot;
      partial?: boolean;
    };
    expect(persisted.partial).toBe(true);
    expect(persisted.snapshot.latestDetail).toBeNull();
    expect(persisted.snapshot.previousDetail).toBeNull();
    expect(persisted.snapshot.signals.codeScanDetail).toBeNull();
    // The summary fields the dashboard skeleton needs survive the slimming.
    expect(persisted.snapshot.latestScanId).toBe(12);
    expect(persisted.snapshot.signals.codeScanSummary?.id).toBe(4);
  });

  it("accepts a restored slimmed session snapshot and refetches the full payload once", async () => {
    const slimmedSnapshot: DashboardSnapshot = {
      ...dashboardSnapshot({
        codeScanSummary: summary(6, "2026-04-10T00:00:00Z"),
      }),
      latestScanId: 21,
    };
    // Simulate a fresh app session: nothing in memory, only the slimmed
    // session entry written by a previous session survives.
    window.sessionStorage.setItem(
      `sitecmd:dashboard-snapshot:${snapshotCacheKey(932, "https://restore.example")}`,
      JSON.stringify({ snapshot: slimmedSnapshot, cachedAt: Date.now(), partial: true }),
    );

    // The peek path accepts the slimmed snapshot for instant paint.
    const peeked = peekDashboardSnapshot(queryClient, 932, "https://restore.example");
    expect(peeked?.latestScanId).toBe(21);
    expect(peeked?.latestDetail).toBeNull();
    expect(peeked?.signals.codeScanSummary?.id).toBe(6);

    // The authoritative read sees the partial entry and refetches over IPC.
    const fullSnapshot: DashboardSnapshot = {
      ...dashboardSnapshot({
        codeScanSummary: summary(6, "2026-04-10T00:00:00Z"),
      }),
      latestScanId: 21,
      latestDetail: webScanResult("restored-full-detail"),
    };
    rawInvokeMock.mockResolvedValue(fullSnapshot);
    const fetched = await getDashboardSnapshot(queryClient, 932, "https://restore.example");
    expect(rawInvokeMock).toHaveBeenCalledWith(
      "get_dashboard_snapshot",
      expect.objectContaining({ projectId: 932 }),
    );
    expect(fetched.latestDetail?.issues[0]?.title).toBe("restored-full-detail");

    // The refetch replaced the partial entry, so the next read is cache-only.
    rawInvokeMock.mockClear();
    const cached = await getDashboardSnapshot(queryClient, 932, "https://restore.example");
    expect(cached.latestDetail?.issues[0]?.title).toBe("restored-full-detail");
    expect(rawInvokeMock).not.toHaveBeenCalled();
  });

  it("keeps backend grouped issue counts when merging a summary-only primed code scan", async () => {
    rawInvokeMock.mockResolvedValue(
      snapshot({
        codeScanSummary: {
          ...summary(9, "2026-04-18T00:00:00Z"),
          issueCount: 59,
          groupedIssueCount: 37,
          topDomain: "security",
          topDomainCount: 12,
        },
        codeScanDetail: null,
      }),
    );
    primeLatestCodeScanSnapshot({
      ...result(9, "2026-04-18T00:00:00Z"),
      issueCount: 59,
      issues: [],
      domainSummaries: [],
    });

    const snap = await getProjectSignalSnapshot(1, "https://example.com", {
      forceRefresh: true,
      includeCodeScanDetail: false,
    });

    expect(snap.codeScanSummary?.issueCount).toBe(59);
    expect(snap.codeScanSummary?.groupedIssueCount).toBe(37);
    expect(snap.codeScanSummary?.topDomain).toBe("security");
    expect(snap.codeScanDetail).toBeNull();
  });

  it("takes backend active critical/high counts when merging a summary-only primed code scan", async () => {
    // Active grouped severity counts must replace raw scan counts together.
    rawInvokeMock.mockResolvedValue(
      snapshot({
        codeScanSummary: {
          ...summary(9, "2026-04-18T00:00:00Z"),
          issueCount: 40,
          groupedIssueCount: 2,
          criticalCount: 0,
          highCount: 0,
        },
        codeScanDetail: null,
      }),
    );
    primeLatestCodeScanSnapshot({
      ...result(9, "2026-04-18T00:00:00Z"),
      issueCount: 40,
      criticalCount: 2,
      highCount: 26,
      issues: [],
      domainSummaries: [],
    });

    const snap = await getProjectSignalSnapshot(1, "https://example.com", {
      forceRefresh: true,
      includeCodeScanDetail: false,
    });

    expect(snap.codeScanSummary?.groupedIssueCount).toBe(2);
    expect(snap.codeScanSummary?.criticalCount).toBe(0);
    expect(snap.codeScanSummary?.highCount).toBe(0);
  });

  it("groups primed code scan issue detail when the response includes duplicated locations", async () => {
    rawInvokeMock.mockResolvedValue(snapshot());
    primeLatestCodeScanSnapshot({
      ...result(11, "2026-04-18T00:00:00Z"),
      issueCount: 2,
      issues: [
        {
          id: "raw-sql",
          checkId: "code_scan.raw-sql",
          category: "security",
          domain: "security",
          severity: "high",
          title: "Unsafe raw SQL query",
          description: "",
          relativePath: "src/a.ts",
          absolutePath: "/tmp/src/a.ts",
          line: 1,
          sourceExcerpt: null,
          evidence: null,
          whyNow: null,
          likelyFix: null,
          confidence: "high",
          verifyHint: null,
        },
        {
          id: "raw-sql",
          checkId: "code_scan.raw-sql",
          category: "security",
          domain: "security",
          severity: "high",
          title: "Unsafe raw SQL query",
          description: "",
          relativePath: "src/b.ts",
          absolutePath: "/tmp/src/b.ts",
          line: 1,
          sourceExcerpt: null,
          evidence: null,
          whyNow: null,
          likelyFix: null,
          confidence: "high",
          verifyHint: null,
        },
      ],
    });

    const snap = await getProjectSignalSnapshot(1, "https://example.com", {
      forceRefresh: true,
      includeCodeScanDetail: true,
    });

    expect(snap.codeScanSummary?.issueCount).toBe(2);
    expect(snap.codeScanSummary?.groupedIssueCount).toBe(1);
  });

  it("loads dashboard reference signals through the dedicated command", async () => {
    const referenceSignals: DashboardReferenceSignals = {
      integrations: [],
      lastCiRun: null,
      psiReport: null,
    };
    rawInvokeMock.mockResolvedValue(referenceSignals);

    await getDashboardReferenceSignals(queryClient, 101, "https://example.com");

    expect(rawInvokeMock).toHaveBeenCalledWith("get_dashboard_reference_signals", {
      projectId: 101,
      url: "https://example.com",
      includePsi: false,
    });
  });

  it("can opt into PageSpeed when loading dashboard reference signals", async () => {
    const referenceSignals: DashboardReferenceSignals = {
      integrations: [],
      lastCiRun: null,
      psiReport: null,
    };
    rawInvokeMock.mockResolvedValue(referenceSignals);

    await getDashboardReferenceSignals(queryClient, 102, "https://example.com", {
      includePsi: true,
    });

    expect(rawInvokeMock).toHaveBeenCalledWith("get_dashboard_reference_signals", {
      projectId: 102,
      url: "https://example.com",
      includePsi: true,
    });
  });

  it("caches dashboard reference signals for repeat dashboard visits", async () => {
    const referenceSignals: DashboardReferenceSignals = {
      integrations: [
        {
          integrationType: "plausible",
          data: { visitors: 42 },
          fetchedAt: "2026-04-20T16:08:00Z",
          error: null,
        },
      ],
      lastCiRun: null,
      psiReport: null,
    };
    rawInvokeMock.mockResolvedValue(referenceSignals);

    await getDashboardReferenceSignals(queryClient, 103, "https://example.com");
    await getDashboardReferenceSignals(queryClient, 103, "https://example.com/");

    expect(rawInvokeMock).toHaveBeenCalledTimes(1);
    expect(peekDashboardReferenceSignals(queryClient, 103, "https://example.com/")).toEqual(
      referenceSignals,
    );
  });

  it("refetches invalidated dashboard reference signals instead of restoring session data", async () => {
    const oldSignals: DashboardReferenceSignals = {
      integrations: [],
      lastCiRun: null,
      psiReport: null,
    };
    const freshSignals: DashboardReferenceSignals = {
      integrations: [
        {
          integrationType: "plausible",
          data: { visitors: 84 },
          fetchedAt: "2026-04-20T16:09:00Z",
          error: null,
        },
      ],
      lastCiRun: null,
      psiReport: null,
    };
    rawInvokeMock.mockResolvedValueOnce(oldSignals).mockResolvedValueOnce(freshSignals);

    await getDashboardReferenceSignals(queryClient, 106, "https://example.com");
    await queryClient.invalidateQueries({
      queryKey: queryKeys.projectSummary.all,
      refetchType: "none",
    });

    const refreshed = await getDashboardReferenceSignals(queryClient, 106, "https://example.com");

    expect(rawInvokeMock).toHaveBeenCalledTimes(2);
    expect(refreshed.integrations[0]?.data).toEqual({ visitors: 84 });
  });

  it("can bypass cached dashboard reference signals", async () => {
    const cachedSignals: DashboardReferenceSignals = {
      integrations: [
        {
          integrationType: "plausible",
          data: { visitors: 42 },
          fetchedAt: "2026-04-20T16:08:00Z",
          error: null,
        },
      ],
      lastCiRun: null,
      psiReport: null,
    };
    const freshSignals: DashboardReferenceSignals = {
      integrations: [
        {
          integrationType: "plausible",
          data: { visitors: 84 },
          fetchedAt: "2026-04-20T16:09:00Z",
          error: null,
        },
      ],
      lastCiRun: null,
      psiReport: null,
    };
    rawInvokeMock.mockResolvedValueOnce(cachedSignals).mockResolvedValueOnce(freshSignals);

    await getDashboardReferenceSignals(queryClient, 104, "https://example.com");
    const fresh = await getDashboardReferenceSignals(queryClient, 104, "https://example.com", {
      bypassCache: true,
    });

    expect(rawInvokeMock).toHaveBeenCalledTimes(2);
    expect(fresh.integrations[0]?.data).toEqual({ visitors: 84 });
  });

  it("does not cache failed dashboard integration reads", async () => {
    const failedSignals: DashboardReferenceSignals = {
      integrations: [
        {
          integrationType: "plausible",
          data: null as unknown as Record<string, unknown>,
          fetchedAt: "2026-04-20T16:08:00Z",
          error: "Plausible site not found",
        },
      ],
      lastCiRun: null,
      psiReport: null,
    };
    const recoveredSignals: DashboardReferenceSignals = {
      integrations: [
        {
          integrationType: "plausible",
          data: { visitors: 21 },
          fetchedAt: "2026-04-20T16:09:00Z",
          error: null,
        },
      ],
      lastCiRun: null,
      psiReport: null,
    };
    rawInvokeMock.mockResolvedValueOnce(failedSignals).mockResolvedValueOnce(recoveredSignals);

    await getDashboardReferenceSignals(queryClient, 105, "https://example.com");
    const recovered = await getDashboardReferenceSignals(queryClient, 105, "https://example.com");

    expect(rawInvokeMock).toHaveBeenCalledTimes(2);
    expect(recovered.integrations[0]?.data).toEqual({ visitors: 21 });
  });
});
