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
import { ConnectedNotificationSettingsSection } from "./ConnectedNotificationSettingsSection";

function renderSection() {
  return render(
    <ConnectedNotificationSettingsSection
      projectId={7}
      environmentScopeKey="https://example.com"
    />,
    { wrapper: withQueryClient() },
  );
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

const unconfigured = {
  allQuietHeartbeat: false,
  destinationId: null,
  mute: false,
  severityFloor: null,
  digestCadence: "weekly",
  contentMode: "private",
  thresholdCount: 0,
  revision: 4,
};

describe("ConnectedNotificationSettingsSection", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastSuccess.mockClear();
    toastError.mockClear();
  });

  it("points the site at a confirmed address and persists the choice", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") return Promise.resolve(unconfigured);
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      if (command === "put_connected_notification_settings") {
        return Promise.resolve({ applied: true, refusal: "", message: "", revision: 5 });
      }
      return Promise.resolve(null);
    });

    renderSection();
    // Wait for remote settings before editing.
    const save = await screen.findByRole("button", { name: "Save Alert Settings" });
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.change(screen.getByLabelText("Send this site's alerts to"), {
      target: { value: "dst_1" },
    });
    fireEvent.click(save);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("put_connected_notification_settings", {
        projectId: 7,
        environmentScopeKey: "https://example.com",
        allQuietHeartbeat: false,
        revision: 4,
        destinationId: "dst_1",
        mute: false,
        severityFloor: null,
        digestCadence: "weekly",
        contentMode: "private",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith("Alert settings saved", expect.any(String));
  });

  it("starts from the settings the service holds, unconfigured included", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") {
        return Promise.resolve({
          ...unconfigured,
          severityFloor: "high",
          digestCadence: "off",
          contentMode: "minimal",
          thresholdCount: 2,
        });
      }
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      return Promise.resolve(null);
    });

    renderSection();
    const destination = await screen.findByLabelText("Send this site's alerts to");
    await waitFor(() => expect(screen.getByLabelText("Email me about")).toHaveValue("high"));
    expect(destination).toHaveValue("");
    expect(screen.getByLabelText("Digest")).toHaveValue("off");
    expect(screen.getByLabelText("What the email may contain")).toHaveValue("minimal");
    expect(screen.getByLabelText(/Send an all-quiet heartbeat/)).not.toBeChecked();
    // Thresholds have no editor here, so the save has to say it keeps them.
    expect(screen.getByText(/2 measurement thresholds are set on this site/)).toBeInTheDocument();
  });

  it("edits the optional all-quiet heartbeat", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") return Promise.resolve(unconfigured);
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      if (command === "put_connected_notification_settings") {
        return Promise.resolve({ applied: true, refusal: "", message: "", revision: 5 });
      }
      return Promise.resolve(null);
    });

    renderSection();
    const heartbeat = await screen.findByLabelText(/Send an all-quiet heartbeat/);
    fireEvent.click(heartbeat);
    const save = screen.getByRole("button", { name: "Save Alert Settings" });
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.click(save);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "put_connected_notification_settings",
        expect.objectContaining({ allQuietHeartbeat: true }),
      ),
    );
  });

  it("describes the email content modes the same way the service renders them", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") return Promise.resolve(unconfigured);
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      return Promise.resolve(null);
    });

    renderSection();
    await screen.findByLabelText("What the email may contain");
    expect(
      screen.getByRole("option", {
        name: "Private: site alias, severity, cause, and counts; never routes, evidence, or code",
      }),
    ).toHaveValue("private");
    expect(
      screen.getByRole("option", {
        name: "Minimal: only that an alert exists and a link, with no site metadata",
      }),
    ).toHaveValue("minimal");
  });

  it("marks an unconfirmed address as one that would page nobody", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") {
        return Promise.resolve({ ...unconfigured, destinationId: "dst_new" });
      }
      if (command === "list_connected_destinations") {
        return Promise.resolve([
          {
            ...confirmed,
            destinationId: "dst_new",
            address: "ops@example.com",
            verification: "unverified",
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(
      await screen.findByText("ops@example.com (waiting for confirmation, receives nothing yet)"),
    ).toBeInTheDocument();
    expect(await screen.findByText(/this site would page nobody/)).toBeInTheDocument();
  });

  it("shows a lost revision race as a re-read rather than retrying over it", async () => {
    let currentRevision = 4;
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") {
        return Promise.resolve({ ...unconfigured, revision: currentRevision });
      }
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      if (command === "put_connected_notification_settings") {
        currentRevision = 5;
        return Promise.resolve({
          applied: false,
          refusal: "stale_revision",
          message: "These settings changed somewhere else while you were deciding.",
          revision: 5,
        });
      }
      return Promise.resolve(null);
    });

    renderSection();
    const save = await screen.findByRole("button", { name: "Save Alert Settings" });
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.click(save);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "These settings changed somewhere else while you were deciding.",
    );
    expect(toastSuccess).not.toHaveBeenCalled();

    // The refusal's revision is adopted, so the next save presents what the
    // service says is current instead of the stale value again.
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.click(save);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "put_connected_notification_settings",
        expect.objectContaining({ revision: 5 }),
      ),
    );
  });

  it("keeps an edit when the settings write fails outside the concurrency contract", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") return Promise.resolve(unconfigured);
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      if (command === "put_connected_notification_settings") {
        return Promise.reject(new Error("the connected service could not be reached"));
      }
      return Promise.resolve(null);
    });

    renderSection();
    const save = await screen.findByRole("button", { name: "Save Alert Settings" });
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.change(screen.getByLabelText("Send this site's alerts to"), {
      target: { value: "dst_1" },
    });
    fireEvent.click(save);

    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith(
        "Could not save the alert settings",
        expect.stringContaining("could not be reached"),
      ),
    );
    expect(screen.getByLabelText("Send this site's alerts to")).toHaveValue("dst_1");
  });

  it("never lets the address list's failure read as an alert setting", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") {
        return Promise.resolve({ ...unconfigured, destinationId: "dst_1" });
      }
      if (command === "list_connected_destinations") {
        return Promise.reject(new Error("the connected service could not be reached"));
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The account's alert addresses could not load",
    );
    const destination = screen.getByLabelText("Send this site's alerts to");
    expect(destination).toHaveValue("dst_1");
    expect(destination).toBeDisabled();
    expect(
      screen.getByRole("option", { name: "dst_1 (address list could not be read)" }),
    ).toBeInTheDocument();
    // No save may carry a destination the list cannot account for.
    expect(screen.getByRole("button", { name: "Save Alert Settings" })).toBeDisabled();
    expect(invokeMock).not.toHaveBeenCalledWith(
      "put_connected_notification_settings",
      expect.anything(),
    );
  });

  it("keeps a deleted address visible as itself rather than as no alerts at all", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") {
        return Promise.resolve({ ...unconfigured, destinationId: "dst_gone" });
      }
      if (command === "list_connected_destinations") return Promise.resolve([confirmed]);
      return Promise.resolve(null);
    });

    renderSection();
    const destination = await screen.findByLabelText("Send this site's alerts to");
    await waitFor(() => expect(destination).toHaveValue("dst_gone"));
    expect(
      screen.getByRole("option", { name: "dst_gone (no longer on this account's address list)" }),
    ).toBeInTheDocument();
    // A known list is still editable, so this one is a statement, not a lock.
    expect(destination).toBeEnabled();
  });

  it("degrades honestly when the site is not connected", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "get_connected_notification_settings") {
        return Promise.reject(new Error("this environment is not connected"));
      }
      if (command === "list_connected_destinations") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByRole("alert")).toHaveTextContent("Alert settings could not load.");
    expect(screen.getByRole("button", { name: "Save Alert Settings" })).toBeDisabled();
  });
});
