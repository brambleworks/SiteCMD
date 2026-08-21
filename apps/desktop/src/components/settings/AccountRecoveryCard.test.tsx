import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, toastSuccess, toastError } = vi.hoisted(() => ({
  invokeMock: vi.fn<(...args: unknown[]) => Promise<unknown>>(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ success: toastSuccess, error: toastError }),
}));

import { withQueryClient } from "@/test-utils/query-client";
import { AccountRecoveryCard } from "./AccountRecoveryCard";

const PENDING = {
  id: "rec_1",
  status: "pending",
  requestedBy: "inst_new_laptop",
  requestedAt: "2026-08-10T12:00:00.000Z",
  eligibleAt: "2026-08-24T12:00:00.000Z",
  exposureDemonstrated: false,
};

function renderCard() {
  return render(<AccountRecoveryCard />, { wrapper: withQueryClient() });
}

describe("AccountRecoveryCard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastSuccess.mockClear();
    toastError.mockClear();
  });

  it("renders the alarm and fires the acknowledgment automatically", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_account_recovery") return Promise.resolve({ recovery: PENDING });
      if (command === "acknowledge_account_recovery") {
        return Promise.resolve({ recovery: PENDING });
      }
      return Promise.resolve(null);
    });

    renderCard();
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByText(/inst_new_laptop requested admin recovery/)).toBeInTheDocument();
    expect(screen.getByText(/treat it as an attempted takeover/)).toBeInTheDocument();

    // Exposure is display: the render itself reports the banner was seen.
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("acknowledge_account_recovery", {}),
    );
  });

  it("cancels the pending recovery", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_account_recovery") return Promise.resolve({ recovery: PENDING });
      return Promise.resolve(null);
    });

    renderCard();
    fireEvent.click(await screen.findByRole("button", { name: "Cancel Recovery" }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("cancel_account_recovery", {}));
    expect(toastSuccess).toHaveBeenCalledWith("Recovery cancelled", expect.any(String));
  });

  it("offers the request path when nothing is pending, and never acks", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_account_recovery") return Promise.resolve({ recovery: null });
      if (command === "request_account_recovery") {
        return Promise.resolve({ ...PENDING, requestedBy: "this-machine" });
      }
      return Promise.resolve(null);
    });

    renderCard();
    fireEvent.click(await screen.findByRole("button", { name: "Request Admin Recovery" }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("request_account_recovery", {}));
    expect(toastSuccess).toHaveBeenCalledWith(
      "Admin recovery requested",
      expect.stringContaining("2026-08-24"),
    );
    // No pending banner was ever rendered, so nothing was acknowledged.
    expect(invokeMock).not.toHaveBeenCalledWith("acknowledge_account_recovery", {});
  });
});
