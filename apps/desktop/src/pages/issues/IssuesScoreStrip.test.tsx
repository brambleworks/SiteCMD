import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { IssuesScoreStrip } from "./IssuesScoreStrip";
import type { ScoreBreakdownDisplay } from "@/lib/score-breakdown";
import { MS_PER_MINUTE } from "@/lib/format";

afterEach(() => {
  vi.useRealTimers();
});

function breakdown(overrides: Partial<ScoreBreakdownDisplay> = {}): ScoreBreakdownDisplay {
  return {
    overall: 79,
    base: 100,
    deductions: [],
    hasDeductions: false,
    exploitableCapped: false,
    floorApplied: false,
    capNote: null,
    floorNote: null,
    ...overrides,
  };
}

describe("IssuesScoreStrip", () => {
  it("shows the unified SiteCMD score above the issues list", () => {
    const { container } = render(
      <IssuesScoreStrip
        score={{
          sitecmdScore: 79,
          totalIssues: 4,
          severityTotals: { critical: 0, high: 1, medium: 2, low: 1 },
          breakdown: breakdown({
            deductions: [
              { tier: "high", label: "High", points: 9 },
              { tier: "medium", label: "Medium", points: 6 },
            ],
            hasDeductions: true,
          }),
        }}
        issueSummary={{
          totalCount: 4,
          severityCounts: { critical: 1, high: 0, medium: 2, low: 1 },
        }}
        checkedAt="2026-04-20T17:08:00Z"
      />,
    );

    expect(
      screen.getByText(
        (content, element) => content === "SiteCMD Score" && element?.className === "card__title",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        (content, element) => content === "79" && element?.className === "score-ring-value",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("%")).not.toBeInTheDocument();
    expect(screen.getByText(/4 issues · 1 critical/)).toBeInTheDocument();
    expect(screen.queryByText(/leading risk/)).not.toBeInTheDocument();
    expect(screen.queryByText(/critical\/high/)).not.toBeInTheDocument();
    expect(container.querySelector(".score-strip-icon")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open SiteCMD Score Guide" })).toBeInTheDocument();
    const breakdownDetails = screen.getByText("How this score is computed").closest("details");
    expect(breakdownDetails).not.toBeNull();
    expect(breakdownDetails).not.toHaveAttribute("open");
    expect(screen.getByText("Starts at")).toBeInTheDocument();
    expect(screen.getByText("-9")).toBeInTheDocument();
    expect(screen.getByText("High issues")).toBeInTheDocument();
    expect(screen.getByText("-6")).toBeInTheDocument();
    expect(screen.getByText("Medium issues")).toBeInTheDocument();
  });

  it("uses the Issues page summary for counts instead of the score snapshot", () => {
    render(
      <IssuesScoreStrip
        score={{
          sitecmdScore: 61,
          totalIssues: 57,
          severityTotals: { critical: 14, high: 16, medium: 20, low: 7 },
          breakdown: breakdown({ overall: 61 }),
        }}
        issueSummary={{
          totalCount: 34,
          severityCounts: { critical: 2, high: 8, medium: 16, low: 8 },
        }}
        checkedAt="2026-04-20T17:08:00Z"
      />,
    );

    expect(screen.getByText(/34 issues · 2 critical/)).toBeInTheDocument();
    expect(screen.queryByText(/57 issues/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Security leading risk/)).not.toBeInTheDocument();
  });

  it("surfaces an honest note when the score is exploitable-capped (D6/D7)", () => {
    render(
      <IssuesScoreStrip
        score={{
          sitecmdScore: 30,
          totalIssues: 3,
          severityTotals: { critical: 1, high: 1, medium: 1, low: 0 },
          breakdown: breakdown({
            overall: 30,
            deductions: [{ tier: "critical", label: "Critical", points: 70 }],
            hasDeductions: true,
            exploitableCapped: true,
            capNote: "Score capped: a confirmed-exploitable critical issue was found.",
          }),
        }}
        checkedAt="2026-04-20T17:08:00Z"
      />,
    );

    expect(screen.getAllByText(/Score capped/).length).toBeGreaterThan(0);
  });

  it("keeps the score slot stable when no issues exist", () => {
    render(
      <IssuesScoreStrip
        score={{
          sitecmdScore: 100,
          totalIssues: 0,
          severityTotals: { critical: 0, high: 0, medium: 0, low: 0 },
          breakdown: breakdown({ overall: 100 }),
        }}
        checkedAt={null}
      />,
    );

    expect(
      screen.getByText(
        (content, element) => content === "100" && element?.className === "score-ring-value",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(/Not run yet/)).toBeInTheDocument();
    expect(screen.getByText("No point deductions")).toBeInTheDocument();
  });

  it("updates the checked time without a parent render", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-20T12:00:00Z"));

    render(<IssuesScoreStrip score={null} checkedAt="2026-08-20T11:31:00Z" />);

    expect(screen.getByText(/Checked 29m ago/)).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(2 * MS_PER_MINUTE);
    });

    expect(screen.getByText(/Checked 31m ago/)).toBeInTheDocument();
  });
});
