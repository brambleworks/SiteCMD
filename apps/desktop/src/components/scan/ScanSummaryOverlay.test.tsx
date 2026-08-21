import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ScanSummaryOverlay } from "./ScanSummaryOverlay";
import type { ScanSummaryModel } from "./scan-summary-model";

function buildSummary(): ScanSummaryModel {
  return {
    id: "web:1|code:abc",
    title: "Full scan complete",
    scopeLabel: "example.com",
    // The product has exactly one score: the unified SiteCMD Score.
    siteCmdScore: 80,
    totalIssues: 7,
    severityCounts: { critical: 1, high: 2, medium: 3, low: 1 },
    estimatedNewIssues: 2,
    resolvedIssues: 1,
    regressionCount: 0,
    note: "Skipped 2 nested repositories (repo-a, repo-b). Nested repositories and gitignored folders are not scanned as this project's code, so they add no findings here.",
  };
}

describe("ScanSummaryOverlay", () => {
  it("renders the summary headline, score, and severity totals", () => {
    render(
      <ScanSummaryOverlay summary={buildSummary()} onClose={vi.fn()} onReviewIssues={vi.fn()} />,
    );

    expect(screen.getByRole("dialog", { name: "Full scan complete" })).toBeInTheDocument();
    expect(screen.getByText("7 open issues after this scan.")).toBeInTheDocument();
    expect(screen.getByLabelText("Issue severity totals")).toBeInTheDocument();
    expect(screen.getByText(/Skipped 2 nested repositories/)).toBeInTheDocument();
  });

  it("shows an unavailable marker instead of calling an unknown delta a first scan", () => {
    render(
      <ScanSummaryOverlay
        summary={{ ...buildSummary(), estimatedNewIssues: null, resolvedIssues: null }}
        onClose={vi.fn()}
        onReviewIssues={vi.fn()}
      />,
    );

    const newLabel = screen.getByText("New");
    const newStat = newLabel.closest(".scan-summary-stat");
    expect(newStat).not.toBeNull();
    expect(within(newStat as HTMLElement).getByText("-")).toBeInTheDocument();
    expect(screen.queryByText("First")).toBeNull();
  });

  it("renders severity counts as color-coded text, not boxed pills", () => {
    render(
      <ScanSummaryOverlay summary={buildSummary()} onClose={vi.fn()} onReviewIssues={vi.fn()} />,
    );

    const severityRow = screen.getByLabelText("Issue severity totals");
    for (const label of ["Critical", "High", "Medium", "Low"]) {
      expect(within(severityRow).getByText(label)).toBeInTheDocument();
    }

    expect(severityRow.querySelector(".scan-summary-severity-pill")).toBeNull();
    const critical = within(severityRow)
      .getByText("Critical")
      .closest(".scan-summary-severity-item");
    expect(critical).toHaveClass("scan-summary-severity-critical");
  });

  it("headlines the single SiteCMD Score, with no secondary project-score line", () => {
    render(
      <ScanSummaryOverlay summary={buildSummary()} onClose={vi.fn()} onReviewIssues={vi.fn()} />,
    );

    expect(screen.getByText("SiteCMD Score")).toBeInTheDocument();
    expect(screen.queryByText("This scan")).toBeNull();
    expect(screen.queryByText(/Project score/)).toBeNull();
  });

  it("presents no per-source score decomposition (single-score thesis)", () => {
    const { container } = render(
      <ScanSummaryOverlay summary={buildSummary()} onClose={vi.fn()} onReviewIssues={vi.fn()} />,
    );

    // Negative control: no "Score by source" heading, no per-source score rows,
    // and no competing web/code score labels anywhere in the overlay.
    expect(screen.queryByText("Score by source")).toBeNull();
    expect(container.querySelector(".scan-summary-section-list")).toBeNull();
    expect(container.querySelector(".scan-summary-section-row")).toBeNull();
    expect(screen.queryByText("Web Scan")).toBeNull();
    expect(screen.queryByText("Code Scan")).toBeNull();
    expect(screen.queryByText("84/100")).toBeNull();
  });
});
