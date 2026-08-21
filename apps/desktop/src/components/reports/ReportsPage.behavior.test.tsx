import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, openUrlMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openUrlMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));
vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@/app/ShellHeader", () => ({
  HeaderActions: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
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

import { ReportsPage } from "./ReportsPage";
import { withQueryClient } from "@/test-utils/query-client";

function renderReports(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

function buildReportData() {
  return {
    reportTitle: "Site & Code Report",
    branding: {
      companyName: "SiteCMD",
      logoPath: null,
      logoDataUrl: null,
      logoName: null,
      primaryColor: "#2563eb",
      footerText: "Confidential",
      clientName: null,
      hideAttribution: false,
    },
    sections: {
      executiveSummary: true,
      categoryBreakdown: true,
      topIssues: true,
      recommendations: true,
      codeScan: true,
      analytics: true,
      uptime: true,
      deploys: true,
    },
    latestScanDate: "2026-04-15T12:00:00Z",
    siteScore: {
      currentScore: 76,
      issuesCritical: 1,
      issuesHigh: 3,
      issuesMedium: 1,
      issuesLow: 0,
      issuesTotal: 5,
    },
    health: {
      currentScore: 82,
      issuesCritical: 1,
      issuesHigh: 2,
      issuesTotal: 5,
    },
    codeScan: {
      currentScore: 74,
      criticalCount: 0,
      highCount: 1,
      issueCount: 2,
      topDomain: "security",
      domainTrend: "stable",
      checkedAt: "2026-04-15T12:00:00Z",
    },
    analytics: null,
    uptime: null,
    deploys: null,
  };
}

describe("ReportsPage behavior", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openUrlMock.mockReset();
    localStorage.clear();
  });

  it("renders the report builder for every install with nothing to sell", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_report_history") return Promise.resolve([]);
      if (command === "generate_report_data") return Promise.resolve(buildReportData());
      return Promise.resolve(null);
    });

    renderReports(
      <ReportsPage projectId={7} siteUrl="https://example.com" projectPath="/tmp/example" />,
    );

    await waitFor(() => {
      expect(
        screen.getByText("This export covers Web Scan issues and linked Code Scan results."),
      ).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: /Upgrade for reports/i })).not.toBeInTheDocument();
    expect(screen.queryByText("Export professional")).not.toBeInTheDocument();
    expect(openUrlMock).not.toHaveBeenCalled();
  });

  it("shows a page-shaped loading skeleton while report data is loading", () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_report_history" || command === "generate_report_data") {
        return new Promise(() => {});
      }
      return Promise.resolve(null);
    });

    renderReports(
      <ReportsPage projectId={7} siteUrl="https://example.com" projectPath="/tmp/example" />,
    );

    expect(screen.getByLabelText("Reports loading state")).toBeInTheDocument();
  });

  it("shows a truthful empty state when no site is selected for reports", () => {
    renderReports(<ReportsPage projectId={null} siteUrl="" projectPath={null} />);

    expect(screen.getByText("No site selected")).toBeInTheDocument();
  });

  it("renders the real preview flow instead of only helper-level report tests", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_report_history") {
        return Promise.resolve([]);
      }
      if (command === "generate_report_data") {
        return Promise.resolve(buildReportData());
      }
      if (command === "render_report_html_from_data") {
        return Promise.resolve("<html><body><h1>Preview Ready</h1></body></html>");
      }
      if (command === "save_report_history") {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    renderReports(
      <ReportsPage projectId={7} siteUrl="https://example.com" projectPath="/tmp/example" />,
    );

    await waitFor(() => {
      expect(
        screen.getByText("This export covers Web Scan issues and linked Code Scan results."),
      ).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /Generate Report/i }));

    await waitFor(() => {
      expect(screen.getByTitle("Report Preview")).toBeInTheDocument();
    });

    expect(invokeMock).toHaveBeenCalledWith("render_report_html_from_data", expect.anything());
    expect(invokeMock).toHaveBeenCalledWith("save_report_history", expect.anything());
  });
});
