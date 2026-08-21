import React from "react";
import { fireEvent, render as rtlRender, screen, waitFor } from "@testing-library/react";
import { withQueryClient } from "@/test-utils/query-client";

// IssuesPage uses useInactiveIssueKeys (a useQuery), so it needs a
// QueryClientProvider. A fresh client per render keeps tests isolated.
const render = (ui: Parameters<typeof rtlRender>[0], options?: Parameters<typeof rtlRender>[1]) =>
  rtlRender(ui, { wrapper: withQueryClient(), ...options });
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CheckResult, IssueGroup } from "@/lib/types";
import { getIssuesWebCategoryFocus } from "@/lib/app-targets";

const {
  invokeMock,
  useDashboardDataMock,
  usePendingVerificationCenterMock,
  useDesktopPromptCenterMock,
  useCurrentScoreMock,
  rankUnifiedSpy,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  useDashboardDataMock: vi.fn(),
  usePendingVerificationCenterMock: vi.fn(),
  useDesktopPromptCenterMock: vi.fn(),
  useCurrentScoreMock: vi.fn(),
  rankUnifiedSpy: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// Pass-through spy so tests can count ranking passes without changing what
// gets ranked.
vi.mock("@/lib/issue-ranking", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/issue-ranking")>();
  rankUnifiedSpy.mockImplementation(actual.rankIssueGroups);
  return { ...actual, rankIssueGroups: rankUnifiedSpy };
});

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/components/dashboard/useDashboardData", () => ({
  useDashboardData: (...args: unknown[]) => useDashboardDataMock(...args),
}));

vi.mock("@/lib/pending-verification", () => ({
  usePendingVerificationCenter: () => usePendingVerificationCenterMock(),
}));

vi.mock("@/lib/desktop-prompts", () => ({
  buildDesktopPromptTarget: vi.fn(),
  useDesktopPromptCenter: () => useDesktopPromptCenterMock(),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: () => false,
  }),
}));

vi.mock("@/hooks/useCurrentScore", () => ({
  useCurrentScore: (projectId: number, url: string) => useCurrentScoreMock(projectId, url),
}));

vi.mock("@/components/issues/IssueDossier", () => ({
  IssueDossier: ({
    selected,
    onClose,
    onDismiss,
  }: {
    selected: { issue: { title?: string; checkId?: string } };
    onClose: () => void;
    onDismiss?: (checkId: string) => void;
  }) =>
    React.createElement("aside", null, [
      React.createElement("div", { key: "title" }, `Selected issue: ${selected.issue.title}`),
      React.createElement(
        "button",
        { key: "close", type: "button", onClick: onClose },
        "Close details",
      ),
      onDismiss && selected.issue.checkId
        ? React.createElement(
            "button",
            {
              key: "dismiss",
              type: "button",
              onClick: () => onDismiss(selected.issue.checkId ?? ""),
            },
            "Dismiss issue",
          )
        : null,
    ]),
}));

vi.mock("@/components/scan/ScanHistory", () => ({
  ScanHistory: () => null,
}));

import { IssuesPage } from "./IssuesPage";
import { withAppContext } from "@/test-utils/app-context";

function buildWebIssue(overrides?: Partial<CheckResult>): CheckResult {
  return {
    checkId: "seo.missing-canonical",
    category: "seo",
    title: "Missing canonical tag",
    description: "Important pages should point to their preferred URL.",
    status: "fail",
    severity: "high",
    fixPrompt: "Add a canonical tag in the page head and rerun the scan.",
    manualFix: "Update the page template with the correct canonical URL.",
    rawData: { url: "https://example.com/docs" },
    confidence: "high",
    whyItMatters: "Search engines can split ranking signals across duplicate URLs.",
    ...overrides,
  };
}

