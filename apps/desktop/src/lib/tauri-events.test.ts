import { afterEach, describe, expect, it, vi } from "vitest";

// Override the global bridge stub from test/setup.ts so we control the unlisten
// returned by Tauri's listen. vi.mock is hoisted above setup.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

import { listen as tauriListen } from "@tauri-apps/api/event";

import { safeListen } from "./tauri-events";

describe("safeListen", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("returns an idempotent unlisten that tears down the listener only once", async () => {
    const rawUnlisten = vi.fn();
    vi.mocked(tauriListen).mockResolvedValue(rawUnlisten);

    const unlisten = await safeListen("evt", () => {});
    unlisten();
    unlisten();
    unlisten();

    expect(rawUnlisten).toHaveBeenCalledTimes(1);
  });

  it("swallows the already-removed error when cleanup double-fires (StrictMode/HMR)", async () => {
    const rawUnlisten = vi.fn(() => {
      // Mirrors Tauri's real failure once listeners[eventId] is already gone.
      throw new TypeError("undefined is not an object (evaluating 'listeners[eventId].handlerId')");
    });
    vi.mocked(tauriListen).mockResolvedValue(rawUnlisten);

    const unlisten = await safeListen("evt", () => {});

    expect(() => unlisten()).not.toThrow();
    // The throwing teardown still counts: a second call must not retry it.
    expect(() => unlisten()).not.toThrow();
    expect(rawUnlisten).toHaveBeenCalledTimes(1);
  });

  it("swallows an asynchronously-rejecting unlisten without an unhandled rejection", async () => {
    const rawUnlisten = vi.fn(() => Promise.reject(new Error("already removed")));
    vi.mocked(tauriListen).mockResolvedValue(rawUnlisten as unknown as () => void);

    const unlisten = await safeListen("evt", () => {});

    expect(() => unlisten()).not.toThrow();
    // Give the swallowed rejection a tick to settle (would surface as unhandled).
    await Promise.resolve();
    expect(rawUnlisten).toHaveBeenCalledTimes(1);
  });
});
