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

import { KeyRotationCard } from "./KeyRotationCard";

const onChanged = vi.fn(() => Promise.resolve());

function renderCard(pendingKeyVersion: number | null = null) {
  return render(
    <KeyRotationCard
      projectId={7}
      environmentScopeKey="https://example.com"
      fingerprintKeyVersion={1}
      pendingKeyVersion={pendingKeyVersion}
      onChanged={onChanged}
    />,
  );
}

describe("KeyRotationCard", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    toastSuccess.mockClear();
    toastError.mockClear();
    onChanged.mockClear();
  });

  it("claims a rotation and explains how it completes", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "rotate_connected_fingerprint_key") {
        return Promise.resolve({
          status: "claimed",
          version: 2,
          expiresAt: "2026-08-13T12:00:00Z",
          claimedBy: null,
        });
      }
      return Promise.resolve(null);
    });

    renderCard();
    fireEvent.click(screen.getByRole("button", { name: "Rotate Key" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("rotate_connected_fingerprint_key", {
        projectId: 7,
        environmentScopeKey: "https://example.com",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith(
      "Rotation to version 2 claimed",
      expect.stringContaining("Sync Now to complete it"),
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("names a claim held elsewhere and offers the machine-lost abort", async () => {
    invokeMock.mockImplementation((command: unknown) => {
      if (command === "rotate_connected_fingerprint_key") {
        return Promise.resolve({
          status: "already_pending",
          version: 3,
          expiresAt: "2026-08-13T12:00:00Z",
          claimedBy: "inst_other",
        });
      }
      return Promise.resolve(null);
    });

    renderCard();
    fireEvent.click(screen.getByRole("button", { name: "Rotate Key" }));

    expect(
      await screen.findByText(/version 3 is already pending on installation inst_other/),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abort That Claim" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("abort_connected_key_rotation", {
        projectId: 7,
        environmentScopeKey: "https://example.com",
      }),
    );
    expect(toastSuccess).toHaveBeenCalledWith(
      "Key rotation aborted",
      "The claimed version number stays burned.",
    );
  });

  it("shows the pending state with completion instructions and an abort", async () => {
    renderCard(2);
    expect(
      screen.getByText("Rotation to version 2 is pending on this desktop."),
    ).toBeInTheDocument();
    expect(screen.getByText(/version 1 stays in force/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abort Rotation" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("abort_connected_key_rotation", {
        projectId: 7,
        environmentScopeKey: "https://example.com",
      }),
    );
  });
});
