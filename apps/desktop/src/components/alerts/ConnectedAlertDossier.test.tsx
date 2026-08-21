import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConnectedAlert, ConnectedDestination } from "@/generated/ipc-bindings-connected";
import { ConnectedAlertDossier } from "./ConnectedAlertDossier";

vi.mock("@/components/issues/IssueDossierPanel", async () => {
  const { buildDossierPanelMock } = await import("@/test-utils/dossier-panel-mock");
  return buildDossierPanelMock();
});

const alert: ConnectedAlert = {
  alertId: "alr_0123456789abcdef01234567",
  causes: [
    { count: 2, kind: "regression", severity: "critical" },
    { count: 1, kind: "protection_degradation", severity: null },
  ],
  contentMode: "private",
  delivery: [
    { outcome: "sent", targetId: "dst_1", targetKind: "destination" },
    { outcome: "failed", targetId: "awh_1", targetKind: "webhook" },
  ],
  deploymentId: "dep_9",
  raisedAt: "2026-08-10T12:00:00.000Z",
  sequence: 12,
  severity: "critical",
  updatedAt: "2026-08-10T12:30:00.000Z",
};

function destination(overrides: Partial<ConnectedDestination> = {}): ConnectedDestination {
  return {
    address: "ops@example.com",
    createdAt: null,
    destinationId: "dst_1",
    digestDisabled: false,
    immediateDisabled: false,
    revision: 1,
    suppressed: false,
    suppressionReason: null,
    verification: "verified",
    verifiedAt: null,
    ...overrides,
  };
}

describe("ConnectedAlertDossier", () => {
  it("shows every cause class, including the ones that bear no severity", () => {
    render(<ConnectedAlertDossier alert={alert} destinations={[]} onClose={vi.fn()} />);

    expect(screen.getByText("Regression of a verified fix")).toBeInTheDocument();
    expect(screen.getByText("Protection degraded")).toBeInTheDocument();
    expect(screen.getByText("No severity")).toBeInTheDocument();
    expect(screen.getByText("2 findings")).toBeInTheDocument();
  });

  it("names the recipient when this installation may read the address", () => {
    render(
      <ConnectedAlertDossier alert={alert} destinations={[destination()]} onClose={vi.fn()} />,
    );

    expect(screen.getByText("Email: ops@example.com")).toBeInTheDocument();
    expect(screen.getByText("Delivered")).toBeInTheDocument();
    expect(screen.getByText("Failed")).toBeInTheDocument();
  });

  it("falls back to the channel rather than inventing an address it cannot read", () => {
    // A non-admin installation reads destination health without addresses.
    render(
      <ConnectedAlertDossier
        alert={alert}
        destinations={[destination({ address: null })]}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Email")).toBeInTheDocument();
    expect(screen.queryByText(/dst_1/)).not.toBeInTheDocument();
  });

  it("says an alert reached nobody instead of showing an empty section", () => {
    render(
      <ConnectedAlertDossier
        alert={{ ...alert, delivery: [] }}
        destinations={[]}
        onClose={vi.fn()}
      />,
    );

    expect(
      screen.getByText(/the service recorded the alert and sent it nowhere/),
    ).toBeInTheDocument();
  });

  it("offers no link to the service's hosted page", () => {
    // That page is reached by redeeming a one-time nonce that travels only in
    // the email, so any link built here would authenticate with nothing.
    const { container } = render(
      <ConnectedAlertDossier alert={alert} destinations={[]} onClose={vi.fn()} />,
    );

    expect(container.querySelector("a[href]")).toBeNull();
    expect(screen.queryByText(/View on the web|Open hosted/i)).not.toBeInTheDocument();
  });
});
