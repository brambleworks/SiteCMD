import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { addJobMock, removeRunningJobMock, updateTrayScanStatusMock } = vi.hoisted(() => ({
  addJobMock: vi.fn(),
  removeRunningJobMock: vi.fn(),
  updateTrayScanStatusMock: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/commands", () => ({
  updateTrayScanStatus: updateTrayScanStatusMock,
}));

vi.mock("@/lib/jobs", () => ({
  addJob: (...args: unknown[]) => addJobMock(...args),
  removeRunningJob: (...args: unknown[]) => removeRunningJobMock(...args),
}));

import { beginScanRun, publishScanProgress, resetScanProgress } from "@/lib/scan-progress-store";
import { useScanShellStatus } from "./useScanShellStatus";

function renderFullScanStatus() {
  return renderHook(() =>
    useScanShellStatus({
      activeEnvUrl: "https://example.com",
      activeProjectId: 1,
      activeScanScope: "Example",
      currentScanType: "full",
      scanRunStep: {
        mode: "full",
        stepIndex: 1,
        stepCount: 2,
        label: "Web Scan",
      },
      scanJobContextRef: {
        current: {
          projectId: 1,
          url: "https://example.com",
          scopeLabel: "Example",
        },
      },
      state: "scanning",
    }),
  );
}

describe("useScanShellStatus", () => {
  beforeEach(() => {
    addJobMock.mockClear();
    removeRunningJobMock.mockClear();
    updateTrayScanStatusMock.mockClear();
    resetScanProgress();
  });

  afterEach(() => {
    resetScanProgress();
    vi.useRealTimers();
  });

  it("latches a unified Full Scan to its Code step after Code progress begins", () => {
    beginScanRun({ web: "health", code: true });
    renderFullScanStatus();

    expect(addJobMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ detail: "Step 1 of 2: Web Scan", progress: 0 }),
    );

    act(() => {
      publishScanProgress({
        check_id: "code-scan.analyze-source",
        category: "config",
        status: "running",
        results_count: 3,
        checks_done: 42,
        checks_total: 100,
      });
    });

    // The code step owns the ring now, at the code scan's own 42%.
    expect(addJobMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        label: "Full scan",
        progress: 42,
        detail: "Step 2 of 2: Code Scan",
      }),
    );
    expect(updateTrayScanStatusMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ scanning: true, pct: 42 }),
    );

    act(() => publishScanProgress(null));
    expect(addJobMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ detail: "Step 2 of 2: Code Scan", progress: 42 }),
    );
  });

  it("keeps the job row and tray moving between events", () => {
    vi.useFakeTimers();
    beginScanRun({ web: "health", code: false });
    renderFullScanStatus();
    act(() => {
      publishScanProgress({
        check_id: "config.www_redirect",
        category: "config",
        status: "running",
        results_count: 0,
        checks_done: 90,
        checks_total: 127,
      });
    });
    const atEvent = addJobMock.mock.lastCall?.[0].progress as number;
    expect(atEvent).toBeGreaterThan(40);

    act(() => {
      vi.advanceTimersByTime(3_000);
    });
    const later = addJobMock.mock.lastCall?.[0].progress as number;
    expect(later).toBeGreaterThan(atEvent);
    expect(later).toBeLessThan(60);
  });
});
