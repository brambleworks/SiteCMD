import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { isTauriMock, openExternalUrlMock } = vi.hoisted(() => ({
  isTauriMock: vi.fn(),
  openExternalUrlMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: isTauriMock,
}));

vi.mock("@/lib/commands", () => ({
  openExternalUrl: openExternalUrlMock,
}));

import { openUrl } from "./open-url";

describe("openUrl", () => {
  const windowOpen = vi.fn();
  const consoleWarn = vi.fn();

  beforeEach(() => {
    isTauriMock.mockReset();
    isTauriMock.mockReturnValue(false);
    openExternalUrlMock.mockReset();
    windowOpen.mockReset();
    consoleWarn.mockReset();
    vi.spyOn(window, "open").mockImplementation(windowOpen);
    vi.spyOn(console, "warn").mockImplementation(consoleWarn);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("uses a noopener noreferrer fallback outside Tauri", async () => {
    await openUrl(" https://sitecmd.com/docs ");

    expect(windowOpen).toHaveBeenCalledWith(
      "https://sitecmd.com/docs",
      "_blank",
      "noopener,noreferrer",
    );
  });

  it.each([
    "https://sitecmd.com/docs",
    "https://www.bing.com/webmasters/",
    "http://localhost:4321/",
  ])("routes %s through the validated command without a browser fallback", async (url) => {
    isTauriMock.mockReturnValue(true);

    await openUrl(url);

    expect(openExternalUrlMock).toHaveBeenCalledExactlyOnceWith({ url });
    expect(windowOpen).not.toHaveBeenCalled();
  });

  it("does not bypass a failed native browser launch", async () => {
    isTauriMock.mockReturnValue(true);
    openExternalUrlMock.mockRejectedValueOnce(new Error("Browser unavailable"));

    await expect(openUrl("https://www.bing.com/webmasters/")).rejects.toThrow(
      "Browser unavailable",
    );

    expect(windowOpen).not.toHaveBeenCalled();
  });

  it("blocks non-http urls before reaching Tauri or the browser fallback", async () => {
    await openUrl("file:///Users/dev/private.txt");
    await openUrl("javascript:alert(1)");

    expect(openExternalUrlMock).not.toHaveBeenCalled();
    expect(windowOpen).not.toHaveBeenCalled();
  });

  it("blocks credential-bearing http urls before reaching Tauri or the browser fallback", async () => {
    await openUrl("https://user:token@example.com/private");

    expect(openExternalUrlMock).not.toHaveBeenCalled();
    expect(windowOpen).not.toHaveBeenCalled();
    expect(JSON.stringify(consoleWarn.mock.calls)).not.toContain("user:token");
  });
});
