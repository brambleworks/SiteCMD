import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, copyMock, toastSuccess, toastError } = vi.hoisted(() => ({
  invokeMock: vi.fn<(...args: unknown[]) => Promise<unknown>>(),
  copyMock: vi.fn<(...args: unknown[]) => Promise<void>>(() => Promise.resolve()),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/lib/clipboard", () => ({ copyToClipboard: copyMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ success: toastSuccess, error: toastError }),
}));

import { withQueryClient } from "@/test-utils/query-client";
import { ConnectedReportsSection } from "./ConnectedReportsSection";

function renderSection() {
  return render(
    <ConnectedReportsSection projectId={7} environmentScopeKey="https://example.com" />,
    { wrapper: withQueryClient() },
  );
}

const liveRow = {
  reportId: "rep_live",
  createdAt: "2026-08-01T00:00:00Z",
  createdBy: "inst_a",
  includeRoutes: true,
  expiresAt: "2026-08-31T00:00:00Z",
  revoked: false,
  viewCount: 3,
};

describe("ConnectedReportsSection", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    copyMock.mockClear();
    toastSuccess.mockClear();
    toastError.mockClear();
  });

  it("creates a link with the chosen toggles and shows it exactly once", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_reports") return Promise.resolve([]);
      if (command === "create_connected_report") {
        return Promise.resolve({
          reportId: "rep_new",
          link: "https://connect.sitecmd.com/r/rlk_capability",
          expiresAt: "2026-08-17T00:00:00Z",
          includeRoutes: true,
        });
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByLabelText("Include route-level detail"));
    fireEvent.change(screen.getByLabelText("Link expires after"), { target: { value: "7" } });
    fireEvent.click(screen.getByRole("button", { name: "Create Report Link" }));

    expect(
      await screen.findByText("https://connect.sitecmd.com/r/rlk_capability"),
    ).toBeInTheDocument();
    expect(screen.getByText(/never shows it again/i)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("create_connected_report", {
      projectId: 7,
      environmentScopeKey: "https://example.com",
      includeRoutes: true,
      ttlDays: 7,
    });

    fireEvent.click(screen.getByRole("button", { name: "Copy Link" }));
    await waitFor(() =>
      expect(copyMock).toHaveBeenCalledWith("https://connect.sitecmd.com/r/rlk_capability"),
    );
  });

  it("lists the registry with provenance and view counts, links themselves absent", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_reports") {
        return Promise.resolve([
          liveRow,
          { ...liveRow, reportId: "rep_gone", revoked: true, viewCount: 1, includeRoutes: false },
        ]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByText("3 views")).toBeInTheDocument();
    expect(screen.getByText(/with route detail/)).toBeInTheDocument();
    expect(screen.getByText(/revoked/)).toBeInTheDocument();
    // A revoked link keeps its history but offers no second revocation.
    expect(screen.getAllByRole("button", { name: "Revoke" })).toHaveLength(1);
    expect(invokeMock).toHaveBeenCalledWith("list_connected_reports", {
      projectId: 7,
      environmentScopeKey: "https://example.com",
    });
  });

  it("revokes a live link immediately", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_reports") return Promise.resolve([liveRow]);
      if (command === "revoke_connected_report") {
        return Promise.resolve({ reportId: "rep_live", revokedAt: "2026-08-10T00:00:00Z" });
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Revoke" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("revoke_connected_report", {
        projectId: 7,
        environmentScopeKey: "https://example.com",
        reportId: "rep_live",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith("Report link revoked", expect.any(String));
  });
});
