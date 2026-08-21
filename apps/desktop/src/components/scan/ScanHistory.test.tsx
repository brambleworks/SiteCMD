import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ScanExecutionSummary } from "@/lib/types";

const hasFeature = vi.fn(() => true);

vi.mock("@/hooks/useTier", () => ({ useTier: () => ({ hasFeature }) }));

import { ScanHistory } from "./ScanHistory";

function scanExecution(overrides: Partial<ScanExecutionSummary> = {}): ScanExecutionSummary {
  return {
    id: 1,
    projectId: 8,
    environmentId: 10,
    environmentUrl: "https://example.com",
    requestedMode: "full",
    webFocus: "health",
    trigger: "manual",
    status: "complete",
    startedAt: Date.parse("2026-04-20T11:00:00Z"),
    completedAt: Date.parse("2026-04-20T11:00:02Z"),
    score: 92,
    criticalCount: 1,
    highCount: 2,
    mediumCount: 3,
    lowCount: 4,
    webStatus: "complete",
    webDetail: null,
    codeStatus: "complete",
    codeDetail: null,
    webScanId: 41,
    webSessionId: null,
    webPageCount: 1,
    codeScanId: 42,
    runs: [],
    ...overrides,
  };
}

function renderHistory(props: Partial<Parameters<typeof ScanHistory>[0]> = {}) {
  return render(<ScanHistory executions={[scanExecution()]} {...props} />);
}

describe("ScanHistory table", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    hasFeature.mockReturnValue(true);
  });

  it("shows one source-neutral row with score, total, and every severity count", () => {
    renderHistory();

    for (const heading of [
      "Date",
      "SiteCMD Score",
      "Total Issues",
      "Critical",
      "High",
      "Medium",
      "Low",
    ]) {
      expect(screen.getByRole("columnheader", { name: heading })).toBeInTheDocument();
    }

    const cells = within(screen.getByTestId("scan-history-row-1"))
      .getAllByRole("cell")
      .map((cell) => cell.textContent);
    expect(cells.slice(1)).toEqual(["92", "10", "1", "2", "3", "4"]);
    expect(screen.queryByText("Web")).not.toBeInTheDocument();
    expect(screen.queryByText("Code")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /compare/i })).not.toBeInTheDocument();
  });

  it("prefers completed Full Scan executions when they are available", () => {
    renderHistory({
      executions: [
        scanExecution({ id: 3, requestedMode: "code", score: 63 }),
        scanExecution({ id: 2, requestedMode: "web", score: 71 }),
        scanExecution({ id: 1, requestedMode: "full", score: 92 }),
      ],
    });

    expect(screen.getByTestId("scan-history-row-1")).toBeInTheDocument();
    expect(screen.queryByTestId("scan-history-row-2")).not.toBeInTheDocument();
    expect(screen.queryByTestId("scan-history-row-3")).not.toBeInTheDocument();
  });

  it("falls back to scored executions until a completed Full Scan exists", () => {
    renderHistory({
      executions: [
        scanExecution({ id: 2, requestedMode: "code", score: 63 }),
        scanExecution({ id: 1, requestedMode: "web", score: 71 }),
      ],
    });

    expect(screen.getByTestId("scan-history-row-1")).toBeInTheDocument();
    expect(screen.getByTestId("scan-history-row-2")).toBeInTheDocument();
  });

  it("omits unscored executions from the metrics table", () => {
    renderHistory({
      executions: [
        scanExecution({ id: 2, score: null, status: "failed" }),
        scanExecution({ id: 1, score: 88 }),
      ],
    });

    expect(screen.getByTestId("scan-history-row-1")).toBeInTheDocument();
    expect(screen.queryByTestId("scan-history-row-2")).not.toBeInTheDocument();
  });

  it("shows every stored scan to every tier", () => {
    const executions = Array.from({ length: 12 }, (_, index) =>
      scanExecution({ id: index + 1, startedAt: Date.now() - index }),
    );
    renderHistory({ executions });

    expect(screen.getAllByTestId(/^scan-history-row-/)).toHaveLength(12);
    expect(screen.queryByText(/older scans hidden/i)).not.toBeInTheDocument();
  });

  it("offers one generic first-scan action when no scored execution exists", () => {
    const onOpenScanConfig = vi.fn();
    renderHistory({ executions: [], onOpenScanConfig });

    expect(screen.getByText("No scans yet")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Run First Scan" }));
    expect(onOpenScanConfig).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: /code scan/i })).not.toBeInTheDocument();
  });
});
