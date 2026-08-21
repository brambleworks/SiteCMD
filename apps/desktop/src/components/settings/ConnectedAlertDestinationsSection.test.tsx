import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
import { ConnectedAlertDestinationsSection } from "./ConnectedAlertDestinationsSection";

function renderSection() {
  return render(<ConnectedAlertDestinationsSection />, { wrapper: withQueryClient() });
}

const confirmed = {
  destinationId: "dst_1",
  address: "alerts@example.com",
  verification: "verified",
  verifiedAt: "2026-08-02T00:00:00Z",
  suppressed: false,
  suppressionReason: null,
  immediateDisabled: false,
  digestDisabled: false,
  revision: 2,
  createdAt: "2026-08-01T00:00:00Z",
};

const unconfirmed = {
  ...confirmed,
  destinationId: "dst_new",
  address: "ops@example.com",
  verification: "unverified",
  verifiedAt: null,
  revision: 1,
};

describe("ConnectedAlertDestinationsSection", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastSuccess.mockClear();
    toastError.mockClear();
  });

  it("adds an address and says the mailbox has to answer before anything is sent", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") return Promise.resolve([]);
      if (command === "create_connected_destination") return Promise.resolve(unconfirmed);
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.change(await screen.findByLabelText("Email address"), {
      target: { value: "ops@example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add Address" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("create_connected_destination", {
        address: "ops@example.com",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith(
      "Confirmation email on its way",
      "Nothing reaches it until someone opens the link in that email.",
    );
  });

  it("distinguishes an unconfirmed address and states why it receives nothing", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") {
        return Promise.resolve([confirmed, unconfirmed]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByText("ops@example.com")).toBeInTheDocument();
    expect(screen.getByText(/Waiting for confirmation\./)).toBeInTheDocument();
    expect(
      screen.getByText(/Nothing is sent here until someone opens the link/),
    ).toBeInTheDocument();
    // The confirmed address gets no confirmation prompt and keeps its controls.
    expect(
      screen.getByText(/Confirmed\. Immediate alerts and the digest both/),
    ).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Send Confirmation Again" })).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Pause Alerts" })).toBeInTheDocument();
  });

  it("says a suppressed address stopped receiving and names confirming again as the way back", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") {
        return Promise.resolve([{ ...confirmed, suppressed: true, suppressionReason: "bounce" }]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByText(/Mail to this address bounced/)).toBeInTheDocument();
    expect(screen.getByText(/Confirming again is the way back/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send Confirmation Again" })).toBeInTheDocument();
  });

  it("resends the confirmation, and shows the rate limit as a wait rather than a failure", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") return Promise.resolve([unconfirmed]);
      if (command === "resend_connected_destination_verification") {
        return Promise.resolve({
          sent: false,
          refusal: "rate_limited",
          message: "A confirmation email went out recently. Wait a few minutes.",
        });
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Send Confirmation Again" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("resend_connected_destination_verification", {
        destinationId: "dst_new",
      }),
    );
    expect(await screen.findByText(/Wait a few minutes/)).toBeInTheDocument();
    expect(toastError).not.toHaveBeenCalled();
  });

  it("pauses immediate alerts under the revision it read", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      if (command === "update_connected_destination_policy") {
        return Promise.resolve({ applied: true, refusal: "", message: "", revision: 3 });
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Pause Alerts" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("update_connected_destination_policy", {
        destinationId: "dst_1",
        revision: 2,
        immediateDisabled: true,
        digestDisabled: false,
      }),
    );
  });

  it("turns a refused delete into the sites the person has to detach first", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      if (command === "delete_connected_destination") {
        return Promise.resolve({
          deleted: false,
          refusal: "destination_in_use",
          message: "Sites still send their alerts here.",
          sites: ["site_a", "site_b"],
        });
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));

    expect(await screen.findByText("Sites still send their alerts here.")).toBeInTheDocument();
    expect(screen.getByText(/site_a, site_b/)).toBeInTheDocument();
    expect(
      screen.getByText(/choose a different address, then remove this one/),
    ).toBeInTheDocument();
    // A refusal the person can act on is not an error toast.
    expect(toastError).not.toHaveBeenCalled();
  });

  it("degrades honestly when the account is not connected", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") {
        return Promise.reject(new Error("no installation token is stored for this machine"));
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByRole("alert")).toHaveTextContent("Alert addresses could not load.");
    // The add form stays, because adding is what a newly connected account does
    // first; it just cannot be submitted with nothing typed.
    expect(screen.getByRole("button", { name: "Add Address" })).toBeDisabled();
  });

  it("shows the identifier when the installation is not entitled to see addresses", async () => {
    // A non-admin installation reads the state of the destinations its own
    // sites use, without the account's address list.
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_destinations") {
        return Promise.resolve([{ ...confirmed, address: null, revision: 0 }]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByText("dst_1")).toBeInTheDocument();
    expect(screen.queryByText("alerts@example.com")).not.toBeInTheDocument();
  });
});
