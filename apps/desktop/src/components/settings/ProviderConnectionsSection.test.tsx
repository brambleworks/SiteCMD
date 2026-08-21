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
import { ProviderConnectionsSection } from "./ProviderConnectionsSection";

function renderSection() {
  return render(<ProviderConnectionsSection />, { wrapper: withQueryClient() });
}

const activeConnection = {
  id: "pc_1",
  provider: "vercel",
  status: "active",
  createdAt: "2026-08-01T00:00:00Z",
  activatedAt: "2026-08-01T00:01:00Z",
  externalAccount: { id: "acct_1", name: "Acme Team" },
  grantedScopes: "read-write projects and deploy hooks",
  failedReason: null,
  revokedAt: null,
  revokedReason: null,
};

describe("ProviderConnectionsSection", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastSuccess.mockClear();
    toastError.mockClear();
  });

  it("shows the requested scopes before anything opens in a browser", async () => {
    invokeMock.mockImplementation((command: unknown, args: unknown) => {
      if (command === "list_connected_provider_connections") return Promise.resolve([]);
      if (command === "create_connected_provider_connection") {
        expect(args).toEqual({ provider: "vercel" });
        return Promise.resolve({
          authorizeUrl: "https://vercel.com/oauth/authorize?state=st_1",
          connection: { ...activeConnection, status: "pending", externalAccount: null },
          requestedScopes: "read-write projects and deploy hooks",
        });
      }
      if (command === "open_external_url") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Connect Vercel" }));

    // Consent renders first; the browser opens only on the explicit click.
    expect(await screen.findByText("read-write projects and deploy hooks")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("open_external_url", expect.anything());

    fireEvent.click(screen.getByRole("button", { name: "Open Provider Sign-in" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("open_external_url", {
        url: "https://vercel.com/oauth/authorize?state=st_1",
      }),
    );
  });

  it("lists connections with status and account, terminal states visible", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_provider_connections") {
        return Promise.resolve([
          activeConnection,
          {
            ...activeConnection,
            id: "pc_dead",
            provider: "netlify",
            status: "failed",
            externalAccount: null,
            grantedScopes: null,
            failedReason: "authorize_expired",
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByText("Vercel - Acme Team")).toBeInTheDocument();
    expect(screen.getByText(/granted read-write projects/)).toBeInTheDocument();
    expect(screen.getByText(/failed: authorize_expired/)).toBeInTheDocument();
    // A failed round offers no revoke; there is nothing to revoke.
    expect(screen.getAllByRole("button", { name: "Revoke" })).toHaveLength(1);
  });

  it("revokes an active connection", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_provider_connections") {
        return Promise.resolve([activeConnection]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Revoke" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("revoke_connected_provider_connection", {
        connectionId: "pc_1",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith("Provider connection revoked");
  });
});
