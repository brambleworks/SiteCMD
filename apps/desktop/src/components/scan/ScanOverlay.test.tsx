import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScanOverlay } from "./ScanOverlay";

const CODE_SCAN_STAGE_LABELS = [
  "Project Files",
  "Source Code",
  "Dependencies",
  "Release Setup",
  "Saving Results",
  "Summary",
];

function visiblePercent() {
  return Number.parseInt(screen.getByTestId("scan-progress-percent").textContent ?? "0", 10);
}

function stageState(label: string) {
  const stage = Array.from(
    screen.getByTestId("scan-stages").querySelectorAll("[data-stage-state]"),
  ).find((node) => node.textContent === label);
  return stage?.getAttribute("data-stage-state");
}

describe("ScanOverlay", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders code scans as a full phased scan instead of a generic spinner", () => {
    render(<ScanOverlay progress={null} scanType="code" url="https://example.com" />);

    expect(screen.getByText("Code Scan")).toBeInTheDocument();
    expect(screen.getByText("Scanning linked project code")).toBeInTheDocument();
    expect(screen.getByText("Checking")).toBeInTheDocument();
    expect(
      screen.getByText("Finding source files and project config that belong in the audit."),
    ).toBeInTheDocument();
    expect(screen.getByText("5% complete · Project Files")).toBeInTheDocument();

    for (const label of CODE_SCAN_STAGE_LABELS) {
      expect(screen.getAllByText(label).length).toBeGreaterThan(0);
    }
  });

  it("keeps the code scan stages in the backend progress order", () => {
    render(<ScanOverlay progress={null} scanType="code" url="https://example.com" />);

    const stageLabels = Array.from(
      screen.getByTestId("scan-stages").querySelectorAll("[data-stage-label]"),
    ).map((node) => node.textContent);

    expect(stageLabels).toEqual(CODE_SCAN_STAGE_LABELS);
  });

  it("uses backend code scan progress instead of timer-based fake progress", () => {
    render(
      <ScanOverlay
        progress={{
          check_id: "code-scan.analyze-source",
          category: "config",
          status: "running",
          results_count: 12,
          checks_done: 44,
          checks_total: 100,
        }}
        scanType="code"
        url="https://example.com"
      />,
    );

    expect(screen.getByText("44")).toBeInTheDocument();
    expect(screen.getByText("44% complete · Source Code")).toBeInTheDocument();
    expect(screen.getByText("12 issues")).toBeInTheDocument();
  });

  it("smooths sudden backend progress jumps after the overlay has started", () => {
    vi.useFakeTimers();
    const { rerender } = render(
      <ScanOverlay progress={null} scanType="code" url="https://example.com" />,
    );

    expect(visiblePercent()).toBe(5);

    rerender(
      <ScanOverlay
        progress={{
          check_id: "code-scan.analyze-source",
          category: "config",
          status: "running",
          results_count: 3,
          checks_done: 80,
          checks_total: 100,
        }}
        scanType="code"
        url="https://example.com"
      />,
    );

    expect(visiblePercent()).toBeLessThan(80);

    act(() => {
      vi.advanceTimersByTime(500);
    });
    expect(visiblePercent()).toBeGreaterThan(5);
    expect(visiblePercent()).toBeLessThan(80);

    act(() => {
      vi.advanceTimersByTime(3_000);
    });
    expect(visiblePercent()).toBe(80);
  });

  it("moves code scan to the final visible phase when completion arrives", () => {
    vi.useFakeTimers();
    const { rerender } = render(
      <ScanOverlay progress={null} scanType="code" url="https://example.com" />,
    );

    expect(stageState("Project Files")).toBe("active");

    rerender(
      <ScanOverlay
        progress={{
          check_id: "code-scan.complete",
          category: "config",
          status: "complete",
          results_count: 4,
          checks_done: 100,
          checks_total: 100,
        }}
        scanType="code"
        url="https://example.com"
      />,
    );

    expect(stageState("Summary")).toBe("active");
  });

  it("shows web scan stages in scan-pipeline order", () => {
    render(
      <ScanOverlay
        progress={{
          check_id: "fetch",
          category: "security",
          status: "running",
          results_count: 0,
          checks_done: 0,
          checks_total: 10,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    const stageLabels = Array.from(
      screen.getByTestId("scan-stages").querySelectorAll("[data-stage-label]"),
    ).map((node) => node.textContent);

    expect(stageLabels).toEqual([
      "Fetch",
      "Security",
      "SEO",
      "Performance",
      "Accessibility",
      "Legal",
      "Polish",
      "Browser",
    ]);
  });

  it("keeps multi-page progress monotonic without replaying the category stage grid", () => {
    vi.useFakeTimers();
    const { rerender } = render(
      <ScanOverlay
        progress={{
          check_id: "browser-analysis",
          category: "performance",
          status: "complete",
          results_count: 2,
          checks_done: 10,
          checks_total: 10,
        }}
        multiProgress={{
          page_index: 0,
          page_count: 2,
          current_url: "https://example.com",
          page_status: "complete",
          session_id: 9,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    const firstPagePercent = visiblePercent();
    expect(firstPagePercent).toBe(50);
    expect(screen.queryByTestId("scan-stages")).not.toBeInTheDocument();

    rerender(
      <ScanOverlay
        progress={{
          check_id: "fetch",
          category: "security",
          status: "running",
          results_count: 0,
          checks_done: 0,
          checks_total: 10,
        }}
        multiProgress={{
          page_index: 1,
          page_count: 2,
          current_url: "https://example.com/about",
          page_status: "scanning",
          session_id: 9,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(visiblePercent()).toBe(firstPagePercent);
    expect(screen.queryByTestId("scan-stages")).not.toBeInTheDocument();
  });

  it("sits above persistent shell surfaces while active", () => {
    const { container } = render(
      <ScanOverlay progress={null} scanType="code" url="https://example.com" />,
    );

    expect(container.firstElementChild).toHaveClass("overlay-backdrop--scan");
  });

  it("keeps the whole overlay reachable when the window is short", () => {
    const { container } = render(
      <ScanOverlay progress={null} scanType="health" url="https://example.com" />,
    );

    expect(container.firstElementChild!.firstElementChild).toHaveClass("scan-overlay-content");
  });

  it("wires the background and cancel controls without making the backdrop actionable", () => {
    const onMinimize = vi.fn();
    const onCancel = vi.fn();
    const { container } = render(
      <ScanOverlay
        progress={null}
        scanType="health"
        url="https://example.com"
        onMinimize={onMinimize}
        onCancel={onCancel}
      />,
    );

    fireEvent.click(container.firstElementChild!);
    expect(onMinimize).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /continue in background/i }));
    expect(onMinimize).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /cancel scan/i }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("shows the overall full-scan step when a scan run includes web and code", () => {
    render(
      <ScanOverlay
        progress={null}
        scanType="code"
        url="https://example.com"
        scanRunStep={{
          mode: "full",
          stepIndex: 2,
          stepCount: 2,
          label: "Code Scan",
        }}
      />,
    );

    expect(screen.getByText("Scanning linked project code")).toBeInTheDocument();
    expect(screen.getByTestId("scan-run-context")).toHaveTextContent(
      /^Full Scan · Step 2 of 2 · Code Scan$/,
    );
  });

  it("starts code progress from the beginning when a full scan moves from web to code", () => {
    vi.useFakeTimers();
    const { rerender } = render(
      <ScanOverlay
        progress={{
          check_id: "browser-analysis",
          category: "performance",
          status: "running",
          results_count: 0,
          checks_done: 8,
          checks_total: 8,
        }}
        scanType="full"
        url="https://example.com"
        scanRunStep={{
          mode: "full",
          stepIndex: 1,
          stepCount: 2,
          label: "Web Scan",
        }}
      />,
    );

    act(() => {
      vi.advanceTimersByTime(30_000);
    });

    rerender(
      <ScanOverlay
        progress={{
          check_id: "code-scan.collect-files",
          category: "config",
          status: "running",
          results_count: 0,
          checks_done: 5,
          checks_total: 100,
        }}
        scanType="full"
        url="https://example.com"
        scanRunStep={{
          mode: "full",
          stepIndex: 1,
          stepCount: 2,
          label: "Web Scan",
        }}
      />,
    );

    expect(screen.getByText("Scanning linked project code")).toBeInTheDocument();
    expect(screen.getByTestId("scan-run-context")).toHaveTextContent(
      /^Full Scan · Step 2 of 2 · Code Scan$/,
    );

    rerender(
      <ScanOverlay
        progress={null}
        scanType="full"
        url="https://example.com"
        scanRunStep={{
          mode: "full",
          stepIndex: 1,
          stepCount: 2,
          label: "Web Scan",
        }}
      />,
    );

    expect(screen.getByText("Scanning linked project code")).toBeInTheDocument();
    expect(screen.queryByText("Scanning example.com")).not.toBeInTheDocument();
  });

  it("keeps web scan header copy stable while showing full-scan state in the meta line", () => {
    render(
      <ScanOverlay
        progress={{
          check_id: "headers",
          category: "security",
          status: "running",
          results_count: 0,
          checks_done: 1,
          checks_total: 10,
        }}
        scanType="health"
        url="https://example.com"
        scanRunStep={{
          mode: "full",
          stepIndex: 1,
          stepCount: 2,
          label: "Web Scan",
        }}
      />,
    );

    expect(screen.getByText("Scanning example.com")).toBeInTheDocument();
    expect(screen.getByText("Web Scan")).toBeInTheDocument();
    expect(screen.getByTestId("scan-run-context")).toHaveTextContent(
      /^Full Scan · Step 1 of 2 · Web Scan$/,
    );
  });

  it("shows web scan phase progress instead of resetting to zero for uncounted phases", () => {
    render(
      <ScanOverlay
        progress={{
          check_id: "polish-css",
          category: "polish",
          status: "running",
          results_count: 0,
          checks_done: 0,
          checks_total: 0,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(screen.getAllByText("Polish").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Fetching styles").length).toBeGreaterThan(0);
    expect(screen.getByText("90")).toBeInTheDocument();
  });

  it("paces visible web scan phase changes instead of skipping straight to the latest event", () => {
    vi.useFakeTimers();
    const { rerender } = render(
      <ScanOverlay
        progress={{
          check_id: "fetch",
          category: "security",
          status: "running",
          results_count: 0,
          checks_done: 0,
          checks_total: 10,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(stageState("Fetch")).toBe("active");

    rerender(
      <ScanOverlay
        progress={{
          check_id: "polish-css",
          category: "polish",
          status: "running",
          results_count: 0,
          checks_done: 0,
          checks_total: 0,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(stageState("Fetch")).toBe("active");

    act(() => {
      vi.advanceTimersByTime(700);
    });
    expect(stageState("Security")).toBe("active");

    for (let i = 0; i < 5; i += 1) {
      act(() => {
        vi.advanceTimersByTime(700);
      });
    }
    expect(stageState("Polish")).toBe("active");
  });

  it("shows actual web scan progress events in the activity feed", () => {
    render(
      <ScanOverlay
        progress={{
          check_id: "security.ssl",
          category: "security",
          status: "complete",
          results_count: 1,
          checks_done: 2,
          checks_total: 10,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(screen.getByText("Live Scan Events")).toBeInTheDocument();
    expect(screen.queryByText("cmd")).not.toBeInTheDocument();
    expect(screen.getByTestId("scan-terminal")).toHaveClass("scan-terminal");
    expect(screen.getByText("Done")).toBeInTheDocument();
    expect(screen.getAllByText(/Security/).length).toBeGreaterThan(0);
    expect(screen.getByText(/Security SSL/)).toBeInTheDocument();
    expect(screen.getByText("1 issue")).toBeInTheDocument();
  });

  it("keeps the current web scan phase in the status block between checks instead of flashing the preparing state", () => {
    const securityDetail = "Checking HTTPS, headers, redirects, cookies, and exposed files.";
    const { rerender } = render(
      <ScanOverlay
        progress={{
          check_id: "headers",
          category: "security",
          status: "running",
          results_count: 0,
          checks_done: 1,
          checks_total: 10,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(screen.getByText(securityDetail)).toBeInTheDocument();
    expect(screen.queryByText(/Preparing/)).not.toBeInTheDocument();

    rerender(
      <ScanOverlay
        progress={{
          check_id: "headers",
          category: "security",
          status: "complete",
          results_count: 0,
          checks_done: 2,
          checks_total: 10,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(screen.getByText(securityDetail)).toBeInTheDocument();
    expect(screen.queryByText(/Preparing/)).not.toBeInTheDocument();
  });

  it("still shows the preparing state before the first web scan progress event", () => {
    render(<ScanOverlay progress={null} scanType="health" url="https://example.com" />);

    expect(screen.getByText("Preparing")).toBeInTheDocument();
    expect(screen.getByText("scan")).toBeInTheDocument();
  });

  it("keeps the final web scan phase below fake-complete while running", () => {
    vi.useFakeTimers();
    const { rerender } = render(
      <ScanOverlay
        progress={{
          check_id: "browser-analysis",
          category: "performance",
          status: "running",
          results_count: 0,
          checks_done: 0,
          checks_total: 0,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    expect(visiblePercent()).toBe(93);

    rerender(
      <ScanOverlay
        progress={{
          check_id: "browser-analysis",
          category: "performance",
          status: "complete",
          results_count: 0,
          checks_done: 0,
          checks_total: 0,
        }}
        scanType="health"
        url="https://example.com"
      />,
    );

    act(() => {
      vi.advanceTimersByTime(150);
    });
    expect(visiblePercent()).toBeGreaterThan(93);
  });
});
