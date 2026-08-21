import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";

vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import { setDesktopPrefs, updateDesktopPrefs, useDesktopPrefs } from "./desktop-prefs";

describe("desktop-prefs store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    // Reset to defaults so tests don't leak
    setDesktopPrefs({
      backgroundMonitoring: false,
      fileWatchSuggestions: false,
      desktopNotifications: true,
      refreshOnFocus: true,
      automaticUpdates: true,
    });
  });

  it("useDesktopPrefs returns the current prefs snapshot", () => {
    const { result } = renderHook(() => useDesktopPrefs());
    expect(result.current.prefs).toEqual({
      backgroundMonitoring: false,
      fileWatchSuggestions: false,
      desktopNotifications: true,
      refreshOnFocus: true,
      automaticUpdates: true,
    });
  });

  it("setDesktopPrefs replaces and publishes to subscribers", () => {
    const { result } = renderHook(() => useDesktopPrefs());
    act(() => {
      setDesktopPrefs({
        backgroundMonitoring: false,
        fileWatchSuggestions: false,
        desktopNotifications: false,
        refreshOnFocus: false,
        automaticUpdates: true,
      });
    });
    expect(result.current.prefs.backgroundMonitoring).toBe(false);
    expect(result.current.prefs.refreshOnFocus).toBe(false);
  });

  it("updateDesktopPrefs patches individual keys while preserving the rest", () => {
    const { result } = renderHook(() => useDesktopPrefs());
    act(() => {
      updateDesktopPrefs({ desktopNotifications: false });
    });
    expect(result.current.prefs.desktopNotifications).toBe(false);
    expect(result.current.prefs.backgroundMonitoring).toBe(false);
    expect(result.current.prefs.fileWatchSuggestions).toBe(false);
  });

  it("setDesktopPrefs persists to localStorage", () => {
    act(() => {
      setDesktopPrefs({
        backgroundMonitoring: false,
        fileWatchSuggestions: true,
        desktopNotifications: false,
        refreshOnFocus: true,
        automaticUpdates: true,
      });
    });
    const raw = window.localStorage.getItem("sitecmd-desktop-prefs");
    expect(raw).not.toBeNull();
    const parsed = raw ? JSON.parse(raw) : null;
    expect(parsed.backgroundMonitoring).toBe(false);
    expect(parsed.desktopNotifications).toBe(false);
  });
});
