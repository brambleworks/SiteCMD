import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NavPage } from "@/components/layout/NavSidebar";
import { ACTION_SHORTCUTS, PAGE_SHORTCUTS } from "@/app/keyboard-shortcuts";
import { useAppKeyboardShortcuts } from "./useAppKeyboardShortcuts";

// The hook registers OS-level shortcuts through this plugin on mount; stub it so
// the effect resolves without a live Tauri backend.
vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn(() => Promise.resolve()),
  unregisterAll: vi.fn(() => Promise.resolve()),
}));

function setup(overrides: Partial<Parameters<typeof useAppKeyboardShortcuts>[0]> = {}) {
  const props = {
    activeEnvUrl: "https://acme.test" as string | null,
    enabledCategories: ["security"],
    navigateTo: vi.fn(),
    openAddProject: vi.fn(),
    openCommandPalette: vi.fn(),
    openScanConfig: vi.fn(),
    page: "dashboard" as NavPage,
    scan: vi.fn(),
    scanState: "idle",
    timeout: 30,
    ...overrides,
  };
  renderHook(() => useAppKeyboardShortcuts(props));
  return props;
}

function press(key: string, init: KeyboardEventInit = {}) {
  window.dispatchEvent(
    new KeyboardEvent("keydown", { key, metaKey: true, cancelable: true, ...init }),
  );
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("useAppKeyboardShortcuts", () => {
  it("pins the documented bindings the palette advertises", () => {
    // The command palette renders these same labels, so a drifting key here is a
    // lie in the palette. Keep the source of truth honest.
    expect(PAGE_SHORTCUTS.dashboard).toEqual({ key: "1", label: "⌘1" });
    expect(PAGE_SHORTCUTS.issues).toEqual({ key: "5", label: "⌘5" });
    expect(PAGE_SHORTCUTS.settings).toEqual({ key: ",", label: "⌘," });
    expect(ACTION_SHORTCUTS.commandPalette).toEqual({ key: "k", label: "⌘K" });
    expect(ACTION_SHORTCUTS.addProject).toEqual({ key: "n", label: "⌘N" });
    expect(ACTION_SHORTCUTS.runScan).toEqual({ key: "r", label: "⌘R" });
  });

  it("opens the command palette on the palette key", () => {
    const { openCommandPalette } = setup();
    press(ACTION_SHORTCUTS.commandPalette.key);
    expect(openCommandPalette).toHaveBeenCalledTimes(1);
  });

  it("navigates to every page its shortcut advertises", () => {
    const { navigateTo } = setup();
    for (const [navPage, binding] of Object.entries(PAGE_SHORTCUTS)) {
      if (binding) press(binding.key);
      expect(navigateTo).toHaveBeenCalledWith(navPage);
    }
  });

  it("adds a project on the add-project key", () => {
    const { openAddProject } = setup();
    press(ACTION_SHORTCUTS.addProject.key);
    expect(openAddProject).toHaveBeenCalledTimes(1);
  });

  it("runs a scan on the run-scan key when a scan can start", () => {
    const { scan } = setup();
    press(ACTION_SHORTCUTS.runScan.key);
    expect(scan).toHaveBeenCalledWith("https://acme.test", {
      enabledCategories: ["security"],
      timeoutSecs: 30,
    });
  });

  it("does not run a scan while one is already in flight", () => {
    const { scan } = setup({ scanState: "scanning" });
    press(ACTION_SHORTCUTS.runScan.key);
    expect(scan).not.toHaveBeenCalled();
  });

  it("only runs a scan from the dashboard or issues pages", () => {
    const { scan } = setup({ page: "settings" });
    press(ACTION_SHORTCUTS.runScan.key);
    expect(scan).not.toHaveBeenCalled();
  });

  it("ignores shortcuts without the Cmd/Ctrl modifier", () => {
    const { navigateTo, openCommandPalette } = setup();
    press(ACTION_SHORTCUTS.commandPalette.key, { metaKey: false });
    press(PAGE_SHORTCUTS.issues!.key, { metaKey: false });
    expect(openCommandPalette).not.toHaveBeenCalled();
    expect(navigateTo).not.toHaveBeenCalled();
  });

  it("ignores shortcuts while typing in a form field", () => {
    const { navigateTo } = setup();
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: PAGE_SHORTCUTS.issues!.key,
        metaKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    expect(navigateTo).not.toHaveBeenCalled();
  });
});
