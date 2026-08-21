import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ReportData } from "./report-pdf-model";

vi.mock("@/lib/react-pdf-browser", () => ({
  Page: ({ children }: { children: ReactNode }) => <section>{children}</section>,
  Text: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  View: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  StyleSheet: { create: <T,>(styles: T) => styles },
}));

import { CodeScanPage, DeploysReportPage, UptimeReportPage } from "./ReportPDFSections";

function reportData(overrides: Partial<ReportData> = {}): ReportData {
  return {
    siteUrl: "https://example.com",
    projectName: "Example",
    reportTitle: "Site & Code Report",
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
    periodLabel: "Last 30 days",
    periodStart: "2026-01-01T00:00:00Z",
    periodEnd: "2026-01-31T00:00:00Z",
    generatedAt: "2026-02-01T00:00:00Z",
    latestScanDate: "2026-01-31T00:00:00Z",
    siteScore: {
      currentScore: 92,
      issuesTotal: 0,
      issuesCritical: 0,
      issuesHigh: 0,
      issuesMedium: 0,
      issuesLow: 0,
    },
    health: {
      currentScore: 92,
      previousScore: 90,
      trend: "up",
      trendPoints: [],
      issuesTotal: 0,
      issuesCritical: 0,
      issuesHigh: 0,
      issuesMedium: 0,
      issuesLow: 0,
    },
    categories: [],
    topIssues: [],
    resolvedCount: 0,
    codeScan: {
      currentScore: 100,
      previousScore: null,
      trend: "stable",
      issueCount: 0,
      criticalCount: 0,
      highCount: 0,
      mediumCount: 0,
      lowCount: 0,
      checkedAt: "2026-01-31T00:00:00Z",
      framework: "React",
      topDomain: null,
      topDomainCount: 0,
      domainTrend: null,
      domains: [],
      topIssues: [],
    },
    analytics: null,
    uptime: {
      uptimePct: 99.95,
      incidents: 2,
      avgResponseMs: 245,
    },
    deploys: {
      count: 12,
      recent: [
        {
          date: "2026-01-20T12:30:00Z",
          message: "Ship Code Scan CLI",
          author: "Kyle",
        },
      ],
    },
    branding: {
      companyName: "SiteCMD",
      logoPath: null,
      logoDataUrl: null,
      logoName: null,
      primaryColor: "#000000",
      footerText: "Confidential",
      clientName: null,
      hideAttribution: false,
    },
    ...overrides,
  };
}

describe("PDF report contract rendering", () => {
  it("renders a clean Code Scan without obsolete paid-detail copy", () => {
    render(<CodeScanPage data={reportData()} />);

    expect(screen.getByText("Code Scan")).toBeInTheDocument();
    expect(screen.getByText(/Latest Code Scan checked/)).toBeInTheDocument();
    expect(screen.queryByText(/locked on this tier/i)).not.toBeInTheDocument();
  });

  it("renders uptime fields from the generated backend contract", () => {
    render(<UptimeReportPage data={reportData()} />);

    expect(screen.getByText("99.95%")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("245 ms")).toBeInTheDocument();
  });

  it("renders deployment totals and recent entries from the generated backend contract", () => {
    render(<DeploysReportPage data={reportData()} />);

    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("Ship Code Scan CLI")).toBeInTheDocument();
    expect(screen.getByText("Kyle")).toBeInTheDocument();
  });
});
