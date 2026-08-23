import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Dialog } from "@/components/ui/dialog";
import { IssueDossierPanel } from "./IssueDossierPanel";

describe("IssueDossierPanel", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it("closes when the user clicks outside the dossier", () => {
    const onClose = vi.fn();

    render(
      <div>
        <button type="button">Outside area</button>
        <IssueDossierPanel title="Missing canonical tag" onClose={onClose}>
          <div>Dossier content</div>
        </IssueDossierPanel>
      </div>,
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: "Outside area" }));

    act(() => {
      vi.advanceTimersByTime(180);
    });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders the fixed overlay through document.body instead of the page tree", () => {
    const onClose = vi.fn();

    render(
      <div data-testid="page-shell" style={{ transform: "translateZ(0)", overflow: "hidden" }}>
        <IssueDossierPanel title="Missing canonical tag" onClose={onClose}>
          <div>Dossier content</div>
        </IssueDossierPanel>
      </div>,
    );

    const panel = screen.getByRole("dialog", { name: "Missing canonical tag" });

    // The native <dialog> is portaled straight to document.body, so it (and its
    // native ::backdrop) always escapes a transformed or overflow-hidden ancestor.
    expect(panel.tagName).toBe("DIALOG");
    expect(panel.parentElement).toBe(document.body);
    expect(panel.closest("[data-testid='page-shell']")).toBeNull();
  });

  it("captures backdrop clicks instead of letting them reach the app shell", () => {
    const onClose = vi.fn();
    const onShellPointerDown = vi.fn();

    render(
      <div onPointerDown={onShellPointerDown}>
        <button type="button">Sidebar item</button>
        <IssueDossierPanel title="Missing canonical tag" onClose={onClose}>
          <div>Dossier content</div>
        </IssueDossierPanel>
      </div>,
    );

    const dialog = screen.getByRole("dialog", { name: "Missing canonical tag" });

    fireEvent.pointerDown(dialog);

    act(() => {
      vi.advanceTimersByTime(180);
    });

    expect(onShellPointerDown).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on a row click even when the row carries a stale switch marker", () => {
    // The one-click "switch dossier without closing" exemption was removed:
    // showModal() makes the rest of the page inert, so that UX is impossible
    // under a real modal dialog. A row click is now an ordinary outside click.
    const onClose = vi.fn();

    render(
      <div>
        <button type="button" data-dossier-switch="true">
          Another issue
        </button>
        <IssueDossierPanel title="Missing canonical tag" onClose={onClose}>
          <div>Dossier content</div>
        </IssueDossierPanel>
      </div>,
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: "Another issue" }));

    act(() => {
      vi.advanceTimersByTime(180);
    });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("ignores clicks on a different dialog stacked on top", () => {
    const onClose = vi.fn();
    const onCloseHandoff = vi.fn();

    render(
      <div>
        <IssueDossierPanel title="Missing canonical tag" onClose={onClose}>
          <div>Dossier content</div>
        </IssueDossierPanel>
        <Dialog label="Fix with your agent" onClose={onCloseHandoff}>
          <button type="button">Close</button>
        </Dialog>
      </div>,
    );

    const handoff = screen.getByRole("dialog", { name: "Fix with your agent" });
    fireEvent.pointerDown(handoff);
    fireEvent.pointerDown(screen.getByRole("button", { name: "Close" }));

    act(() => {
      vi.advanceTimersByTime(180);
    });

    expect(onClose).not.toHaveBeenCalled();
  });

  it("keeps the close timer alive across rerenders", () => {
    const firstOnClose = vi.fn();
    const secondOnClose = vi.fn();

    const { rerender } = render(
      <div>
        <button type="button">Outside area</button>
        <IssueDossierPanel title="Missing canonical tag" onClose={firstOnClose}>
          <div>Dossier content</div>
        </IssueDossierPanel>
      </div>,
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: "Outside area" }));

    rerender(
      <div>
        <button type="button">Outside area</button>
        <IssueDossierPanel title="Missing canonical tag" onClose={secondOnClose}>
          <div>Dossier content</div>
        </IssueDossierPanel>
      </div>,
    );

    act(() => {
      vi.advanceTimersByTime(180);
    });

    expect(firstOnClose).not.toHaveBeenCalled();
    expect(secondOnClose).toHaveBeenCalledTimes(1);
  });
});
