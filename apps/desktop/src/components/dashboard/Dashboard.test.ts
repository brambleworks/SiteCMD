import React from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { withQueryClient } from "@/test-utils/query-client";

const {
  useDashboardDataMock,
  useTierMock,
  usePendingVerificationCenterMock,
  useDesktopPromptCenterMock,
} = vi.hoisted(() => ({
  useDashboardDataMock: vi.fn(),
  useTierMock: vi.fn(),
  usePendingVerificationCenterMock: vi.fn(),
  useDesktopPromptCenterMock: vi.fn(),
}));

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
vi.mock("@/hooks/useTier", () => ({
  useTier: () => useTierMock(),
}));
vi.mock("@/lib/open-url", () => ({
  openUrl: vi.fn(),
}));
vi.mock("@/lib/desktop-prompts", () => ({
  buildDesktopPromptTarget: vi.fn(),
  useDesktopPromptCenter: () => useDesktopPromptCenterMock(),
}));
vi.mock("@/lib/pending-verification", () => ({
  usePendingVerificationCenter: () => usePendingVerificationCenterMock(),
}));
vi.mock("@/lib/nav-badges", () => ({
  setUpdatesBadge: vi.fn(),
  clearUpdatesBadgeForProject: vi.fn(),
}));
vi.mock("./useDashboardData", () => ({
  useDashboardData: () => useDashboardDataMock(),
}));
vi.mock("./SinceLastScan", () => ({
  SinceLastScan: () => React.createElement("div", null, "SinceLastScan"),
}));
vi.mock("./DashboardComponents", () => ({
  WebIssueDossier: () => null,
  FirstScanBanner: () => null,
  IssueListOverlay: () => null,
  VitalCard: ({
    label,
    title,
    onClick,
  }: {
    label?: string;
    title?: string;
    onClick?: () => void;
  }) =>
    React.createElement(
      onClick ? "button" : "div",
      onClick ? { type: "button", onClick } : null,
      title ?? label ?? "VitalCard",
    ),
}));

import { Dashboard } from "./Dashboard";
import { getDesktopPromptAttentionMeta } from "@/lib/attention-targets";
import type { ScanResult } from "@/generated/ipc-bindings";
import type { ScoreTrendPoint } from "./DashboardTrendComponents";

