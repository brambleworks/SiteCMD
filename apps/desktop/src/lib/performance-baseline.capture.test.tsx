import React from "react";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

declare const process:
  | {
      env?: {
        SITECMD_PERF_BASELINE?: string;
      };
    }
  | undefined;

const describePerformanceBaseline =
  typeof process !== "undefined" && process.env?.SITECMD_PERF_BASELINE === "1"
    ? describe
    : describe.skip;

const {
  invokeMock,
  useDashboardDataMock,
  usePendingVerificationCenterMock,
  useDesktopPromptCenterMock,
  useEventsMock,
  useProjectMock,
  useToastMock,
  getProjectSignalSnapshotMock,
  peekDashboardSnapshotMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  useDashboardDataMock: vi.fn(),
  usePendingVerificationCenterMock: vi.fn(),
  useDesktopPromptCenterMock: vi.fn(),
  useEventsMock: vi.fn(),
  useProjectMock: vi.fn(),
  useToastMock: vi.fn(),
  getProjectSignalSnapshotMock: vi.fn(),
  peekDashboardSnapshotMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn((_localStorageKey: string, _storeKey: string, fallback: unknown) =>
    Promise.resolve(fallback),
  ),
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

vi.mock("@/hooks/useEvents", () => ({
  useEvents: (...args: unknown[]) => useEventsMock(...args),
}));

vi.mock("@/hooks/useProject", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../hooks/useProject")>();
  return {
    ...actual,
    useProject: () => useProjectMock(),
  };
});

vi.mock("@/hooks/useToast", () => ({
  useToast: () => useToastMock(),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    tier: "free",
    hasFeature: () => false,
  }),
}));

