import { useState } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Dialog } from "./dialog";

describe("Dialog", () => {
  it("opens as a modal with an accessible name", () => {
    render(
      <Dialog label="Scan configuration" onClose={vi.fn()}>
        <p>Body</p>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog", { name: "Scan configuration" });
    expect(dialog.tagName).toBe("DIALOG");
    expect(dialog).toHaveAttribute("open");
  });

  it("associates describedBy with the dialog", () => {
    render(
      <Dialog label="Scan configuration" describedBy="scan-config-desc" onClose={vi.fn()}>
        <p id="scan-config-desc">Runs a full scan of the site.</p>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog", { name: "Scan configuration" });
    expect(dialog).toHaveAttribute("aria-describedby", "scan-config-desc");
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Dialog label="Escape me" onClose={onClose}>
        <button type="button">Inside</button>
      </Dialog>,
    );

    await user.keyboard("{Escape}");

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("keeps Escape away from dialogs that must finish", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Dialog label="Scan in progress" onClose={onClose} closeOnEscape={false}>
        <button type="button">Cancel scan</button>
      </Dialog>,
    );

    await user.keyboard("{Escape}");

    expect(onClose).not.toHaveBeenCalled();
  });

  it("prevents the native cancel default even when closeOnEscape is false", () => {
    const onClose = vi.fn();
    render(
      <Dialog label="Scan in progress" onClose={onClose} closeOnEscape={false}>
        <button type="button">Cancel scan</button>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog", { name: "Scan in progress" });
    const cancelEvent = new Event("cancel", { cancelable: true });
    dialog.dispatchEvent(cancelEvent);

    // The browser's own close request (Escape, Android back) must never be
    // allowed to bypass React state, even for a dialog that ignores Escape.
    expect(cancelEvent.defaultPrevented).toBe(true);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("keeps a nested Escape from closing the dialog underneath", async () => {
    const user = userEvent.setup();
    const onCloseOuter = vi.fn();
    const onCloseInner = vi.fn();

    function Nested() {
      const [showInner, setShowInner] = useState(false);
      return (
        <Dialog label="Outer" onClose={onCloseOuter}>
          <button type="button" onClick={() => setShowInner(true)}>
            Open inner
          </button>
          {showInner ? (
            <Dialog label="Inner" onClose={onCloseInner}>
              <button type="button">Inner button</button>
            </Dialog>
          ) : null}
        </Dialog>
      );
    }

    render(<Nested />);
    // React re-dispatches portal events through the React tree, so the outer
    // Dialog is a logical ancestor of the inner one even though both portal
    // to document.body as DOM siblings.
    await user.click(screen.getByRole("button", { name: "Open inner" }));

    await user.keyboard("{Escape}");

    expect(onCloseInner).toHaveBeenCalledTimes(1);
    expect(onCloseOuter).not.toHaveBeenCalled();
  });

  it("closes on a backdrop click unless the caller opts out", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const { unmount } = render(
      <Dialog label="Dismissable" onClose={onClose}>
        <p>Body</p>
      </Dialog>,
    );
    await user.click(screen.getByRole("dialog", { name: "Dismissable" }));
    expect(onClose).toHaveBeenCalledTimes(1);
    unmount();

    const onCloseHandoff = vi.fn();
    render(
      <Dialog label="Handoff" onClose={onCloseHandoff} dismissOnBackdrop={false}>
        <p>Body</p>
      </Dialog>,
    );
    await user.click(screen.getByRole("dialog", { name: "Handoff" }));
    expect(onCloseHandoff).not.toHaveBeenCalled();
  });

  it("returns focus to the opener when it unmounts", async () => {
    const opener = document.createElement("button");
    opener.textContent = "Open";
    document.body.append(opener);
    opener.focus();

    const { unmount } = render(
      <Dialog label="Focus" onClose={vi.fn()}>
        <button type="button">Inside</button>
      </Dialog>,
    );
    unmount();

    await waitFor(() => expect(opener).toHaveFocus());
    opener.remove();
  });

  it("restores focus to restoreFocusTo even when activeElement was body at open", async () => {
    // WKWebView and WebKitGTK do not focus a button on click, so a mouse-opened
    // dialog can find document.activeElement still at body; restoreFocusTo is
    // how a caller with its own trigger ref recovers from that.
    const trigger = document.createElement("button");
    trigger.textContent = "Open guide";
    document.body.append(trigger);
    (document.activeElement as HTMLElement | null)?.blur();
    expect(document.activeElement).toBe(document.body);

    const { unmount } = render(
      <Dialog label="Focus" onClose={vi.fn()} restoreFocusTo={{ current: trigger }}>
        <button type="button">Inside</button>
      </Dialog>,
    );
    unmount();

    await waitFor(() => expect(trigger).toHaveFocus());
    trigger.remove();
  });
});