describe("Dashboard rendering", () => {
  it("can rerender from setup state into latest-scan state without hook-order crashes", () => {
    useTierMock.mockReturnValue({
      hasFeature: vi.fn((feature?: string) => feature === "code_scan" || feature === "analytics"),
      licenseInfo: { checkoutUrls: { coreMonthly: "" } },
    });
    usePendingVerificationCenterMock.mockReturnValue([]);
    useDesktopPromptCenterMock.mockReturnValue([]);

    const trendPoint: ScoreTrendPoint = {
      overall: 81,
      security: 80,
      performance: 75,
      seo: 82,
      accessibility: 88,
      compliance: 90,
      config: 84,
      polish: 79,
      timestamp: "2026-04-11T12:00:00Z",
      issues: 3,
      scanType: "health",
    };
    const populatedDetail: ScanResult = {
      url: "https://example.com",
      mode: "live",
      scanType: "health",
      overallScore: 81,
      categories: [],
      issues: [],
      detectedStack: null,
      durationMs: 1200,
      timestamp: "2026-04-11T12:00:00Z",
    };

    let dashboardState: any = {
      trend: [] as ScoreTrendPoint[],
      codeTrend: [] as Array<{ score: number; timestamp: string }>,
      latestDetail: null,
      previousDetail: null,
      latestScanId: null,
      aggregatedCheckCounts: { passed: 0, total: 0, failed: 0 },
      aggregatedFailedIssues: [] as Array<unknown>,
      securityUpdates: [],
      allUpdates: [],
      integrations: [],
      configuredIntegrations: new Set<string>(),
      lastCIRun: null,
      commitsSinceLastScan: [],
      issueLinks: [],
      psiReport: null,
      dashboardReady: true,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      latestCodeScanSummary: null,
      previousCodeScanSummary: null,
      latestCodeScanDetail: null,
      updatesCheckedAt: null,
      searchRegression: null,
      integrationFailureCount: 0,
      staleIntegrationCount: 0,
      firstScanBannerDismissed: true,
      workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
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
      probesRefreshing: false,
      sslProbe: null,
      verdict: { kind: "healthy" as const, phrase: "Healthy", reasons: [] },
      criticalRollup: { total: 0, web: 0, code: 0, securityPatches: 0 },
      bootstrapTasks: [],
      criticalWebIssues: 0,
      criticalCodeIssues: 0,
      highWebIssues: 0,
      recentEvents: [],
      recentEventsLoading: false,
      dismissFirstScanBanner: vi.fn(),
      refreshDashboard: vi.fn(),
    };

    useDashboardDataMock.mockImplementation(() => dashboardState);

    const view = render(
      React.createElement(Dashboard, {
        url: "https://example.com",
        projectId: 7,
        projectName: "Example Site",
        framework: "Next.js",
        projectPath: "/tmp/example",
        onViewResults: vi.fn(),
        onViewCodeScan: vi.fn(),
        onRescan: vi.fn(),
        onOpenScanConfig: vi.fn(),
        onOpenCodeScanConfig: vi.fn(),
        onAddFolder: vi.fn(),
        onNavigate: vi.fn(),
        onOpenTarget: vi.fn(),
        scanning: false,
        latestResult: null,
        latestCodeResult: null,
      }),
      { wrapper: withQueryClient() },
    );

    expect(screen.getByText("Run your first scan")).toBeInTheDocument();

    dashboardState = {
      ...dashboardState,
      trend: [trendPoint],
      latestDetail: populatedDetail,
      latestScanId: 42,
      aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
    };

    // A hook-order violation on the setup -> populated boundary throws
    // here and fails the test on its own; no assertion wrapper needed.
    view.rerender(
      React.createElement(Dashboard, {
        url: "https://example.com",
        projectId: 7,
        projectName: "Example Site",
        framework: "Next.js",
        projectPath: "/tmp/example",
        onViewResults: vi.fn(),
        onViewCodeScan: vi.fn(),
        onRescan: vi.fn(),
        onOpenScanConfig: vi.fn(),
        onOpenCodeScanConfig: vi.fn(),
        onAddFolder: vi.fn(),
        onNavigate: vi.fn(),
        onOpenTarget: vi.fn(),
        scanning: false,
        latestResult: null,
        latestCodeResult: null,
      }),
    );

    // The rerender must actually cross into the populated branch: the
    // setup CTA is gone and the Zone 1 identity strip renders.
    expect(screen.queryByText("Run your first scan")).not.toBeInTheDocument();
    expect(screen.getByText("example.com")).toBeInTheDocument();
  });

  it("shows a retry state when the dashboard snapshot fails before any data loads", () => {
    const refreshDashboard = vi.fn();

    useTierMock.mockReturnValue({
      hasFeature: vi.fn((feature?: string) => feature === "code_scan" || feature === "analytics"),
      licenseInfo: { checkoutUrls: { coreMonthly: "" } },
    });
    usePendingVerificationCenterMock.mockReturnValue([]);
    useDesktopPromptCenterMock.mockReturnValue([]);
    useDashboardDataMock.mockReturnValue({
      trend: [],
      codeTrend: [],
      latestDetail: null,
      previousDetail: null,
      latestScanId: null,
      aggregatedCheckCounts: { passed: 0, total: 0, failed: 0 },
      aggregatedFailedIssues: [],
      securityUpdates: [],
      allUpdates: [],
      integrations: [],
      configuredIntegrations: new Set<string>(),
      lastCIRun: null,
      commitsSinceLastScan: [],
      issueLinks: [],
      psiReport: null,
      dashboardReady: true,
      dashboardLoadError: "Issues could not load right now.",
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      latestCodeScanSummary: null,
      previousCodeScanSummary: null,
      latestCodeScanDetail: null,
      updatesCheckedAt: null,
      searchRegression: null,
      integrationFailureCount: 0,
      staleIntegrationCount: 0,
      firstScanBannerDismissed: true,
      workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
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
      probesRefreshing: false,
      sslProbe: null,
      verdict: { kind: "healthy" as const, phrase: "Healthy", reasons: [] },
      criticalRollup: { total: 0, web: 0, code: 0, securityPatches: 0 },
      bootstrapTasks: [],
      criticalWebIssues: 0,
      criticalCodeIssues: 0,
      highWebIssues: 0,
      dismissFirstScanBanner: vi.fn(),
      refreshDashboard,
    });

    render(
      React.createElement(Dashboard, {
        url: "https://example.com",
        projectId: 7,
        projectName: "Example Site",
        framework: "Next.js",
        projectPath: "/tmp/example",
        onViewResults: vi.fn(),
        onViewCodeScan: vi.fn(),
        onRescan: vi.fn(),
        onOpenScanConfig: vi.fn(),
        onOpenCodeScanConfig: vi.fn(),
        onAddFolder: vi.fn(),
        onNavigate: vi.fn(),
        onOpenTarget: vi.fn(),
        scanning: false,
        latestResult: null,
        latestCodeResult: null,
      }),
      { wrapper: withQueryClient() },
    );

    expect(screen.getByText("Dashboard could not load")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(refreshDashboard).toHaveBeenCalledTimes(1);
  });
});

describe("getDesktopPromptAttentionMeta", () => {
  it("search-console path", () => {
    const meta = getDesktopPromptAttentionMeta({
      title: "Re-check SEO",
      detail: "",
      page: "search-console",
    });
    expect(meta.action).toBe("Open Search & SEO");
    expect(meta.description).toContain("SEO and indexing");
  });

  it("updates path", () => {
    const meta = getDesktopPromptAttentionMeta({
      title: "Re-check deps",
      detail: "",
      page: "updates",
    });
    expect(meta.action).toBe("Open Updates");
  });

  it("security path", () => {
    const meta = getDesktopPromptAttentionMeta({
      title: "Re-check security",
      detail: "",
      page: "issues",
    });
    expect(meta.action).toBe("Open Issues");
  });

  it("uses prompt.detail when non-empty, else default copy", () => {
    const withDetail = getDesktopPromptAttentionMeta({
      title: "t",
      detail: "edited src/auth.ts",
      page: "issues",
    });
    expect(withDetail.description).toBe("edited src/auth.ts");
    const empty = getDesktopPromptAttentionMeta({
      title: "t",
      detail: "",
      page: "issues",
    });
    expect(empty.description).toContain("Launch-sensitive");
  });

  it("uses the semantic target label when the prompt target has a specific reason", () => {
    const searchMeta = getDesktopPromptAttentionMeta({
      title: "robots.txt changed",
      detail: "Changed file: public/robots.txt.",
      page: "search-console",
      target: {
        page: "search-console",
        projectId: 1,
        url: "https://example.com",
        reason: "changed-search-file",
      },
    });
    expect(searchMeta.action).toBe("Verify Search & SEO");
  });
});
