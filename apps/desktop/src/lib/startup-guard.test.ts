import { fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { readPersistedShellPage, writePersistedShellPage } from "@/lib/app-shell-state";
import { reloadAppWindow } from "@/lib/app-reload";
import { persistProjectSelection, readStoredProjectSelection } from "@/lib/project-selection-state";
import {
  markStartupStage,
  readStartupStage,
  renderStartupFallback,
  resetPersistedWorkspaceState,
  supportsRequiredWebviewFeatures,
} from "./startup-guard";

vi.mock("@/lib/app-reload", () => ({
  reloadAppWindow: vi.fn(),
}));

describe("startup-guard", () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="root"></div>';
    document.documentElement.removeAttribute("data-sitecmd-startup");
    window.localStorage.clear();
    vi.mocked(reloadAppWindow).mockClear();
  });

  it("tracks the startup stage on the document element", () => {
    markStartupStage("booting");
    expect(readStartupStage()).toBe("booting");

    markStartupStage("mounted");
    expect(readStartupStage()).toBe("mounted");
  });

  it("renders a startup fallback with recovery actions", () => {
    renderStartupFallback({
      title: "SiteCMD could not start",
      description: "boot failed",
      details: "boom",
    });

    expect(document.getElementById("root")?.textContent).toContain("SiteCMD could not start");
    expect(document.getElementById("root")?.textContent).toContain("Reset Saved State");
    expect(readStartupStage()).toBe("failed");
  });

  it("can omit the irrelevant reset action for compatibility failures", () => {
    renderStartupFallback({
      title: "SiteCMD needs a newer system webview",
      description: "Update the system webview.",
      showResetAction: false,
    });

    expect(document.getElementById("root")?.textContent).toContain("Reload App");
    expect(document.getElementById("root")?.textContent).not.toContain("Reset Saved State");
  });

  it("detects the CSS feature required by the desktop theme", () => {
    vi.stubGlobal("CSS", { supports: vi.fn(() => true) });
    expect(supportsRequiredWebviewFeatures()).toBe(true);

    vi.stubGlobal("CSS", { supports: vi.fn(() => false) });
    expect(supportsRequiredWebviewFeatures()).toBe(false);
    vi.unstubAllGlobals();
  });

  it("resets persisted workspace state before reload", () => {
    writePersistedShellPage("security");
    persistProjectSelection(22, "https://example.com");

    renderStartupFallback({
      title: "SiteCMD could not start",
      description: "boot failed",
    });

    const resetButton = Array.from(document.querySelectorAll("button")).find(
      (button) => button.textContent === "Reset Saved State",
    );
    expect(resetButton).toBeInstanceOf(HTMLButtonElement);
    fireEvent.click(resetButton!);

    expect(readPersistedShellPage()).toBeNull();
    expect(readStoredProjectSelection()).toBeNull();
    expect(reloadAppWindow).toHaveBeenCalledTimes(1);
  });

  it("builds the fallback DOM with class names only (no inline style attributes)", () => {
    renderStartupFallback({
      title: "SiteCMD could not start",
      description: "boot failed",
      details: "boom",
    });

    const allElements = document.querySelectorAll(
      "#root .sitecmd-startup-fallback, #root .sitecmd-startup-fallback *",
    );
    expect(allElements.length).toBeGreaterThan(0);
    for (const element of allElements) {
      expect(
        element.getAttribute("style"),
        `${element.tagName.toLowerCase()} must not carry an inline style attribute`,
      ).toBeNull();
    }
  });

  it("can clear persisted workspace state directly", () => {
    writePersistedShellPage("security");
    persistProjectSelection(22, "https://example.com");

    resetPersistedWorkspaceState();

    expect(readPersistedShellPage()).toBeNull();
    expect(readStoredProjectSelection()).toBeNull();
  });
});
