import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ScoreSnapshot } from "@/lib/types";
import { withQueryClient } from "@/test-utils/query-client";

type CurrentScoreHookValue = { score: ScoreSnapshot | null; refresh: () => void };

const { useDashboardDataMock, hasFeatureMock, openUrlMock, useCurrentScoreMock } = vi.hoisted(
  () => ({
    useDashboardDataMock: vi.fn(),
    hasFeatureMock: vi.fn((_feature?: string) => false),
    openUrlMock: vi.fn(),
    useCurrentScoreMock: vi.fn((_projectId?: number, _url?: string): CurrentScoreHookValue => ({
      score: null,
      refresh: vi.fn(),
    })),
  }),
);

vi.mock("@/components/dashboard/useDashboardData", () => ({
  useDashboardData: (...args: unknown[]) => useDashboardDataMock(...args),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: hasFeatureMock,
    licenseInfo: {
      checkoutUrls: {
        coreMonthly: "https://checkout.sitecmd.test/core",
      },
    },
  }),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock("@/lib/open-url", () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
}));

vi.mock("@/hooks/useCurrentScore", () => ({
  useCurrentScore: (projectId: number, url: string) => useCurrentScoreMock(projectId, url),
}));

import { Dashboard } from "./Dashboard";

function baseDashboardData(overrides: Record<string, unknown> = {}) {
  return {
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
    dashboardLoadError: null,
    recentEvents: [],
    recentEventsLoading: false,
    latestCodeScanSummary: null,
    previousCodeScanSummary: null,
    latestCodeScanDetail: null,
    updatesCheckedAt: null,
    searchRegression: null,
    integrationFailureCount: 0,
    staleIntegrationCount: 0,
    firstScanBannerDismissed: true,
    dismissedIds: new Set<string>(),
    dismissedProjectId: 7,
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
    workQueue: { resumeNow: [], verifyNow: [], fixNext: [], maintenance: [] },
    probesRefreshing: false,
    referenceSignalsLoading: false,
    sslProbe: null,
    verdict: { kind: "healthy" as const, phrase: "Healthy", reasons: [] },
    criticalRollup: { total: 0, web: 0, code: 0, securityPatches: 0 },
    bootstrapTasks: [],
    criticalWebIssues: 0,
    criticalCodeIssues: 0,
    highWebIssues: 0,
    dismissFirstScanBanner: vi.fn(),
    refreshDashboard: vi.fn(),
    ...overrides,
  };
}

function renderDashboard(
  dataOverrides: Record<string, unknown> = {},
  propOverrides: Record<string, unknown> = {},
) {
  useDashboardDataMock.mockReturnValue(baseDashboardData(dataOverrides));

  return render(
    <Dashboard
      url="https://example.com"
      projectId={7}
      projectName="Example Site"
      framework="Next.js"
      projectPath={null}
      onViewResults={vi.fn()}
      onViewCodeScan={vi.fn()}
      onRescan={vi.fn()}
      onOpenScanConfig={vi.fn()}
      onOpenCodeScanConfig={vi.fn()}
      onAddFolder={vi.fn()}
      onNavigate={vi.fn()}
      onOpenTarget={vi.fn()}
      scanning={false}
      latestResult={null}
      latestCodeResult={null}
      {...propOverrides}
    />,
    { wrapper: withQueryClient() },
  );
}