vi.mock("@/app/ShellHeader", () => ({
  HeaderActions: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));

vi.mock("react-virtuoso", () => ({
  Virtuoso: ({
    data,
    itemContent,
  }: {
    data: unknown[];
    itemContent: (index: number, item: unknown) => React.ReactNode;
  }) =>
    React.createElement(
      "div",
      null,
      data.map((item, index) =>
        React.createElement("div", { key: index }, itemContent(index, item)),
      ),
    ),
}));

vi.mock("@/lib/project-summary-signals", () => ({
  getProjectSignalSnapshot: (...args: unknown[]) => getProjectSignalSnapshotMock(...args),
  peekDashboardSnapshot: (...args: unknown[]) => peekDashboardSnapshotMock(...args),
}));

vi.mock("@/components/issues/IssueDossier", () => ({
  IssueDossier: () => null,
}));

vi.mock("@/components/scan/ScanHistory", () => ({
  ScanHistory: () => null,
}));

import { EventsPage } from "@/components/events/EventsPage";
import { IssuesPage } from "@/pages/IssuesPage";
import { withAppContext } from "@/test-utils/app-context";
import { withQueryClient } from "@/test-utils/query-client";
import type { CheckResult, IssueGroup, SiteEvent } from "@/lib/types";
import {
  PERFORMANCE_BUDGETS,
  clearPerformanceSnapshot,
  readPerformanceSnapshot,
} from "./performance-metrics";

const WEB_CATEGORIES = ["seo", "performance", "config"] as const;
const WEB_STATUSES = ["warn", "fail"] as const;
const WEB_SEVERITIES = ["high", "medium"] as const;
const EVENT_TYPES = ["scan", "deploy", "analytics", "update"] as const;
const EVENT_SEVERITIES = ["warning", "info"] as const;

function buildWebIssueGroup(index: number): IssueGroup {
  const issue = buildWebIssue(index);
  const severity = WEB_SEVERITIES[index % WEB_SEVERITIES.length];
  return {
    checkId: issue.checkId,
    category: issue.category,
    severity,
    title: issue.title,
    description: issue.description,
    instances: [
      {
        id: index + 1,
        source: "web_scan",
        signalId: issue.checkId,
        producerCheckId: issue.checkId,
        url: `https://example.com/page-${index + 1}`,
        pageUrl: null,
        severity,
        title: issue.title,
        description: issue.description,
        detailJson: null,
        firstSeenAt: 0,
        lastSeenAt: 0,
        confidence: "high",
        domain: null,
        relativePath: null,
        line: null,
      },
    ],
    sources: ["web_scan"],
    status: "new",
    snoozeUntil: null,
    blockReason: null,
    impactScore: 5,
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
    displayConfidence: "high",
    observationCount: 1,
    anomalyScore: null,
  };
}

function buildWebIssue(index: number): CheckResult {
  return {
    checkId: `seo.issue-${index}`,
    category: WEB_CATEGORIES[index % WEB_CATEGORIES.length],
    title: `Issue ${index + 1}`,
    description: `Representative issue description ${index + 1}`,
    status: WEB_STATUSES[index % WEB_STATUSES.length],
    severity: WEB_SEVERITIES[index % WEB_SEVERITIES.length],
    fixPrompt: "Open the right file, change the affected metadata, then rerun the scan.",
    manualFix: "Update the related page or config and rerun the scan.",
    rawData: {
      url: `https://example.com/page-${index + 1}`,
      issue_index: index + 1,
    },
    confidence: "high",
    whyItMatters: "This can hurt discoverability and trust if it stays unresolved.",
  };
}

function buildEvent(index: number): SiteEvent {
  return {
    id: index + 1,
    projectId: 7,
    eventType: EVENT_TYPES[index % EVENT_TYPES.length],
    severity: EVENT_SEVERITIES[index % EVENT_SEVERITIES.length],
    occurredAtMs: Date.UTC(2026, 3, 14, 12, 0, 0) - index * 60_000,
    title: `Event ${index + 1}`,
    summary: `Representative activity summary ${index + 1}`,
    detail: JSON.stringify({
      url: `https://example.com/page-${(index % 24) + 1}`,
      overall_score: 82 - (index % 10),
    }),
    source: "internal" as const,
    sourceId: `event-${index + 1}`,
    metadata: null,
    affectedCheckIds: null,
  };
}

function metricLatestDuration(key: keyof typeof PERFORMANCE_BUDGETS) {
  return readPerformanceSnapshot().find((metric) => metric.key === key)?.latestDurationMs ?? null;
}

function average(values: number[]) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

async function waitForMetric(key: keyof typeof PERFORMANCE_BUDGETS) {
  await waitFor(() => {
    expect(metricLatestDuration(key)).not.toBeNull();
  });
  return metricLatestDuration(key);
}

beforeEach(() => {
  cleanup();
  clearPerformanceSnapshot();
  window.localStorage.clear();
  invokeMock.mockReset();
  useDashboardDataMock.mockReset();
  usePendingVerificationCenterMock.mockReset();
  useDesktopPromptCenterMock.mockReset();
  useEventsMock.mockReset();
  useProjectMock.mockReset();
  useToastMock.mockReset();
  getProjectSignalSnapshotMock.mockReset();
  peekDashboardSnapshotMock.mockReset();

  usePendingVerificationCenterMock.mockReturnValue([]);
  useDesktopPromptCenterMock.mockReturnValue([]);
  useToastMock.mockReturnValue({
    success: vi.fn(),
    info: vi.fn(),
    error: vi.fn(),
  });
  useProjectMock.mockReturnValue({
    activeEnv: { url: "https://example.com" },
  });
  getProjectSignalSnapshotMock.mockResolvedValue({
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
  });
  peekDashboardSnapshotMock.mockReturnValue(null);
});

afterEach(() => {
  cleanup();
  clearPerformanceSnapshot();
});

describePerformanceBaseline("performance baseline harness", () => {
  it("captures a large-result Issues page render baseline", async () => {
    const issues = Array.from({ length: 220 }, (_, index) => buildWebIssue(index));
    const issueGroups = Array.from({ length: 220 }, (_, index) => buildWebIssueGroup(index));
    const samples: number[] = [];

    invokeMock.mockImplementation((command: string) => {
      if (command === "get_work_items") return Promise.resolve(issueGroups);
      return Promise.resolve(null);
    });

    useDashboardDataMock.mockReturnValue({
      aggregatedFailedIssues: issues,
      securityUpdates: [],
      allUpdates: [],
      lastCIRun: null,
      latestDetail: {
        issues,
        url: "https://example.com",
        overall_score: 72,
        scan_type: "health",
        detected_stack: null,
      },
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

    for (let iteration = 0; iteration < 5; iteration += 1) {
      cleanup();
      clearPerformanceSnapshot();

      render(
        withAppContext(
          <IssuesPage
            projectId={7}
            url="https://example.com"
            latestResult={{
              url: "https://example.com",
              mode: "live",
              scanType: "health",
              overallScore: 72,
              categories: [],
              issues,
              detectedStack: null,
              durationMs: 1300,
              timestamp: "2026-04-14T12:00:00Z",
            }}
            latestCodeResult={null}
            projectPath="/tmp/example"
            onNavigate={vi.fn()}
            openScanConfig={vi.fn()}
          />,
        ),
        { wrapper: withQueryClient() },
      );

      await screen.findByText("Issue 1");
      const sample = await waitForMetric("issues.initial_ready_ms");
      expect(sample).not.toBeNull();
      samples.push(sample ?? 0);
    }

    const averageMs = Math.round(average(samples));
    console.info(
      `[perf-baseline] issues_initial_ready_ms avg=${averageMs} samples=${samples.join(",")}`,
    );
    expect(averageMs).toBeLessThanOrEqual(PERFORMANCE_BUDGETS["issues.initial_ready_ms"].budgetMs);
  });

  it("captures a long-lived Activity page load baseline", { timeout: 60000 }, async () => {
    // Reduced fixture size preserves the render path under parallel CI load.
    const events = Array.from({ length: 300 }, (_, index) => buildEvent(index));
    const samples: number[] = [];

    for (let iteration = 0; iteration < 3; iteration += 1) {
      cleanup();
      clearPerformanceSnapshot();
      const state = { loading: true };
      const loadEvents = vi.fn();
      useEventsMock.mockImplementation(() => ({
        events,
        hasMore: true,
        loading: state.loading,
        error: null,
        loadEvents,
        refreshIntegrations: vi.fn(() => Promise.resolve(0)),
      }));

      // EventsPage reads its signal snapshot through the shared query layer,
      // so it needs a QueryClient even with useEvents mocked.
      const view = render(<EventsPage projectId={7} onOpenTarget={vi.fn()} />, {
        wrapper: withQueryClient(),
      });

      await screen.findByText("Event 1");
      state.loading = false;
      view.rerender(<EventsPage projectId={7} onOpenTarget={vi.fn()} />);
      const sample = await waitForMetric("events.initial_ready_ms");
      expect(sample).not.toBeNull();
      samples.push(sample ?? 0);
    }

    const averageMs = Math.round(average(samples));
    console.info(
      `[perf-baseline] events_initial_ready_ms avg=${averageMs} samples=${samples.join(",")}`,
    );
    expect(averageMs).toBeLessThanOrEqual(PERFORMANCE_BUDGETS["events.initial_ready_ms"].budgetMs);
  });
});
