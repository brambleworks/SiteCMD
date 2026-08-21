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

import { publishScanProgress, resetScanProgress } from "@/lib/scan-progress-store";
import { useScanShellStatus } from "./useScanShellStatus";

describe("useScanShellStatus", () => {
  beforeEach(() => {
    addJobMock.mockClear();
    removeRunningJobMock.mockClear();
    updateTrayScanStatusMock.mockClear();
    resetScanProgress();
  });

  afterEach(() => {
    resetScanProgress();
  });

  it("latches a unified Full Scan to its Code step after Code progress begins", () => {
    renderHook(() =>
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

    expect(addJobMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ detail: "Step 1 of 2: Web Scan" }),
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

    expect(addJobMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        label: "Full scan",
        progress: undefined,
        detail: "Step 2 of 2: Code Scan",
      }),
    );

    act(() => publishScanProgress(null));
    expect(addJobMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ detail: "Step 2 of 2: Code Scan" }),
    );
  });
});
