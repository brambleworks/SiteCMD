import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { sendActionableDesktopNotification } from "./actionable-notifications";

describe("sendActionableDesktopNotification", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("returns true when the desktop notification command succeeds", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await expect(
      sendActionableDesktopNotification({
        title: "Heads up",
        body: "Scan complete",
      }),
    ).resolves.toBe(true);

    expect(invokeMock).toHaveBeenCalledWith("send_actionable_desktop_notification", {
      request: {
        title: "Heads up",
        body: "Scan complete",
      },
    });
  });

  it("returns false instead of throwing when notification delivery fails", async () => {
    invokeMock.mockRejectedValueOnce(new Error("notification unavailable"));

    await expect(
      sendActionableDesktopNotification({
        title: "Heads up",
        body: "Scan complete",
      }),
    ).resolves.toBe(false);
  });
});