function buildCodeIssue(overrides?: Record<string, unknown>) {
  return {
    id: "code-security-env-file",
    checkId: "code_scan.code-security-env-file",
    category: "security",
    domain: "security",
    severity: "high",
    title: "Environment file is committed",
    description: "Sensitive credentials are stored in a tracked env file.",
    relativePath: "src/config/env.ts",
    absolutePath: "/tmp/example/src/config/env.ts",
    line: 12,
    sourceExcerpt: "export const OPENAI_KEY = process.env.OPENAI_KEY;",
    evidence: "The repository contains committed environment secrets.",
    whyNow: "Committed secrets are easy to leak and hard to rotate cleanly.",
    likelyFix: "Move secrets out of the repo and rotate them.",
    verifyHint: "Confirm the file is removed from git and secrets are rotated.",
    ...overrides,
  };
}

let activeIssueGroups: IssueGroup[] = [];

function issueGroupBase(
  checkId: string,
  category: string,
  severity: IssueGroup["severity"],
  title: string,
  description: string,
): Omit<IssueGroup, "instances" | "sources"> {
  return {
    checkId,
    category,
    severity,
    title,
    description,
    status: "new",
    snoozeUntil: null,
    blockReason: null,
    impactScore: severity === "critical" ? 15 : severity === "high" ? 8 : 3,
    likelyCauses: [],
    suggestedIntegrations: [],
    fixLocations: [],
    transitiveCauses: [],
    downstreamEffects: [],
    recentEvents: [],
    enrichments: [],
    correlationEvidence: [],
    affectedPages: [],
    crossEnvSignal: null,
    crossProjectPattern: null,
    displayConfidence: null,
    observationCount: 1,
    anomalyScore: null,
  };
}

function webIssueGroup(issue: CheckResult): IssueGroup {
  return {
    ...issueGroupBase(
      issue.checkId,
      issue.category,
      issue.severity,
      issue.title,
      issue.description,
    ),
    sources: ["web_scan"],
    instances: [
      {
        id: 1,
        source: "web_scan",
        signalId: `web:${issue.checkId}`,
        producerCheckId: issue.checkId,
        url: "https://example.com",
        pageUrl: "https://example.com/docs",
        severity: issue.severity,
        title: issue.title,
        description: issue.description,
        category: issue.category,
        checkStatus: issue.status,
        fixPrompt: issue.fixPrompt ?? undefined,
        manualFix: issue.manualFix ?? undefined,
        whyItMatters: issue.whyItMatters,
        detailJson: JSON.stringify(issue.rawData),
        firstSeenAt: 1,
        lastSeenAt: 2,
        confidence: issue.confidence,
        confidenceReason: issue.confidenceReason,
        domain: null,
        relativePath: null,
        line: null,
      },
    ],
  };
}

function codeIssueGroup(issue: ReturnType<typeof buildCodeIssue>): IssueGroup {
  const checkId = String(issue.checkId);
  const severity = issue.severity as IssueGroup["severity"];
  return {
    ...issueGroupBase(
      checkId,
      String(issue.category),
      severity,
      String(issue.title),
      String(issue.description),
    ),
    sources: ["code_scan"],
    instances: [
      {
        id: 1,
        source: "code_scan",
        signalId: `code:${String(issue.id)}:${String(issue.relativePath)}`,
        producerCheckId: String(issue.id),
        url: "https://example.com",
        pageUrl: null,
        severity,
        title: String(issue.title),
        description: String(issue.description),
        category: String(issue.category),
        checkStatus: "fail",
        whyItMatters: String(issue.whyNow),
        detailJson: JSON.stringify(issue),
        firstSeenAt: 1,
        lastSeenAt: 2,
        confidence: "high",
        domain: issue.domain as IssueGroup["instances"][number]["domain"],
        relativePath: String(issue.relativePath),
        line: Number(issue.line),
        producerFixPrompt: String(issue.likelyFix),
      },
    ],
  };
}

function buildDashboardState(overrides?: Record<string, unknown>) {
  return {
    aggregatedFailedIssues: [],
    securityUpdates: [],
    allUpdates: [],
    lastCIRun: null,
    latestDetail: null,
    latestCodeScanSummary: null,
    latestCodeScanDetail: null,
    issueLinks: [],
    dashboardReady: true,
    dashboardLoadError: null,
    dismissedIds: new Set<string>(),
    dismissedProjectId: 7,
    workQueue: {
      resumeNow: [],
      verifyNow: [],
      fixNext: [],
      maintenance: [],
    },
    refreshDashboard: vi.fn(),
    ...overrides,
  };
}

