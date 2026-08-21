import React from "react";
import { render as rtlRender, screen } from "@testing-library/react";
import { withQueryClient } from "@/test-utils/query-client";

// IssuesPage uses useInactiveIssueKeys (a useQuery), so it needs a
// QueryClientProvider. A fresh client per render keeps tests isolated.
const render = (ui: Parameters<typeof rtlRender>[0], options?: Parameters<typeof rtlRender>[1]) =>
  rtlRender(ui, { wrapper: withQueryClient(), ...options });
import { beforeEach, describe, expect, it, vi } from "vitest";

const { useDashboardDataMock } = vi.hoisted(() => ({
  useDashboardDataMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@/components/dashboard/useDashboardData", () => ({
  useDashboardData: (...args: unknown[]) => useDashboardDataMock(...args),
}));
vi.mock("@/components/issues/IssueList", () => ({
  IssueList: () => React.createElement("div", null, "Issue list"),
}));
vi.mock("@/components/issues/IssueDossier", () => ({
  IssueDossier: () => null,
}));
vi.mock("@/components/scan/ScanHistory", () => ({
  ScanHistory: () => null,
}));

import { IssuesPage } from "./IssuesPage";
import { withAppContext } from "@/test-utils/app-context";

describe("IssuesPage onboarding follow-up", () => {
  beforeEach(() => {
    window.localStorage.clear();
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
  });

  it("does not render setup nudges above the Issues queue", () => {
    window.localStorage.setItem(
      "sitecmd_setup_pending:7",
      JSON.stringify(["code-scan", "updates"]),
    );

    render(
      withAppContext(
        <IssuesPage
          projectId={7}
          url="https://example.com"
          latestResult={{
            url: "https://example.com",
            mode: "live",
            scanType: "health",
            overallScore: 82,
            categories: [],
            issues: [],
            detectedStack: null,
            durationMs: 1200,
            timestamp: "2026-04-14T12:00:00Z",
          }}
          latestCodeResult={null}
          projectPath="/tmp/example"
          onNavigate={vi.fn()}
          openScanConfig={vi.fn()}
        />,
      ),
    );

    expect(screen.queryByText("Your first Web baseline is ready")).not.toBeInTheDocument();
    expect(screen.queryByText("Check package updates next")).not.toBeInTheDocument();
    expect(screen.getByText("Issue list")).toBeInTheDocument();
  });
});
