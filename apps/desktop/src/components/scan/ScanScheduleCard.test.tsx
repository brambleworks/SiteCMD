import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, toastSuccessMock, toastErrorMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: invokeMock,
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: toastSuccessMock,
    error: toastErrorMock,
  }),
}));

import { ScanScheduleCard } from "./ScanScheduleCard";
import { withQueryClient } from "@/test-utils/query-client";

function renderSchedule(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

describe("ScanScheduleCard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastSuccessMock.mockReset();
    toastErrorMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_scan_schedule") return Promise.resolve(null);
      if (command === "save_scan_schedule") return Promise.resolve({ next_run_at: null });
      return Promise.resolve(null);
    });
  });

  it("schedules a single full scan with no web-vs-code choice, matching the runner", async () => {
    renderSchedule(<ScanScheduleCard projectId={7} environmentId={11} projectPath="/tmp/alpha" />);

    expect(await screen.findByText("Scheduled scans")).toBeInTheDocument();
    // Loads (and later saves) as the unified "full" scan, not a per-focus type.
    expect(invokeMock).toHaveBeenCalledWith("get_scan_schedule", {
      projectId: 7,
      environmentId: 11,
      scanType: "full",
    });

    const setupButton = screen.getByRole("button", { name: "Set Up Schedule" });
    await waitFor(() => expect(setupButton).toBeEnabled());
    fireEvent.click(setupButton);

    // The web-vs-code picker is gone entirely.
    expect(screen.queryByRole("button", { name: "Web Scan" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Code Scan/ })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Daily" }));
    fireEvent.click(screen.getByRole("button", { name: "Save Schedule" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("save_scan_schedule", {
        projectId: 7,
        environmentId: 11,
        frequency: "daily",
        timeOfDay: "09:00",
        dayOfWeek: null,
        scanType: "full",
      });
    });
    expect(toastSuccessMock).toHaveBeenCalledWith(
      "Schedule saved",
      "A full scan will run on this cadence.",
    );
  });

  it("notes a Code Scan is included when a project folder is linked", async () => {
    renderSchedule(<ScanScheduleCard projectId={7} environmentId={11} projectPath="/tmp/alpha" />);

    expect(await screen.findByText(/full scan: web checks plus a Code Scan/i)).toBeInTheDocument();
  });

  it("notes code is excluded until a folder is linked", async () => {
    renderSchedule(<ScanScheduleCard projectId={7} environmentId={11} projectPath={null} />);

    expect(
      await screen.findByText(/Link a project folder to include a Code Scan/i),
    ).toBeInTheDocument();
  });

  it("renders nothing without a project and environment", () => {
    const { container } = renderSchedule(<ScanScheduleCard />);
    expect(container).toBeEmptyDOMElement();
  });
});
