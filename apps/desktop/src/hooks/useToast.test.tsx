import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ToastProvider, useToast } from "./useToast";

function Trigger() {
  const toast = useToast();
  return (
    <button type="button" onClick={() => toast.success("Saved", "Your changes are live")}>
      Fire
    </button>
  );
}

describe("ToastProvider", () => {
  it("announces toasts through a polite live region", () => {
    render(
      <ToastProvider>
        <Trigger />
      </ToastProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Fire" }));

    const region = screen.getByRole("status");
    expect(region).toHaveAttribute("aria-live", "polite");
    expect(region).toHaveTextContent("Saved");
    expect(region).toHaveTextContent("Your changes are live");
  });

  it("names the dismiss control for screen readers", async () => {
    render(
      <ToastProvider>
        <Trigger />
      </ToastProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Fire" }));

    fireEvent.click(screen.getByRole("button", { name: "Dismiss notification" }));

    await waitFor(() => expect(screen.queryByText("Saved")).not.toBeInTheDocument());
  });
});