function routeCodeDetailInvokes(responses: Array<() => Promise<unknown>>) {
  const queue = [...responses];
  invokeMock.mockImplementation((command: string) => {
    if (command === "get_work_items") {
      return Promise.resolve(activeIssueGroups);
    }
    const next = queue.shift();
    return next ? next() : Promise.resolve(undefined);
  });
}

describe("IssuesPage real behavior", () => {
  beforeEach(() => {
    window.localStorage.clear();
    invokeMock.mockReset();
    activeIssueGroups = [];
    invokeMock.mockImplementation((command: string) =>
      Promise.resolve(command === "get_work_items" ? activeIssueGroups : null),
    );
    useDashboardDataMock.mockReset();
    usePendingVerificationCenterMock.mockReset();
    useDesktopPromptCenterMock.mockReset();
    useCurrentScoreMock.mockReset();

    usePendingVerificationCenterMock.mockReturnValue([]);
    useDesktopPromptCenterMock.mockReturnValue([]);
    useCurrentScoreMock.mockReturnValue({ score: null, refresh: vi.fn() });
    // Clear call history but keep the pass-through implementation installed by
    // the module mock above.
    rankUnifiedSpy.mockClear();
  });

  it("renders the real issue list rows and opens the dossier when a user picks an issue", async () => {
    const issue = buildWebIssue();
    activeIssueGroups = [webIssueGroup(issue)];

    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [issue],
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: null,
      latestCodeScanDetail: null,
      issueLinks: [],
      dashboardReady: true,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [],
        verifyNow: [],
        fixNext: [],
        maintenance: [],
      },
      refreshDashboard: vi.fn(),
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={{
            url: "https://example.com",
            mode: "live",
            scanType: "health",
            overallScore: 71,
            categories: [],
            issues: [],
            detectedStack: null,
            durationMs: 900,
            timestamp: "2026-04-15T12:00:00Z",
          }}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /missing canonical tag/i })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /missing canonical tag/i }));

    expect(await screen.findByText("Selected issue: Missing canonical tag")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Close details" }));

    await waitFor(() => {
      expect(screen.queryByText("Selected issue: Missing canonical tag")).not.toBeInTheDocument();
    });
  });

  it("ranks the unified issue list once per data change (page ranks, list consumes the prop)", async () => {
    const issue = buildWebIssue();
    activeIssueGroups = [webIssueGroup(issue)];

    useDashboardDataMock.mockReturnValue(
      buildDashboardState({
        aggregatedFailedIssues: [issue],
      }),
    );
    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /missing canonical tag/i })).toBeInTheDocument();
    });

    const populatedRankingCalls = rankUnifiedSpy.mock.calls.filter(
      ([groups]) => Array.isArray(groups) && groups.length > 0,
    );
    expect(populatedRankingCalls).toHaveLength(1);
  });

  it("renders the persisted current score while issue details follow the page summary", async () => {
    const noisyAggregatedIssues = Array.from({ length: 20 }, (_, index) =>
      buildWebIssue({
        checkId: `aggregated-high-${index}`,
        category: "security",
        title: `Aggregated high issue ${index}`,
        description: "Aggregated issue from the broader fix queue.",
        severity: "high",
      }),
    );
    activeIssueGroups = noisyAggregatedIssues.map(webIssueGroup);

    useDashboardDataMock.mockReturnValue(
      buildDashboardState({
        aggregatedFailedIssues: noisyAggregatedIssues,
      }),
    );
    useCurrentScoreMock.mockReturnValue({
      score: {
        overall: 25,
        perCategory: { security: 50 },
        criticalCount: 1,
        highCount: 2,
        mediumCount: 3,
        lowCount: 4,
        computedAt: 1,
      },
      refresh: vi.fn(),
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={{
            url: "https://example.com",
            mode: "live",
            scanType: "health",
            overallScore: 81,
            categories: [],
            issues: [
              buildWebIssue({
                checkId: "latest-medium",
                category: "seo",
                title: "Latest medium issue",
                description: "The latest scan has one medium issue.",
                severity: "medium",
              }),
            ],
            detectedStack: null,
            durationMs: 900,
            timestamp: "2026-04-15T12:00:00Z",
          }}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      const scoreCard = screen.getByText("SiteCMD Score").closest(".panel")!;
      expect(scoreCard).toHaveTextContent("25");
      expect(scoreCard).toHaveTextContent("20 issues");
    });

    const scoreCard = screen.getByText("SiteCMD Score").closest(".panel")!;
    expect(scoreCard).not.toHaveTextContent("10 issues");
    expect(scoreCard).not.toHaveTextContent("critical/high");
  });

  it("applies an initial focus filter when Issues opens from a targeted deep link", async () => {
    const securityIssue = buildWebIssue({
      checkId: "security.hsts",
      category: "security",
      title: "Missing HSTS header",
    });
    const seoIssue = buildWebIssue();
    activeIssueGroups = [webIssueGroup(securityIssue), webIssueGroup(seoIssue)];

    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [securityIssue, seoIssue],
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: null,
      latestCodeScanDetail: null,
      issueLinks: [],
      dashboardReady: true,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [],
        verifyNow: [],
        fixNext: [],
        maintenance: [],
      },
      refreshDashboard: vi.fn(),
      workSummary: {
        unresolvedCount: 2,
        blockedCount: 0,
        launchBlockerCount: 0,
      },
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
        { navigation: { issuesTarget: { focus: getIssuesWebCategoryFocus("security") } } },
      ),
    );

    await waitFor(() => {
      expect(screen.getByText("Missing HSTS header")).toBeInTheDocument();
    });

    expect(screen.queryByText("Missing canonical tag")).not.toBeInTheDocument();
  });

  it("does not inject generic code work-queue reminders into the issues list", async () => {
    const codeIssue = buildCodeIssue();
    activeIssueGroups = [codeIssueGroup(codeIssue)];

    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [],
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: {
        id: 401,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 64,
        issueCount: 1,
        criticalCount: 0,
        highCount: 1,
        durationMs: 920,
        checkedAt: "2026-04-15T12:00:00Z",
        framework: "react",
        topDomain: "security",
        topDomainCount: 1,
        domainSummaries: [],
      },
      latestCodeScanDetail: {
        id: 401,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 64,
        issueCount: 1,
        criticalCount: 0,
        highCount: 1,
        mediumCount: 0,
        lowCount: 0,
        durationMs: 920,
        checkedAt: "2026-04-15T12:00:00Z",
        framework: "react",
        domainSummaries: [],
        issues: [codeIssue],
      },
      issueLinks: [],
      dashboardReady: true,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [
          {
            stableKey: "code:security",
            projectId: 7,
            environmentUrl: "https://example.com",
            kind: "code",
            status: "working",
            severity: "high",
            title: "Security code issue still open",
            summary: "Open Code Scan to review the highest-priority issue in the security lane.",
            category: "security",
            domain: "security",
            packageName: null,
            target: {
              page: "issues",
              panel: "code",
              codeIssueId: null,
              itemId: null,
            },
            firstSeenAt: "2026-04-15T11:00:00Z",
            lastSeenAt: "2026-04-15T12:00:00Z",
            lastVerifiedAt: null,
            lastStatusChangedAt: "2026-04-15T12:00:00Z",
          },
        ],
        verifyNow: [],
        fixNext: [],
        maintenance: [],
      },
      refreshDashboard: vi.fn(),
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={{
            url: "https://example.com",
            mode: "live",
            scanType: "health",
            overallScore: 71,
            categories: [],
            issues: [],
            detectedStack: null,
            durationMs: 900,
            timestamp: "2026-04-15T12:00:00Z",
          }}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /environment file is committed/i }),
      ).toBeInTheDocument();
    });

    expect(screen.queryByText("Security code issue still open")).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        "Open Code Scan to review the highest-priority issue in the security lane.",
      ),
    ).not.toBeInTheDocument();
  });

  it("does not synthesize active issues from a Code history summary", async () => {
    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [],
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: {
        id: 402,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 64,
        issueCount: 4,
        criticalCount: 1,
        highCount: 2,
        durationMs: 920,
        checkedAt: "2026-04-15T12:00:00Z",
        framework: "react",
        topDomain: "security",
        topDomainCount: 2,
        domainSummaries: [],
      },
      latestCodeScanDetail: null,
      issueLinks: [],
      dashboardReady: true,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [],
        verifyNow: [],
        fixNext: [],
        maintenance: [],
      },
      workSummary: {
        unresolvedCount: 4,
        blockedCount: 0,
        launchBlockerCount: 0,
      },
      refreshDashboard: vi.fn(),
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(screen.getByText("No web or code issues open")).toBeInTheDocument();
    });
    expect(screen.queryByText(/Code Scan issues from the latest scan/i)).not.toBeInTheDocument();
  });

  it("shows critical security updates as page banners and skips routine update reminders", async () => {
    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [],
      securityUpdates: [
        {
          name: "next",
          currentVersion: "14.2.0",
          latestVersion: "14.2.9",
          ecosystem: "npm",
          updateType: "patch",
          isSecurity: true,
          advisorySeverity: "critical",
          advisoryUrl: null,
          source: "package.json",
          isDev: false,
        },
        {
          name: "vite",
          currentVersion: "5.4.0",
          latestVersion: "5.4.4",
          ecosystem: "npm",
          updateType: "patch",
          isSecurity: true,
          advisorySeverity: "high",
          advisoryUrl: null,
          source: "package.json",
          isDev: true,
        },
      ],
      allUpdates: [
        {
          name: "next",
          currentVersion: "14.2.0",
          latestVersion: "14.2.9",
          ecosystem: "npm",
          updateType: "patch",
          isSecurity: true,
          advisorySeverity: "critical",
          advisoryUrl: null,
          source: "package.json",
          isDev: false,
        },
        {
          name: "vite",
          currentVersion: "5.4.0",
          latestVersion: "5.4.4",
          ecosystem: "npm",
          updateType: "patch",
          isSecurity: true,
          advisorySeverity: "high",
          advisoryUrl: null,
          source: "package.json",
          isDev: true,
        },
        {
          name: "react",
          currentVersion: "18.2.0",
          latestVersion: "19.0.0",
          ecosystem: "npm",
          updateType: "major",
          isSecurity: false,
          advisorySeverity: null,
          advisoryUrl: null,
          source: "package.json",
          isDev: false,
        },
      ],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: null,
      latestCodeScanDetail: null,
      issueLinks: [],
      dashboardReady: true,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [],
        verifyNow: [],
        fixNext: [
          {
            stableKey: "update:7:npm:react",
            projectId: 7,
            environmentUrl: "https://example.com",
            kind: "update",
            status: "new",
            severity: "medium",
            title: "react is outdated",
            summary: "18.2.0 -> 19.0.0",
            category: "updates",
            domain: null,
            packageName: "react",
            target: {
              page: "updates",
              projectId: 7,
              url: "https://example.com",
              itemId: "npm:react",
            },
            firstSeenAt: "2026-04-15T11:00:00Z",
            lastSeenAt: "2026-04-15T12:00:00Z",
            lastVerifiedAt: null,
            lastStatusChangedAt: "2026-04-15T12:00:00Z",
          },
        ],
        maintenance: [],
      },
      refreshDashboard: vi.fn(),
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(screen.getByText("No scans yet")).toBeInTheDocument();
    });

    expect(screen.getAllByText(/\bnext\b/).length).toBeGreaterThan(0);
    expect(screen.queryByText(/\bvite\b/)).not.toBeInTheDocument();
    expect(screen.queryByText("react is outdated")).not.toBeInTheDocument();
    expect(screen.queryByText(/1 critical package security update/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /alerts/i })).not.toBeInTheDocument();
  });

  it("shows a truthful retry state when the issues view cannot load", async () => {
    const refreshDashboard = vi.fn();

    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [],
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: null,
      latestCodeScanDetail: null,
      issueLinks: [],
      dashboardReady: true,
      dashboardLoadError: "offline",
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [],
        verifyNow: [],
        fixNext: [],
        maintenance: [],
      },
      refreshDashboard,
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath={null}
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    expect(screen.getByText("Issues could not load")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(refreshDashboard).toHaveBeenCalled();
  });

  it("keeps the sidebar issues badge stable while the issues snapshot is still loading", () => {
    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [],
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: null,
      latestCodeScanDetail: null,
      issueLinks: [],
      dashboardReady: false,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [],
        verifyNow: [],
        fixNext: [],
        maintenance: [],
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
      refreshDashboard: vi.fn(),
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath={null}
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    expect(screen.getByLabelText("Issues data loading state")).toBeInTheDocument();
    expect(screen.getAllByText("Issues").length).toBeGreaterThan(0);
  });

  it("renders canonical Code issue groups when only a scan summary is cached", async () => {
    activeIssueGroups = [
      webIssueGroup(buildWebIssue()),
      codeIssueGroup(buildCodeIssue()),
      codeIssueGroup(
        buildCodeIssue({
          id: "code-ops-timeout",
          checkId: "code_scan.code-ops-timeout",
          category: "operations",
          domain: "operations",
          title: "Worker timeout is too short",
          description: "Background jobs can time out before database work completes.",
          relativePath: "workers/queue.ts",
          absolutePath: "/tmp/example/workers/queue.ts",
        }),
      ),
    ];
    routeCodeDetailInvokes([
      () =>
        Promise.resolve({
          id: 501,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 68,
          issueCount: 4,
          criticalCount: 1,
          highCount: 2,
          mediumCount: 1,
          lowCount: 0,
          durationMs: 880,
          checkedAt: "2026-04-15T12:00:00Z",
          framework: "react",
          domainSummaries: [],
          issues: [
            buildCodeIssue(),
            buildCodeIssue({
              id: "code-ops-timeout",
              category: "operations",
              domain: "operations",
              title: "Worker timeout is too short",
              description: "Background jobs can time out before database work completes.",
              relativePath: "workers/queue.ts",
              absolutePath: "/tmp/example/workers/queue.ts",
            }),
          ],
        }),
    ]);

    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: [buildWebIssue()],
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: null,
      latestCodeScanSummary: {
        id: 501,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 68,
        issueCount: 4,
        criticalCount: 1,
        highCount: 2,
        durationMs: 880,
        checkedAt: "2026-04-15T12:00:00Z",
        framework: "react",
        topDomain: "security",
        topDomainCount: 3,
        domainSummaries: [],
      },
      latestCodeScanDetail: null,
      issueLinks: [],
      dashboardReady: true,
      dashboardLoadError: null,
      dismissedIds: new Set<string>(),
      dismissedProjectId: 7,
      workQueue: {
        resumeNow: [],
        verifyNow: [],
        fixNext: [],
        maintenance: [],
      },
      refreshDashboard: vi.fn(),
    });

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(screen.getByRole("combobox", { name: /issue source/i })).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /environment file is committed/i }),
      ).toBeInTheDocument();
    });

    expect(
      screen.getByRole("button", { name: /worker timeout is too short/i }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Code Scan issues from the latest scan/i)).not.toBeInTheDocument();
  });

  it("keeps canonical Code issue groups when a newer summary has no embedded detail", async () => {
    activeIssueGroups = [codeIssueGroup(buildCodeIssue())];
    routeCodeDetailInvokes([
      () =>
        Promise.resolve({
          id: 501,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 68,
          issueCount: 1,
          criticalCount: 0,
          highCount: 1,
          mediumCount: 0,
          lowCount: 0,
          durationMs: 880,
          checkedAt: "2026-04-15T12:00:00Z",
          framework: "react",
          domainSummaries: [],
          issues: [buildCodeIssue()],
        }),
      () => Promise.resolve(null),
    ]);

    useDashboardDataMock.mockReturnValue(
      buildDashboardState({
        latestCodeScanSummary: {
          id: 501,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 68,
          issueCount: 1,
          criticalCount: 0,
          highCount: 1,
          durationMs: 880,
          checkedAt: "2026-04-15T12:00:00Z",
          framework: "react",
          topDomain: "security",
          topDomainCount: 1,
          domainSummaries: [],
        },
      }),
    );

    const view = render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /environment file is committed/i }),
      ).toBeInTheDocument();
    });

    useDashboardDataMock.mockReturnValue(
      buildDashboardState({
        latestCodeScanSummary: {
          id: 777,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 74,
          issueCount: 2,
          criticalCount: 0,
          highCount: 2,
          durationMs: 910,
          checkedAt: "2026-04-16T12:00:00Z",
          framework: "react",
          topDomain: "security",
          topDomainCount: 2,
          domainSummaries: [],
        },
      }),
    );

    view.rerender(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /environment file is committed/i }),
      ).toBeInTheDocument();
    });
  });

  it("keeps canonical Code issue groups when presentation-detail loading fails", async () => {
    activeIssueGroups = [codeIssueGroup(buildCodeIssue())];
    routeCodeDetailInvokes([
      () =>
        Promise.resolve({
          id: 501,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 68,
          issueCount: 1,
          criticalCount: 0,
          highCount: 1,
          mediumCount: 0,
          lowCount: 0,
          durationMs: 880,
          checkedAt: "2026-04-15T12:00:00Z",
          framework: "react",
          domainSummaries: [],
          issues: [buildCodeIssue()],
        }),
      () => Promise.reject(new Error("detail fetch failed")),
    ]);

    useDashboardDataMock.mockReturnValue(
      buildDashboardState({
        latestCodeScanSummary: {
          id: 501,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 68,
          issueCount: 1,
          criticalCount: 0,
          highCount: 1,
          durationMs: 880,
          checkedAt: "2026-04-15T12:00:00Z",
          framework: "react",
          topDomain: "security",
          topDomainCount: 1,
          domainSummaries: [],
        },
      }),
    );

    const view = render(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /environment file is committed/i }),
      ).toBeInTheDocument();
    });

    useDashboardDataMock.mockReturnValue(
      buildDashboardState({
        latestCodeScanSummary: {
          id: 778,
          projectId: 7,
          environmentUrl: "https://example.com",
          overallScore: 71,
          issueCount: 2,
          criticalCount: 0,
          highCount: 2,
          durationMs: 920,
          checkedAt: "2026-04-17T12:00:00Z",
          framework: "react",
          topDomain: "security",
          topDomainCount: 2,
          domainSummaries: [],
        },
      }),
    );

    view.rerender(
      withAppContext(
        <IssuesPage
          projectId={7}
          environmentId={77}
          url="https://example.com"
          latestResult={null}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /environment file is committed/i }),
      ).toBeInTheDocument();
    });
  });

  describe("consolidated reset effect", () => {
    function defaultProps() {
      return {
        projectId: 7,
        environmentId: 77,
        latestResult: null,
        latestCodeResult: null,
        projectPath: "/tmp/example",
        onNavigate: vi.fn(),
        openScanConfig: vi.fn(),
      };
    }

    it("clears dismissals, closes the dossier, and resets the status filter when the project URL changes", async () => {
      const issue = buildWebIssue();
      activeIssueGroups = [webIssueGroup(issue)];

      useDashboardDataMock.mockReturnValue(
        buildDashboardState({
          aggregatedFailedIssues: [issue],
        }),
      );
      const props = defaultProps();
      const { rerender } = render(
        withAppContext(<IssuesPage {...props} url="https://a.example" />),
      );

      fireEvent.click(await screen.findByRole("button", { name: /missing canonical tag/i }));
      expect(await screen.findByText("Selected issue: Missing canonical tag")).toBeInTheDocument();

      fireEvent.click(screen.getByRole("button", { name: "Dismiss issue" }));
      await waitFor(() => {
        expect(
          screen.queryByRole("button", { name: /missing canonical tag/i }),
        ).not.toBeInTheDocument();
      });

      rerender(withAppContext(<IssuesPage {...props} url="https://b.example" />));

      await waitFor(() => {
        expect(screen.getByRole("button", { name: /missing canonical tag/i })).toBeInTheDocument();
      });
      expect(screen.queryByText("Selected issue: Missing canonical tag")).not.toBeInTheDocument();
    });

    it("preserves dismissals when initialFocus changes, but switches to the Issues tab and clears the dossier", async () => {
      const issue = buildWebIssue();
      activeIssueGroups = [webIssueGroup(issue)];

      useDashboardDataMock.mockReturnValue(
        buildDashboardState({
          aggregatedFailedIssues: [issue],
        }),
      );
      const props = defaultProps();
      const { rerender } = render(
        withAppContext(<IssuesPage {...props} url="https://a.example" />),
      );

      // Move to the History tab so we can prove the focus change pulls us
      // back to Issues.
      fireEvent.click(screen.getByRole("button", { name: "History" }));
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "History" }).className).toContain("tab-active");
      });

      // Dismiss the issue from the queue, then reopen the Issues tab to read
      // the current dismissed state from the rendered list.
      fireEvent.click(screen.getByRole("button", { name: "Issues" }));
      fireEvent.click(await screen.findByRole("button", { name: /missing canonical tag/i }));
      expect(await screen.findByText("Selected issue: Missing canonical tag")).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Dismiss issue" }));
      await waitFor(() => {
        expect(
          screen.queryByRole("button", { name: /missing canonical tag/i }),
        ).not.toBeInTheDocument();
      });

      // Move back to History so the focus change has a tab to overcome.
      fireEvent.click(screen.getByRole("button", { name: "History" }));
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "History" }).className).toContain("tab-active");
      });

      // Change the deep-link focus target - the focus-driven branch should
      // snap us back to the Issues tab without clearing the dismissed state.
      rerender(
        withAppContext(<IssuesPage {...props} url="https://a.example" />, {
          navigation: { issuesTarget: { focus: getIssuesWebCategoryFocus("seo") } },
        }),
      );

      await waitFor(() => {
        expect(screen.getByRole("button", { name: "Issues" }).className).toContain("tab-active");
      });
      // Dismissal must survive a focus change.
      expect(
        screen.queryByRole("button", { name: /missing canonical tag/i }),
      ).not.toBeInTheDocument();
    });

    it("snaps back to the Issues tab when tabResetKey changes without clearing dismissals", async () => {
      const issue = buildWebIssue();
      activeIssueGroups = [webIssueGroup(issue)];

      useDashboardDataMock.mockReturnValue(
        buildDashboardState({
          aggregatedFailedIssues: [issue],
        }),
      );
      const props = defaultProps();
      const { rerender } = render(
        withAppContext(<IssuesPage {...props} url="https://a.example" />, {
          navigation: { issuesTabResetKey: 1 },
        }),
      );

      // Dismiss the issue, then switch off Issues so the tabResetKey change
      // has something to reset.
      fireEvent.click(await screen.findByRole("button", { name: /missing canonical tag/i }));
      expect(await screen.findByText("Selected issue: Missing canonical tag")).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Dismiss issue" }));
      await waitFor(() => {
        expect(
          screen.queryByRole("button", { name: /missing canonical tag/i }),
        ).not.toBeInTheDocument();
      });
      fireEvent.click(screen.getByRole("button", { name: "History" }));
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "History" }).className).toContain("tab-active");
      });

      // Bumping issuesTabResetKey simulates the user clicking the Issues nav
      // while already on this page.
      rerender(
        withAppContext(<IssuesPage {...props} url="https://a.example" />, {
          navigation: { issuesTabResetKey: 2 },
        }),
      );

      await waitFor(() => {
        expect(screen.getByRole("button", { name: "Issues" }).className).toContain("tab-active");
      });
      // Dismissal must survive a tabResetKey bump.
      expect(
        screen.queryByRole("button", { name: /missing canonical tag/i }),
      ).not.toBeInTheDocument();
    });
  });
});
