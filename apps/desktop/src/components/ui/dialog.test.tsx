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
});