describe("Dashboard behavior", () => {
  beforeEach(() => {
    useDashboardDataMock.mockReset();
    hasFeatureMock.mockReset();
    openUrlMock.mockReset();
    useCurrentScoreMock.mockReset();
    hasFeatureMock.mockReturnValue(false);
    useCurrentScoreMock.mockReturnValue({ score: null, refresh: vi.fn() });
  });

  it("shows a single Full Scan activity item and keeps newer update checks above it", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 78,
          performance: 74,
          seo: 82,
          accessibility: 85,
          compliance: 88,
          config: 79,
          polish: 80,
          timestamp: "2026-04-20T16:08:00Z",
          issues: 1,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [
          {
            checkId: "seo.meta",
            category: "seo",
            title: "Meta description missing",
            description: "Add a meta description.",
            status: "warn",
            severity: "medium",
            fixPrompt: null,
            manualFix: null,
            rawData: null,
          },
        ],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-20T16:08:00Z",
      },
      latestCodeScanSummary: {
        id: 42,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 77,
        issueCount: 4,
        criticalCount: 0,
        highCount: 1,
        durationMs: 900,
        checkedAt: "2026-04-20T16:09:00Z",
        framework: "Next.js",
        topDomain: null,
        topDomainCount: 0,
        domainSummaries: [],
      },
      updatesCheckedAt: "2026-04-20T16:35:00Z",
      allUpdates: [
        {
          name: "react",
          ecosystem: "npm",
          currentVersion: "18.2.0",
          latestVersion: "19.0.0",
          updateType: "major",
          isSecurity: false,
          source: "package.json",
        },
      ],
      securityUpdates: [],
    });

    const updateCheck = screen.getByText("Update Check");
    const fullScan = screen.getByText("Full Scan");

    expect(screen.queryByText("Web Scan")).not.toBeInTheDocument();
    expect(screen.queryByText("Code Scan")).not.toBeInTheDocument();
    expect(screen.getByText("0 Critical, 1 Major, 0 Minor, 0 Patch")).toBeInTheDocument();
    expect(screen.getByText(/1 web issue · 4 code issues/i)).toBeInTheDocument();
    expect(
      updateCheck.compareDocumentPosition(fullScan) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("opens Issues when the unified SiteCMD Score card is clicked", () => {
    const onNavigate = vi.fn();
    useCurrentScoreMock.mockReturnValue({
      score: {
        overall: 81,
        perCategory: {},
        criticalCount: 0,
        highCount: 1,
        mediumCount: 0,
        lowCount: 0,
        exploitableCapped: false,
        breakdown: {
          base: 100,
          criticalPoints: 0,
          highPoints: 0,
          mediumPoints: 0,
          lowPoints: 0,
          effCritical: 0,
          effHigh: 0,
          effMedium: 0,
          effLow: 0,
          floorApplied: false,
          ceilingApplied: false,
        },
        computedAt: 1,
      },
      refresh: vi.fn(),
    });
    renderDashboard(
      {
        trend: [
          {
            overall: 81,
            security: 78,
            performance: 74,
            seo: 82,
            accessibility: 85,
            compliance: 88,
            config: 79,
            polish: 80,
            timestamp: "2026-04-20T16:08:00Z",
            issues: 1,
            scanType: "health",
          },
        ],
        latestDetail: {
          url: "https://example.com",
          mode: "live",
          scanType: "health",
          overallScore: 81,
          categories: [],
          issues: [],
          detectedStack: null,
          durationMs: 1200,
          timestamp: "2026-04-20T16:08:00Z",
        },
        latestCodeScanSummary: {
          id: 42,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 77,
          issueCount: 4,
          criticalCount: 0,
          highCount: 1,
          durationMs: 900,
          checkedAt: "2026-04-20T16:09:00Z",
          framework: "Next.js",
          topDomain: null,
          topDomainCount: 0,
          domainSummaries: [],
        },
      },
      { onNavigate },
    );

    fireEvent.click(screen.getByText("SiteCMD Score").closest("button")!);
    expect(onNavigate).toHaveBeenCalledWith("issues");
  });

  it("renders the persisted current score instead of recomputing from page payloads", () => {
    const noisyAggregatedIssues = Array.from({ length: 20 }, (_, index) => ({
      checkId: `aggregated-high-${index}`,
      category: "security",
      title: `Aggregated high issue ${index}`,
      description: "Aggregated issue from the broader fix queue.",
      status: "fail",
      severity: "high",
      fixPrompt: null,
      manualFix: null,
      rawData: null,
    }));
    useCurrentScoreMock.mockReturnValue({
      score: {
        overall: 25,
        perCategory: { security: 50 },
        exploitableCapped: false,
        criticalCount: 1,
        highCount: 2,
        mediumCount: 3,
        lowCount: 4,
        breakdown: {
          base: 100,
          criticalPoints: 0,
          highPoints: 0,
          mediumPoints: 0,
          lowPoints: 0,
          effCritical: 0,
          effHigh: 0,
          effMedium: 0,
          effLow: 0,
          floorApplied: false,
          ceilingApplied: false,
        },
        computedAt: 1,
      },
      refresh: vi.fn(),
    });

    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 80,
          seo: 80,
          accessibility: 80,
          compliance: 80,
          config: 80,
          polish: 80,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 1,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [
          {
            checkId: "latest-medium",
            category: "seo",
            title: "Latest medium issue",
            description: "The latest scan has one medium issue.",
            status: "fail",
            severity: "medium",
            fixPrompt: null,
            manualFix: null,
            rawData: null,
          },
        ],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      aggregatedCheckCounts: { passed: 10, total: 30, failed: 20 },
      aggregatedFailedIssues: noisyAggregatedIssues,
    });

    const siteScoreTile = screen.getByText("SiteCMD Score").closest("button")!;
    expect(siteScoreTile).toHaveTextContent("25");
    expect(siteScoreTile).toHaveTextContent("Updated");
    expect(siteScoreTile).not.toHaveTextContent("issues");
  });

  it("shows five real activity events, collapses full scans, and links to the Activity page", () => {
    const onNavigate = vi.fn();

    renderDashboard(
      {
        trend: [
          {
            overall: 81,
            security: 79,
            performance: 75,
            seo: 83,
            accessibility: 86,
            compliance: 88,
            config: 77,
            polish: 80,
            timestamp: "2026-04-20T16:08:00Z",
            issues: 3,
            scanType: "health",
          },
        ],
        latestDetail: {
          url: "https://example.com",
          mode: "live",
          scanType: "health",
          overallScore: 81,
          categories: [],
          issues: [],
          detectedStack: null,
          durationMs: 1200,
          timestamp: "2026-04-20T16:08:00Z",
        },
        recentEvents: [
          {
            id: 1001,
            projectId: 7,
            eventType: "update",
            severity: "warning",
            occurredAtMs: Date.parse("2026-04-20T16:35:00Z"),
            title: "3 Updates Applied",
            summary: "react, vite, and lucide-react were updated.",
            detail: null,
            source: "internal",
            sourceId: "updates-1001",
          },
          {
            id: 1002,
            projectId: 7,
            eventType: "scan",
            severity: "warning",
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
            source: "internal",
            sourceId: "code_scan_42",
          },
          {
            id: 1003,
            projectId: 7,
            eventType: "scan",
            severity: "warning",
            occurredAtMs: Date.parse("2026-04-20T16:08:00Z"),
            title: "SiteCMD Score: 81/100",
            summary: "3 issues (1 critical, 1 high)",
            detail: JSON.stringify({
              scan_id: 41,
              scan_type: "health",
              overall_score: 81,
              issues_total: 3,
              url: "https://example.com",
            }),
            source: "internal",
            sourceId: "scan_41",
          },
          {
            id: 1004,
            projectId: 7,
            eventType: "deploy",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-20T15:50:00Z"),
            title: "Deploy passed",
            summary: "main deployed successfully.",
            detail: null,
            source: "git",
            sourceId: "deploy-1004",
          },
          {
            id: 1005,
            projectId: 7,
            eventType: "search",
            severity: "warning",
            occurredAtMs: Date.parse("2026-04-20T15:30:00Z"),
            title: "Search clicks dropped on /pricing",
            summary: "Clicks are down 18% week over week.",
            detail: null,
            source: "internal",
            sourceId: "search-1005",
          },
          {
            id: 1006,
            projectId: 7,
            eventType: "uptime",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-20T14:00:00Z"),
            title: "Uptime recovered",
            summary: "The site is back up after a brief outage.",
            detail: null,
            source: "uptimerobot",
            sourceId: "uptime-1006",
          },
        ],
      },
      { onNavigate },
    );

    expect(screen.getByText("3 Updates Applied")).toBeInTheDocument();
    expect(screen.getByText("Full Scan")).toBeInTheDocument();
    expect(screen.queryByText("Web Scan")).not.toBeInTheDocument();
    expect(screen.queryByText("Code Scan")).not.toBeInTheDocument();
    expect(screen.getByText("Deploy passed")).toBeInTheDocument();
    expect(screen.getByText("Search clicks dropped on /pricing")).toBeInTheDocument();
    expect(screen.getByText("Uptime recovered")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "View All Activity" }));
    expect(onNavigate).toHaveBeenCalledWith("events");
  });

  it("shows the real retry state when the dashboard cannot load before the first scan", () => {
    const refreshDashboard = vi.fn();

    renderDashboard({
      dashboardLoadError: new Error("offline"),
      refreshDashboard,
    });

    expect(screen.getByText("Dashboard could not load")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(refreshDashboard).toHaveBeenCalled();
  });

  it("pins the score card and Issues tile to one env-scoped, display-group, lifecycle-filtered count", () => {
    // Code detail can be present for presentation, but active counts come from
    // the backend's canonical IssueGroup projection in workSummary.
    hasFeatureMock.mockImplementation((feature?: string) => feature === "code_scan");

    // One active Web group plus one Code rule with two locations is two issues.
    useCurrentScoreMock.mockReturnValue({
      score: {
        overall: 81,
        perCategory: { security: 80 },
        exploitableCapped: false,
        criticalCount: 0,
        highCount: 1,
        mediumCount: 1,
        lowCount: 0,
        breakdown: {
          base: 100,
          criticalPoints: 0,
          highPoints: 0,
          mediumPoints: 0,
          lowPoints: 0,
          effCritical: 0,
          effHigh: 0,
          effMedium: 0,
          effLow: 0,
          floorApplied: false,
          ceilingApplied: false,
        },
        computedAt: 1,
      },
      refresh: vi.fn(),
    });

    const webIssue = (checkId: string, severity: string, title: string) => ({
      checkId,
      category: "security",
      title,
      description: "Web issue",
      status: "fail",
      severity,
      fixPrompt: null,
      manualFix: null,
      rawData: null,
    });
    const codeIssue = (id: string, checkId: string) => ({
      id,
      checkId,
      category: "architecture",
      domain: "architecture" as const,
      severity: "medium" as const,
      title: "Route mixes too many responsibilities",
      description: "Code issue",
      relativePath: `src/${id}.ts`,
      absolutePath: `/repo/src/${id}.ts`,
      line: null,
      sourceExcerpt: null,
      evidence: null,
      whyNow: null,
      likelyFix: null,
      verifyHint: null,
    });

    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 3,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [webIssue("web-a", "high", "Missing HSTS header")],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      aggregatedCheckCounts: { passed: 8, total: 10, failed: 2 },
      aggregatedFailedIssues: [
        webIssue("web-a", "high", "Missing HSTS header"),
        webIssue("web-b", "medium", "Missing canonical tag"),
      ],
      latestCodeScanDetail: {
        id: 1,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 80,
        issueCount: 3,
        criticalCount: 0,
        highCount: 0,
        mediumCount: 3,
        lowCount: 0,
        durationMs: 10,
        checkedAt: "2026-04-15T12:00:00Z",
        framework: "Astro",
        issues: [
          codeIssue("code-a1", "code_scan.god-route"),
          codeIssue("code-a2", "code_scan.god-route"),
          codeIssue("code-blocked", "code_scan.raw-sql"),
        ],
      },
      dismissedIds: new Set(["web-b", "code_scan.raw-sql"]),
      dismissedProjectId: 7,
      workSummary: {
        ...baseDashboardData().workSummary,
        issueCount: 2,
        issueWebCount: 1,
        issueCodeCount: 1,
        issueCriticalCount: 0,
        issueHighCount: 1,
        issueMediumCount: 1,
        issueLowCount: 0,
        unresolvedCount: 2,
        newCount: 2,
      },
    });

    // The score tile uses the snapshot without duplicating the issue count.
    const siteScoreTile = screen.getByText("SiteCMD Score").closest("button")!;
    expect(siteScoreTile).toHaveTextContent("81");
    expect(siteScoreTile).not.toHaveTextContent("issues");

    // The Issues action item reads the same canonical projection as the list.
    const issuesTile = screen.getByText("Issues").closest("button")!;
    expect(issuesTile).toHaveTextContent("2 Open");
  });

  it("shows the real first-scan actions instead of only mocked dashboard cards", () => {
    const onOpenScanConfig = vi.fn();
    const onAddFolder = vi.fn();

    renderDashboard(
      {},
      {
        onOpenScanConfig,
        onAddFolder,
      },
    );

    fireEvent.click(screen.getByRole("button", { name: /Run your first scan/i }));
    expect(onOpenScanConfig).toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /Link your project folder/i }));
    expect(onAddFolder).toHaveBeenCalled();
  });

  it("surfaces operational signal cards on the dashboard even without the analytics feature", () => {
    const onNavigate = vi.fn();

    renderDashboard(
      {
        trend: [
          {
            overall: 81,
            security: 80,
            performance: 74,
            seo: 79,
            accessibility: 85,
            compliance: 88,
            config: 76,
            polish: 77,
            timestamp: "2026-04-15T12:00:00Z",
            issues: 0,
            scanType: "health",
          },
        ],
        latestDetail: {
          url: "https://example.com",
          mode: "live",
          scanType: "health",
          overallScore: 81,
          categories: [],
          issues: [],
          detectedStack: null,
          durationMs: 1200,
          timestamp: "2026-04-15T12:00:00Z",
        },
        latestScanId: 42,
        aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
        aggregatedFailedIssues: [],
        integrations: [
          {
            integrationType: "plausible",
            error: null,
            data: {
              visitors: 4321,
              pageviews: 9876,
              bounce_rate: 42,
              visit_duration: 123,
            },
          },
        ],
        configuredIntegrations: new Set(["plausible"]),
      },
      {
        onNavigate,
      },
    );

    // Zone 2 Visitors tile is present even without the analytics feature.
    expect(screen.getByText("Visitors 30d")).toBeInTheDocument();

    // Visitors tile shows visitor count from plausible data (formatNum: 4.3k).
    expect(screen.getByText(/4\.3k|4,321|4321/i)).toBeInTheDocument();

    // Clicking the Visitors tile routes to the Analytics page; that page handles gating itself.
    const visitorsTile = screen.getByText("Visitors 30d").closest("button")!;
    fireEvent.click(visitorsTile);
    expect(onNavigate).toHaveBeenCalledWith("analytics");
  });

  it("shows connected Search Console data on the dashboard search cards", () => {
    const onNavigate = vi.fn();

    renderDashboard(
      {
        trend: [
          {
            overall: 81,
            security: 80,
            performance: 74,
            seo: 79,
            accessibility: 85,
            compliance: 88,
            config: 76,
            polish: 77,
            timestamp: "2026-04-15T12:00:00Z",
            issues: 0,
            scanType: "health",
          },
        ],
        latestDetail: {
          url: "https://example.com",
          mode: "live",
          scanType: "health",
          overallScore: 81,
          categories: [],
          issues: [],
          detectedStack: null,
          durationMs: 1200,
          timestamp: "2026-04-15T12:00:00Z",
        },
        latestScanId: 42,
        aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
        aggregatedFailedIssues: [],
        integrations: [
          {
            integrationType: "googlesearchconsole",
            error: null,
            data: {
              total_clicks: 123,
              total_impressions: 4567,
              average_ctr: 0.04,
              average_position: 8.4,
              top_queries: [],
              top_pages: [
                {
                  page: "https://example.com/",
                  clicks: 80,
                  impressions: 3000,
                  ctr: 0.03,
                  position: 7,
                },
                {
                  page: "https://example.com/pricing",
                  clicks: 43,
                  impressions: 1567,
                  ctr: 0.03,
                  position: 11,
                },
              ],
              daily: [],
              devices: [],
            },
          },
        ],
        configuredIntegrations: new Set(["googlesearchconsole"]),
        searchRegression: { source: "gsc", deltaPct: -12.4 },
      },
      { onNavigate },
    );

    const seoTile = screen.getByText("SEO clicks 28d").closest("button")!;
    expect(seoTile).toHaveTextContent("123");
    expect(seoTile).toHaveTextContent("4.6k impressions");

    const searchTile = screen.getByText("Search & Index").closest("button")!;
    expect(searchTile).toHaveTextContent("2 visible pages");
    expect(searchTile).toHaveTextContent("Search Console");

    fireEvent.click(seoTile);
    expect(onNavigate).toHaveBeenCalledWith("search-console");
  });

  it("shows Cloudflare cache hit rate as the provider percentage", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 0,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      latestScanId: 42,
      aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
      aggregatedFailedIssues: [],
      integrations: [
        {
          integrationType: "cloudflare",
          error: null,
          data: {
            cache_hit_rate: 87,
            requests_total: 1000,
            threats_blocked: 3,
            bandwidth_total: 1024 * 1024 * 12,
          },
        },
      ],
      configuredIntegrations: new Set(["cloudflare"]),
    });

    expect(screen.getByText("87% cache hit")).toBeInTheDocument();
    expect(screen.queryByText("8700% cache hit")).not.toBeInTheDocument();
  });

  it("places reference signal cards between action items and activity rows", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 0,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      latestScanId: 42,
      aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
    });

    const issuesCard = screen.getByText("Issues");
    const webVitals = screen.getByText("Web Vitals");
    const recentActivity = screen.getByText("Recent Activity");

    expect(
      issuesCard.compareDocumentPosition(webVitals) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      webVitals.compareDocumentPosition(recentActivity) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("folds last-checked into the score tile and drops the duplicate issue stat", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 0,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      latestScanId: 42,
      aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
    });

    expect(screen.queryByText("Last Checked")).not.toBeInTheDocument();
    expect(screen.queryByText("Critical Issues")).not.toBeInTheDocument();
  });

  it("does not show a failed integration action item when live integration cards are healthy", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 0,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      latestScanId: 42,
      aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
      integrationFailureCount: 1,
      integrations: [
        {
          integrationType: "plausible",
          error: null,
          data: { visitors: 100, pageviews: 250, bounce_rate: 40, visit_duration: 80 },
        },
      ],
      configuredIntegrations: new Set(["plausible"]),
    });

    expect(screen.queryByText(/failed sync/i)).not.toBeInTheDocument();
  });

  it("does not show a fake zero visitors count while analytics is still loading", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 0,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      latestScanId: 42,
      aggregatedCheckCounts: { passed: 10, total: 10, failed: 0 },
      aggregatedFailedIssues: [],
      integrations: [],
      configuredIntegrations: new Set(["plausible"]),
      referenceSignalsLoading: true,
    });

    expect(screen.getByText("Loading analytics...")).toBeInTheDocument();
  });

  it("ignores scan-measured vitals for Web Vitals when PSI data is missing (PageSpeed-only)", () => {
    renderDashboard(
      {
        trend: [
          {
            overall: 81,
            security: 80,
            performance: 74,
            seo: 79,
            accessibility: 85,
            compliance: 88,
            config: 76,
            polish: 77,
            timestamp: "2026-04-15T12:00:00Z",
            issues: 2,
            scanType: "health",
          },
        ],
        latestDetail: null,
        psiReport: null,
      },
      {
        latestResult: {
          url: "https://example.com",
          mode: "live",
          scanType: "health",
          overallScore: 81,
          categories: [
            {
              category: "performance",
              score: 74,
              checks_passed: 6,
              checks_total: 8,
            },
          ],
          issues: [
            {
              checkId: "performance.lcp",
              category: "performance",
              title: "Largest Contentful Paint",
              description: "LCP is slower than ideal.",
              status: "fail",
              severity: "medium",
              fixPrompt: null,
              manualFix: null,
              rawData: {
                lcpMs: 2100,
                rating: "needs-improvement",
              },
            },
            {
              checkId: "performance.cls",
              category: "performance",
              title: "Cumulative Layout Shift",
              description: "CLS is elevated.",
              status: "warn",
              severity: "low",
              fixPrompt: null,
              manualFix: null,
              rawData: {
                cls: 0.08,
                rating: "good",
              },
            },
          ],
          detectedStack: null,
          durationMs: 1200,
          timestamp: "2026-04-15T12:00:00Z",
        },
      },
    );

    // PageSpeed-only: even though the scan measured LCP/CLS, the card shows the
    // PageSpeed prompt rather than scan-derived vitals.
    expect(screen.getByText("Run PageSpeed")).toBeInTheDocument();
    expect(screen.queryByText(/LCP 2\.1s/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Score 74\/100/i)).not.toBeInTheDocument();
  });

  it("shows the PageSpeed prompt for Web Vitals when PSI is unavailable even with a scan score", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 0,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [
          {
            category: "performance",
            score: 74,
            checks_passed: 6,
            checks_total: 8,
          },
        ],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      psiReport: null,
    });

    // PageSpeed-only: the scan's performance category score is not shown here.
    expect(screen.getByText("Run PageSpeed")).toBeInTheDocument();
    expect(screen.queryByText("Performance 74/100")).not.toBeInTheDocument();
  });

  it("does not render the retired standalone Code Scan Snapshot section", () => {
    renderDashboard(
      {
        trend: [
          {
            overall: 81,
            security: 80,
            performance: 74,
            seo: 79,
            accessibility: 85,
            compliance: 88,
            config: 76,
            polish: 77,
            timestamp: "2026-04-15T12:00:00Z",
            issues: 3,
            scanType: "health",
          },
        ],
        latestDetail: {
          url: "https://example.com",
          mode: "live",
          scanType: "health",
          overallScore: 81,
          categories: [],
          issues: [],
          detectedStack: null,
          durationMs: 1200,
          timestamp: "2026-04-15T12:00:00Z",
        },
        latestScanId: 42,
        aggregatedCheckCounts: { passed: 7, total: 10, failed: 3 },
        aggregatedFailedIssues: [],
        latestCodeScanSummary: {
          id: 91,
          overallScore: 73,
          issueCount: 4,
          criticalCount: 1,
          highCount: 2,
          mediumCount: 1,
          lowCount: 0,
          topDomain: "security",
          topDomainCount: 2,
          checkedAt: "2026-04-15T12:00:00Z",
          framework: "Next.js",
        },
        latestCodeScanDetail: {
          projectId: 7,
          environmentUrl: "https://example.com",
          checkedAt: "2026-04-15T12:00:00Z",
          framework: "Next.js",
          issueCount: 4,
          criticalCount: 1,
          highCount: 2,
          mediumCount: 1,
          lowCount: 0,
          issues: [],
        },
        codeTrend: [{ score: 73, timestamp: "2026-04-15T12:00:00Z" }],
      },
      {
        projectPath: "/tmp/example-site",
      },
    );

    expect(screen.queryByText("Code Scan Snapshot")).not.toBeInTheDocument();
    expect(screen.queryByText("View latest Code Scan")).not.toBeInTheDocument();
  });

  it("publishes issues badge counts from code summary when detail has not loaded yet", () => {
    renderDashboard({
      aggregatedCheckCounts: { passed: 8, total: 9, failed: 1 },
      aggregatedFailedIssues: [
        {
          checkId: "security.hsts",
          category: "security",
          title: "Missing HSTS header",
          description: "Strict-Transport-Security header is not set.",
          status: "fail",
          severity: "critical",
          fixPrompt: null,
          manualFix: null,
          rawData: null,
        },
      ],
      latestCodeScanSummary: {
        id: 91,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 73,
        issueCount: 4,
        criticalCount: 2,
        highCount: 1,
        durationMs: 900,
        checkedAt: "2026-04-15T12:00:00Z",
        framework: "Next.js",
        topDomain: "security",
        topDomainCount: 2,
        domainSummaries: [],
      },
      latestCodeScanDetail: null,
    });

    expect(screen.getByText("example.com")).toBeInTheDocument();
  });

  it("shows the shared total on the Issues stat card instead of only web issues", () => {
    renderDashboard({
      trend: [
        {
          overall: 81,
          security: 80,
          performance: 74,
          seo: 79,
          accessibility: 85,
          compliance: 88,
          config: 76,
          polish: 77,
          timestamp: "2026-04-15T12:00:00Z",
          issues: 3,
          scanType: "health",
        },
      ],
      latestDetail: {
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 81,
        categories: [],
        issues: [],
        detectedStack: null,
        durationMs: 1200,
        timestamp: "2026-04-15T12:00:00Z",
      },
      latestScanId: 42,
      aggregatedCheckCounts: { passed: 7, total: 10, failed: 1 },
      aggregatedFailedIssues: [
        {
          checkId: "security.hsts",
          category: "security",
          title: "Missing HSTS header",
          description: "Strict-Transport-Security header is not set.",
          status: "fail",
          severity: "critical",
          fixPrompt: null,
          manualFix: null,
          rawData: null,
        },
      ],
      latestCodeScanSummary: {
        id: 91,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 73,
        issueCount: 4,
        criticalCount: 2,
        highCount: 1,
        durationMs: 900,
        checkedAt: "2026-04-15T12:00:00Z",
        framework: "Next.js",
        topDomain: "security",
        topDomainCount: 2,
        domainSummaries: [],
      },
      latestCodeScanDetail: null,
      workSummary: {
        unresolvedCount: 7,
        newCount: 0,
        workingCount: 0,
        regressedCount: 0,
        ignoredCount: 0,
        blockedCount: 1,
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
    });

    // Zone 1 identity strip confirms Dashboard rendered in populated state.
    expect(screen.getByText("example.com")).toBeInTheDocument();
  });

  it("does not render the retired attention panel", () => {
    renderDashboard(
      {
        trend: [
          {
            overall: 81,
            security: 80,
            performance: 74,
            seo: 79,
            accessibility: 85,
            compliance: 88,
            config: 76,
            polish: 77,
            timestamp: "2026-04-15T12:00:00Z",
            issues: 1,
            scanType: "health",
          },
        ],
        latestDetail: {
          url: "https://example.com",
          mode: "live",
          scanType: "health",
          overallScore: 81,
          categories: [],
          issues: [],
          detectedStack: null,
          durationMs: 1200,
          timestamp: "2026-04-15T12:00:00Z",
        },
        latestScanId: 42,
        aggregatedCheckCounts: { passed: 9, total: 10, failed: 1 },
        aggregatedFailedIssues: [],
        workSummary: {
          unresolvedCount: 1,
          newCount: 1,
          workingCount: 0,
          regressedCount: 0,
          ignoredCount: 0,
          blockedCount: 0,
          launchBlockerCount: 1,
          maintenanceCount: 0,
          primaryAction: {
            stableKey: "launch:hero",
            projectId: 7,
            environmentUrl: "https://example.com",
            kind: "launch",
            status: "new",
            severity: "high",
            title: "Launch blockers remain",
            summary: "Clear the launch blockers before you ship.",
            category: "launch",
            domain: null,
            packageName: null,
            target: {
              page: "issues",
              projectId: 7,
              url: "https://example.com",
              itemId: "launch-hero",
            },
            firstSeenAt: "2026-04-15T12:00:00Z",
            lastSeenAt: "2026-04-15T12:00:00Z",
            lastVerifiedAt: null,
            lastStatusChangedAt: "2026-04-15T12:00:00Z",
          },
          regressedAction: null,
          workingAction: null,
          blockedAction: null,
          ignoredAction: null,
          launchBlockerAction: {
            stableKey: "launch:hero",
            projectId: 7,
            environmentUrl: "https://example.com",
            kind: "launch",
            status: "new",
            severity: "high",
            title: "Launch blockers remain",
            summary: "Clear the launch blockers before you ship.",
            category: "launch",
            domain: null,
            packageName: null,
            target: {
              page: "issues",
              projectId: 7,
              url: "https://example.com",
              itemId: "launch-hero",
            },
            firstSeenAt: "2026-04-15T12:00:00Z",
            lastSeenAt: "2026-04-15T12:00:00Z",
            lastVerifiedAt: null,
            lastStatusChangedAt: "2026-04-15T12:00:00Z",
          },
          weeklySummary: null,
        },
      },
      {
        projectPath: "/tmp/example-site",
      },
    );

    expect(screen.queryByText("What Needs Attention Now")).not.toBeInTheDocument();
  });

  it("keeps existing nav badges stable while dashboard data is still loading", () => {
    renderDashboard({
      dashboardReady: false,
      dashboardLoadError: null,
      allUpdates: [],
      securityUpdates: [],
      aggregatedFailedIssues: [],
      latestCodeScanSummary: null,
      latestCodeScanDetail: null,
    });

    expect(screen.getByLabelText("Dashboard loading state")).toBeInTheDocument();
  });

  it("shows the populated dashboard for a code-only project with no web scan", () => {
    renderDashboard(
      {
        trend: [],
        latestDetail: null,
        latestScanId: null,
        latestCodeScanSummary: {
          id: 42,
          projectId: 7,
          environmentUrl: "",
          overallScore: 77,
          issueCount: 4,
          criticalCount: 0,
          highCount: 1,
          durationMs: 900,
          checkedAt: "2026-04-20T16:09:00Z",
          framework: "Next.js",
          topDomain: null,
          topDomainCount: 0,
          domainSummaries: [],
        },
      },
      { url: "", projectPath: "/Users/dev/app" },
    );

    expect(screen.queryByRole("button", { name: /Run your first scan/i })).not.toBeInTheDocument();
    expect(screen.getByText("SiteCMD Score")).toBeInTheDocument();
    expect(screen.getByText("Issues")).toBeInTheDocument();
  });
});
