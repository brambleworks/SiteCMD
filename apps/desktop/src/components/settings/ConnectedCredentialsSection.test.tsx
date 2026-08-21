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
import { ConnectedCredentialsSection } from "./ConnectedCredentialsSection";

function renderSection() {
  return render(
    <ConnectedCredentialsSection projectId={7} environmentScopeKey="https://example.com" />,
    { wrapper: withQueryClient() },
  );
}

const ciRow = {
  id: "cit_1",
  kind: "ci",
  createdAt: "2026-08-01T00:00:00Z",
  createdBy: "inst_a",
  repository: "acme/site",
  lastUsedAt: "2026-08-09T12:00:00Z",
  revokedAt: null,
  secretFingerprint: null,
  secretGeneration: null,
  rotationOverlapUntil: null,
};

const webhookRow = {
  id: "swh_1",
  kind: "webhook",
  createdAt: "2026-08-02T00:00:00Z",
  createdBy: "inst_a",
  repository: null,
  lastUsedAt: null,
  revokedAt: null,
  secretFingerprint: "sha256:0123456789abcdef",
  secretGeneration: 2,
  rotationOverlapUntil: null,
};

describe("ConnectedCredentialsSection", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    copyMock.mockClear();
    toastSuccess.mockClear();
    toastError.mockClear();
  });

  it("lists both credential kinds and keeps revoked ones visible as tombstones", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_site_credentials") {
        return Promise.resolve([
          ciRow,
          { ...ciRow, id: "cit_dead", repository: null, revokedAt: "2026-08-05T00:00:00Z" },
          webhookRow,
        ]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(await screen.findByText("CI token for acme/site")).toBeInTheDocument();
    expect(screen.getByText(/cit_1; last used 2026-08-09/)).toBeInTheDocument();
    expect(screen.getByText(/cit_dead; revoked 2026-08-05/)).toBeInTheDocument();
    expect(screen.getByText("Deploy webhook secret, generation 2")).toBeInTheDocument();
    expect(screen.getByText(/sha256:0123456789abcdef/)).toBeInTheDocument();
    // A live webhook secret exists, so there is nothing to mint.
    expect(screen.queryByRole("button", { name: /Mint Webhook Secret/ })).not.toBeInTheDocument();
  });

  it("mints the webhook secret and shows it exactly once with the signing rule", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_site_credentials") return Promise.resolve([ciRow]);
      if (command === "mint_connected_webhook_secret") {
        return Promise.resolve({
          id: "swh_new",
          secret: "sitecmd_whs_shown_once",
          secretFingerprint: "sha256:feedface",
          secretGeneration: 1,
        });
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Mint Webhook Secret" }));

    expect(await screen.findByText("sitecmd_whs_shown_once")).toBeInTheDocument();
    expect(screen.getByText(/one time it is readable/i)).toBeInTheDocument();
    expect(screen.getByText(/X-SiteCMD-Signature/)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("mint_connected_webhook_secret", {
      projectId: 7,
      environmentScopeKey: "https://example.com",
    });

    fireEvent.click(screen.getByRole("button", { name: "Copy Secret" }));
    await waitFor(() => expect(copyMock).toHaveBeenCalledWith("sitecmd_whs_shown_once"));
  });

  it("offers a revoked webhook secret the mint-again path", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_site_credentials") {
        return Promise.resolve([{ ...webhookRow, revokedAt: "2026-08-08T00:00:00Z" }]);
      }
      return Promise.resolve(null);
    });

    renderSection();
    expect(
      await screen.findByRole("button", { name: "Mint Webhook Secret Again" }),
    ).toBeInTheDocument();
    // The tombstone offers no rotate: every value its holder saw is dead.
    expect(screen.queryByRole("button", { name: "Rotate Secret" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Revoke" })).not.toBeInTheDocument();
  });

  it("rotates the webhook secret and explains the dual-validity overlap", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_site_credentials") return Promise.resolve([webhookRow]);
      if (command === "rotate_connected_site_credential") {
        return Promise.resolve({
          id: "swh_1",
          secret: "sitecmd_whs_next_generation",
          secretFingerprint: "sha256:aaaabbbb",
          secretGeneration: 3,
          rotationOverlapUntil: "2026-08-11T00:00:00Z",
        });
      }
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Rotate Secret" }));

    expect(await screen.findByText("sitecmd_whs_next_generation")).toBeInTheDocument();
    expect(screen.getByText(/previous generation still opens the deploy hook/)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("rotate_connected_site_credential", {
      projectId: 7,
      environmentScopeKey: "https://example.com",
      tokenId: "swh_1",
    });
  });

  it("revokes a CI token by its public handle", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "list_connected_site_credentials") return Promise.resolve([ciRow]);
      return Promise.resolve(null);
    });

    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Revoke" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("revoke_connected_site_credential", {
        projectId: 7,
        environmentScopeKey: "https://example.com",
        tokenId: "cit_1",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith("Credential revoked");
  });
});
