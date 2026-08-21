import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { addJob, clearJobsByType, completeJob, type BackgroundJob } from "@/lib/jobs";
import { JobsTray } from "./JobsTray";

type JobSeed = Omit<BackgroundJob, "startedAt" | "status" | "endedAt">;

// JobsTray subscribes to the jobs store itself, so tests drive the store
// rather than passing job lists as props.
function seedJob(overrides: Partial<JobSeed> = {}) {
  act(() => {
    addJob({
      id: "scan-1",
      type: "scan",
      label: "Web scan",
      scopeLabel: "Example Site",
      detail: "81/100",
      progress: 81,
      target: {
        page: "issues",
        projectId: 1,
        url: "https://example.com",
        scanId: 12,
        scanKind: "site",
      },
      ...overrides,
    });
  });
}

function resetJobs() {
  clearJobsByType("scan");
  clearJobsByType("probes");
  clearJobsByType("sync");
}

describe("JobsTray", () => {
  beforeEach(resetJobs);
  afterEach(resetJobs);

  it("renders nothing when there are no running jobs", () => {
    const { container } = render(<JobsTray />);

    expect(container).toBeEmptyDOMElement();
  });

  it("does not render completed jobs", () => {
    seedJob();
    act(() => {
      completeJob("scan-1");
    });

    const { container } = render(<JobsTray />);

    expect(container).toBeEmptyDOMElement();
  });

  it("opens a running job when it has a concrete target", () => {
    const onOpenJob = vi.fn();
    seedJob();

    render(<JobsTray onOpenJob={onOpenJob} />);

    fireEvent.click(screen.getByText("Web scan"));
    expect(onOpenJob).toHaveBeenCalledTimes(1);
    expect(onOpenJob).toHaveBeenCalledWith(
      expect.objectContaining({ id: "scan-1", label: "Web scan", status: "running" }),
    );
  });

  it("renders below modal scan overlays", () => {
    seedJob();
    const { container } = render(<JobsTray />);

    expect(container.firstElementChild).toHaveClass("jobs-tray-stack");
  });

  it("keeps rows non-interactive when a job has no target", () => {
    seedJob({
      id: "sync-1",
      type: "sync",
      label: "Metadata sync",
      detail: "No destination",
      target: null,
      progress: undefined,
    });

    render(<JobsTray onOpenJob={vi.fn()} />);

    expect(screen.getByText("Metadata sync").closest("button")).toBeNull();
  });

  it("treats restoreScan jobs as clickable even without a page target", () => {
    const onOpenJob = vi.fn();
    seedJob({
      id: "scan-restore",
      label: "Restore scan overlay",
      detail: "Resume backgrounded scan",
      target: { restoreScan: true },
      progress: 42,
    });

    render(<JobsTray onOpenJob={onOpenJob} />);

    fireEvent.click(screen.getByText("Restore scan overlay"));
    expect(onOpenJob).toHaveBeenCalledWith(expect.objectContaining({ id: "scan-restore" }));
  });

  it("can be minimized and restored while jobs are running", () => {
    seedJob({ progress: 42 });

    render(<JobsTray />);

    fireEvent.click(screen.getByRole("button", { name: "Minimize jobs" }));

    expect(screen.queryByText("Web scan")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show running jobs" })).toHaveTextContent(
      "1job running",
    );

    fireEvent.click(screen.getByRole("button", { name: "Show running jobs" }));

    expect(screen.getByText("Web scan")).toBeInTheDocument();
  });

  it("repaints only the tray on scan-progress ticks, never the shell above it", () => {
    const shellRenderSpy = vi.fn();
    function Shell() {
      shellRenderSpy();
      return (
        <div>
          <JobsTray />
        </div>
      );
    }

    seedJob({ progress: 1, detail: "1 of 124 checks" });
    render(<Shell />);
    const rendersAfterMount = shellRenderSpy.mock.calls.length;

    for (let tick = 2; tick <= 11; tick++) {
      seedJob({ progress: tick, detail: `${tick} of 124 checks` });
    }

    expect(screen.getByText(/11 of 124 checks/)).toBeInTheDocument();
    expect(screen.getByText("11%")).toBeInTheDocument();
    expect(shellRenderSpy).toHaveBeenCalledTimes(rendersAfterMount);
  });
});
