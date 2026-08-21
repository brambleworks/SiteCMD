import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { readPersistedShellPage, writePersistedShellPage } from "@/lib/app-shell-state";
import { reloadAppWindow } from "@/lib/app-reload";
import { persistProjectSelection, readStoredProjectSelection } from "@/lib/project-selection-state";
import { ErrorBoundary } from "./error-boundary";

vi.mock("@/lib/logger", () => ({
  logger: {
    error: vi.fn(),
  },
}));

vi.mock("@/lib/app-reload", () => ({
  reloadAppWindow: vi.fn(),
}));

function AlwaysThrows(): null {
  throw new Error("boom");
}

describe("ErrorBoundary", () => {
  const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

  beforeEach(() => {
    window.localStorage.clear();
    consoleErrorSpy.mockClear();
    vi.mocked(reloadAppWindow).mockClear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("can retry after a transient render crash", () => {
    let shouldThrow = true;

    function MaybeThrows() {
      if (shouldThrow) throw new Error("boom");
      return <div>Recovered shell</div>;
    }

    render(
      <ErrorBoundary>
        <MaybeThrows />
      </ErrorBoundary>,
    );

    expect(screen.getByText("Something went wrong")).toBeInTheDocument();

    shouldThrow = false;
    fireEvent.click(screen.getByRole("button", { name: "Try Again" }));

    expect(screen.getByText("Recovered shell")).toBeInTheDocument();
  });

  it("clears persisted shell state before reloading when the user confirms", () => {
    writePersistedShellPage("issues");
    persistProjectSelection(42, "https://example.com");
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    render(
      <ErrorBoundary>
        <AlwaysThrows />
      </ErrorBoundary>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset Saved State" }));

    expect(confirmSpy).toHaveBeenCalledTimes(1);
    expect(readPersistedShellPage()).toBeNull();
    expect(readStoredProjectSelection()).toBeNull();
    expect(reloadAppWindow).toHaveBeenCalledTimes(1);
    confirmSpy.mockRestore();
  });

  it("leaves persisted state alone when the user cancels the reset confirm", () => {
    writePersistedShellPage("issues");
    persistProjectSelection(42, "https://example.com");
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    render(
      <ErrorBoundary>
        <AlwaysThrows />
      </ErrorBoundary>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset Saved State" }));

    expect(confirmSpy).toHaveBeenCalledTimes(1);
    expect(readPersistedShellPage()).toBe("issues");
    expect(readStoredProjectSelection()).not.toBeNull();
    expect(reloadAppWindow).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});
