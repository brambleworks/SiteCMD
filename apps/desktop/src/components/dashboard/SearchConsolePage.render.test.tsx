import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const {
  invokeMock,
  useTierMock,
  useDesktopPromptCenterMock,
  usePendingVerificationCenterMock,
  addJobMock,
  completeJobMock,
  failJobMock,
  sendActionableDesktopNotificationMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  useTierMock: vi.fn(),
  useDesktopPromptCenterMock: vi.fn(),
  usePendingVerificationCenterMock: vi.fn(),
  addJobMock: vi.fn(),
  completeJobMock: vi.fn(),
  failJobMock: vi.fn(),
  sendActionableDesktopNotificationMock: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/lib/scan-execution-adapters", () => ({
  getScanHistory: (args: unknown) => invokeMock("get_scan_executions", args),
  getScanDetail: (args: unknown) => invokeMock("get_scan_execution_detail", args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@/app/ShellHeader", () => ({
  HeaderActions: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));
vi.mock("@/hooks/useTier", () => ({
  useTier: () => useTierMock(),
}));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  }),
}));
vi.mock("@/lib/jobs", () => ({
  addJob: addJobMock,
  completeJob: completeJobMock,
  failJob: failJobMock,
}));
vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: {
      desktopNotifications: true,
    },
  }),
}));
vi.mock("@/lib/actionable-notifications", () => ({
  sendActionableDesktopNotification: sendActionableDesktopNotificationMock,
}));
vi.mock("@/components/ui/card", () => ({
  Card: ({ children }: { children: React.ReactNode }) => React.createElement("div", null, children),
  CardContent: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));
vi.mock("@/components/ui/button", () => ({
  Button: ({
    children,
    type = "button",
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) =>
    React.createElement("button", { type, ...props }, children),
}));
vi.mock("@/components/ui/markdown", () => ({
  Markdown: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));
vi.mock("@/components/issues/IssueActionBar", () => ({
  IssueActionBar: ({ extraActions }: { extraActions?: React.ReactNode }) =>
    React.createElement("div", null, "IssueActionBar", extraActions),
}));
vi.mock("@/components/issues/FixWithAgentAction", () => ({
  FixWithAgentAction: () => React.createElement("div", null, "FixWithAgentAction"),
}));
vi.mock("@/components/issues/IssueDossierPanel", async () => {
  const { buildDossierPanelMock } = await import("@/test-utils/dossier-panel-mock");
  return buildDossierPanelMock();
});
vi.mock("@/components/issues/CommandExecutionPanel", () => ({
  CommandExecutionPanel: () => null,
}));
vi.mock("@/components/issues/IssueMemorySection", () => ({
  IssueMemorySection: () => null,
  IssueMemoryRail: () => null,
}));
vi.mock("@/components/issues/RecentWatchedFileSection", () => ({
  RecentWatchedFileSection: () => React.createElement("div", null, "RecentWatchedFileSection"),
}));
vi.mock("@/components/issues/WatchedFileArrivalBanner", () => ({
  WatchedFileArrivalBanner: () => React.createElement("div", null, "WatchedFileArrivalBanner"),
}));
vi.mock("@/components/issues/IssueScopeSummary", () => ({
  IssueScopeInline: () => null,
  IssueScopeSection: () => null,
}));
vi.mock("@/components/ui/FixGuideSteps", () => ({
  FixGuideSteps: () => React.createElement("div", null, "FixGuideSteps"),
}));
vi.mock("@/components/settings/InlineIntegrationSetup", () => ({
  InlineIntegrationSetup: ({
    serviceTypes,
    allowReconnect = [],
  }: {
    serviceTypes: string[];
    allowReconnect?: string[];
  }) =>
    React.createElement("div", {
      "data-testid": "inline-integration-setup",
      "data-services": serviceTypes.join(","),
      "data-reconnect": allowReconnect.join(","),
    }),
}));
vi.mock("@/components/ui/surface-meta", () => ({
  FreshnessBadge: () => null,
  ScopeBadge: () => null,
}));
vi.mock("@/lib/desktop-actions", () => ({
  extractDesktopCommands: vi.fn(() => []),
  openPathInEditor: vi.fn(() => Promise.resolve()),
  revealPath: vi.fn(() => Promise.resolve()),
  runProjectCommand: vi.fn(() => Promise.resolve({ success: true, stdout: "", stderr: "" })),
}));
vi.mock("@/lib/desktop-prompts", () => ({
  getLatestDesktopPrompt: vi.fn(() => null),
  useDesktopPromptCenter: () => useDesktopPromptCenterMock(),
}));
vi.mock("@/lib/issue-scope", () => ({
  getCheckIssueScope: vi.fn(() => ({ issueLabel: "SEO check" })),
}));
vi.mock("@/lib/pending-verification", () => ({
  buildPendingVerificationId: vi.fn(() => "pending-id"),
  queuePendingVerification: vi.fn(),
  resolvePendingVerification: vi.fn(),
  usePendingVerificationCenter: () => usePendingVerificationCenterMock(),
}));
vi.mock("@/lib/action-language", () => ({
  getCopyActionLabel: vi.fn(() => "Copy Fix Prompt"),
  getVerificationActionLabel: vi.fn(() => "Verify now"),
}));
vi.mock("@/lib/fix-guides", () => ({
  getFixGuide: vi.fn(() => null),
  // The catalog-first loader normalizes the check id before its lookups;
  // identity is enough for these render tests.
  normalizeFixGuideKey: vi.fn((checkId: string) => checkId),
}));

import { SearchConsolePage } from "./SearchConsolePage";
import { __resetAnalyticsSnapshotCacheForTests } from "@/lib/analytics-snapshot-cache";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderSearchConsolePage(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient(createTestQueryClient()) });
}

function buildScanDetail(issues: Array<Record<string, unknown>>) {
  return {
    url: "https://example.com",
    mode: "live",
    scanType: "health",
    overallScore: 82,
    categories: [
      {
        category: "seo",
        score: 76,
        issuesTotal: issues.length,
        issuesCritical: 0,
        issuesHigh: 1,
        issuesMedium: Math.max(issues.length - 1, 0),
        issuesLow: 0,
        issuesPassed: 0,
      },
    ],
    issues,
    detectedStack: null,
    durationMs: 1200,
    timestamp: "2026-04-11T12:00:00Z",
  };
}

describe("SearchConsolePage autofocus", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    window.localStorage.clear();
    __resetAnalyticsSnapshotCacheForTests();
    useTierMock.mockReturnValue({
      hasFeature: vi.fn(() => false),
      isLoading: false,
      licenseInfo: {
        checkoutUrls: { core: "https://example.com/core", pro: "https://example.com/pro" },
      },
    });
    useDesktopPromptCenterMock.mockReturnValue([]);
    usePendingVerificationCenterMock.mockReturnValue([]);
    addJobMock.mockReset();
    completeJobMock.mockReset();
    failJobMock.mockReset();
    sendActionableDesktopNotificationMock.mockReset();
    sendActionableDesktopNotificationMock.mockResolvedValue(undefined);
    Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
  });

  it("frames Search & SEO as the search visibility surface", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_scan_executions":
          return [{ id: 101, scanType: "health", timestamp: "2026-04-11T12:00:00Z" }];
        case "get_scan_execution_detail":
          return buildScanDetail([
            {
              checkId: "seo.robots_txt",
              category: "seo",
              title: "Robots.txt blocks crawling",
              description: "Search engines cannot crawl key pages.",
              status: "fail",
              severity: "high",
              fixPrompt: null,
              manualFix: null,
              rawData: null,
            },
          ]);
        default:
          return null;
      }
    });

    renderSearchConsolePage(
      <SearchConsolePage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    // Wait for scan data to land so the SEO panels render.
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_scan_execution_detail", expect.any(Object)),
    );
    expect(screen.queryByText("See what changed in search visibility")).not.toBeInTheDocument();
  });

  it("prompts to reconnect when Search Console is configured but its token expired", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_scan_executions":
          return [];
        case "fetch_analytics":
          // Configured but the Google sign-in expired: an error, no data. This
          // must read as "reconnect", not the first-time "Connect..." copy.
          return {
            search_console_error:
              "Google sign-in expired. Reconnect Search Console to refresh the data.",
          };
        case "get_integrations":
          return [{ integrationType: "googlesearchconsole" }];
        default:
          return null;
      }
    });

    renderSearchConsolePage(
      <SearchConsolePage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    await waitFor(() => {
      const gscSetup = screen
        .getAllByTestId("inline-integration-setup")
        .find((el) => el.getAttribute("data-services") === "googlesearchconsole");
      expect(gscSetup).toHaveAttribute("data-reconnect", "googlesearchconsole");
    });
    expect(screen.getByText(/sign in again to reconnect/i)).toBeInTheDocument();
  });

  it("auto-opens the dossier when the incoming SEO focus narrows to one issue", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_scan_executions":
          return [{ id: 101, scanType: "health", timestamp: "2026-04-11T12:00:00Z" }];
        case "get_scan_execution_detail":
          return buildScanDetail([
            {
              checkId: "seo.robots_txt",
              category: "seo",
              title: "Robots.txt blocks crawling",
              description: "Search engines cannot crawl key pages.",
              status: "fail",
              severity: "high",
              fixPrompt: null,
              manualFix: null,
              rawData: null,
            },
            {
              checkId: "seo.title",
              category: "seo",
              title: "Title tag is missing",
              description: "Pages need better titles.",
              status: "fail",
              severity: "medium",
              fixPrompt: null,
              manualFix: null,
              rawData: null,
            },
          ]);
        default:
          return null;
      }
    });

    renderSearchConsolePage(
      <SearchConsolePage
        projectId={7}
        url="https://example.com"
        onNavigate={vi.fn()}
        initialFocus="seo.robots"
      />,
    );

    const dossier = await screen.findByTestId("issue-dossier");
    expect(dossier).toHaveTextContent("Robots.txt blocks crawling");
    // The dossier hands fixes to the agent loop; the static copy-prompt panel is gone.
    expect(dossier).toHaveTextContent("FixWithAgentAction");
  });

  it("does not auto-open a dossier when the focus still matches multiple SEO issues", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_scan_executions":
          return [{ id: 102, scanType: "health", timestamp: "2026-04-11T12:00:00Z" }];
        case "get_scan_execution_detail":
          return buildScanDetail([
            {
              checkId: "seo.title",
              category: "seo",
              title: "Title tag is missing",
              description: "Pages need better titles.",
              status: "fail",
              severity: "medium",
              fixPrompt: null,
              manualFix: null,
              rawData: null,
            },
            {
              checkId: "seo.page_title_duplicate",
              category: "seo",
              title: "Page title is duplicated",
              description: "Pages reuse the same title.",
              status: "fail",
              severity: "medium",
              fixPrompt: null,
              manualFix: null,
              rawData: null,
            },
          ]);
        default:
          return null;
      }
    });

    renderSearchConsolePage(
      <SearchConsolePage
        projectId={7}
        url="https://example.com"
        onNavigate={vi.fn()}
        initialFocus="seo.titles"
      />,
    );

    // Wait for scan data to land so the SEO panels render.
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_scan_execution_detail", expect.any(Object)),
    );
    // Multiple matching issues should NOT auto-open a dossier - that's the contract.
    expect(screen.queryByTestId("issue-dossier")).not.toBeInTheDocument();
  });

  it("keeps grouped Search & SEO verify jobs pointed at the strongest remaining issue", async () => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => "hidden",
    });
    usePendingVerificationCenterMock.mockReturnValue([
      {
        id: "7:https://example.com:search-console:seo.robots_txt",
        projectId: 7,
        url: "https://example.com",
        itemId: "seo.robots_txt",
        label: "Robots.txt blocks crawling",
        reason: "Re-check robots-related SEO work before moving on.",
        page: "search-console",
        focus: "seo.robots",
        filePath: null,
        createdAt: 2,
        updatedAt: 2,
      },
      {
        id: "7:https://example.com:search-console:seo.title",
        projectId: 7,
        url: "https://example.com",
        itemId: "seo.title",
        label: "Title tag is missing",
        reason: "Re-check title-related SEO work before moving on.",
        page: "search-console",
        focus: "seo.titles",
        filePath: null,
        createdAt: 1,
        updatedAt: 1,
      },
    ]);

    invokeMock.mockImplementation(async (command: string, args?: { checkIds?: string[] }) => {
      switch (command) {
        case "get_scan_executions":
          return [{ id: 103, scanType: "health", timestamp: "2026-04-11T12:00:00Z" }];
        case "get_scan_execution_detail":
          return buildScanDetail([
            {
              checkId: "seo.robots_txt",
              category: "seo",
              title: "Robots.txt blocks crawling",
              description: "Search engines cannot crawl key pages.",
              status: "fail",
              severity: "high",
              fixPrompt: null,
              manualFix: null,
              rawData: null,
            },
            {
              checkId: "seo.title",
              category: "seo",
              title: "Title tag is missing",
              description: "Pages need better titles.",
              status: "fail",
              severity: "medium",
              fixPrompt: null,
              manualFix: null,
              rawData: null,
            },
          ]);
        case "verify_scan_checks":
          if (args?.checkIds?.includes("seo.robots_txt")) {
            return {
              checkedAt: "2026-04-11T12:05:00Z",
              results: [
                {
                  checkId: "seo.robots_txt",
                  category: "seo",
                  title: "Robots.txt blocks crawling",
                  description: "Search engines cannot crawl key pages.",
                  status: "pass",
                  severity: "high",
                  fixPrompt: null,
                  manualFix: null,
                  rawData: null,
                },
              ],
            };
          }

          return {
            checkedAt: "2026-04-11T12:06:00Z",
            results: [
              {
                checkId: "seo.title",
                category: "seo",
                title: "Title tag is missing",
                description: "Pages need better titles.",
                status: "fail",
                severity: "medium",
                fixPrompt: null,
                manualFix: null,
                rawData: null,
              },
            ],
          };
        default:
          return null;
      }
    });

    renderSearchConsolePage(
      <SearchConsolePage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    // Title appears in both the pending-verification summary and the issue list - findAllByText handles the duplication.
    expect((await screen.findAllByText("Robots.txt blocks crawling")).length).toBeGreaterThan(0);
    fireEvent.click(await screen.findByRole("button", { name: "Verify all" }));

    await waitFor(() => {
      expect(completeJobMock).toHaveBeenCalledWith(
        "search-verify-all:7:https://example.com",
        expect.objectContaining({
          label: "Search & SEO checks still open",
          target: {
            page: "search-console",
            projectId: 7,
            url: "https://example.com",
            itemId: "seo.title",
            focus: "seo.titles",
          },
        }),
      );
    });

    expect(invokeMock).toHaveBeenCalledWith(
      "verify_scan_checks",
      expect.objectContaining({
        projectId: 7,
        url: "https://example.com",
      }),
    );

    expect(invokeMock).toHaveBeenCalledWith(
      "record_search_event",
      expect.objectContaining({
        projectId: 7,
        title: "Search & SEO checks still open",
        detail: expect.stringContaining('"page":"search-console"'),
        severity: "warning",
      }),
    );
  });
});
