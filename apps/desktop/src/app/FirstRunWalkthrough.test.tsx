import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { FirstRunWalkthrough } from "./FirstRunWalkthrough";

describe("FirstRunWalkthrough", () => {
  it("renders as a corner walkthrough instead of a blocking modal", () => {
    render(
      <FirstRunWalkthrough
        currentPage="issues"
        projectName="Example Site"
        onClose={vi.fn()}
        onNavigate={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("First run walkthrough")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByLabelText("First run walkthrough")).toHaveClass(
      "panel",
      "panel--flush",
      "walkthrough-panel",
    );
  });

  it("starts at step 1 on Issues, where the first scan drops the user", () => {
    const onNavigate = vi.fn();

    render(
      <FirstRunWalkthrough
        currentPage="issues"
        projectName="Example Site"
        onClose={vi.fn()}
        onNavigate={onNavigate}
      />,
    );

    expect(screen.getByText("Step 1 of 6")).toBeInTheDocument();
    expect(screen.getByText("Start with what the scan found")).toBeInTheDocument();
    // Already on Issues: opening the tour must not move the user anywhere.
    expect(onNavigate).not.toHaveBeenCalled();
  });

  it("pulls the app to Issues when opened on a different page", () => {
    const onNavigate = vi.fn();

    render(
      <FirstRunWalkthrough
        currentPage="dashboard"
        projectName="Example Site"
        onClose={vi.fn()}
        onNavigate={onNavigate}
      />,
    );

    expect(screen.getByText("Step 1 of 6")).toBeInTheDocument();
    expect(onNavigate).toHaveBeenCalledWith("issues");
  });

  it("navigates through Updates, Alerts, Integrations, and Dashboard as the user advances", () => {
    const onNavigate = vi.fn();

    render(
      <FirstRunWalkthrough
        currentPage="issues"
        projectName="Example Site"
        onClose={vi.fn()}
        onNavigate={onNavigate}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Next: Updates/i }));
    expect(onNavigate).toHaveBeenLastCalledWith("updates");

    fireEvent.click(screen.getByRole("button", { name: /Next: Alerts/i }));
    expect(onNavigate).toHaveBeenLastCalledWith("alerts");
    expect(screen.getByText("See what changed while you were away")).toBeInTheDocument();

    // The AI editor step lives on Integrations, where Agent tools are.
    fireEvent.click(screen.getByRole("button", { name: /Next: AI editor/i }));
    expect(onNavigate).toHaveBeenLastCalledWith("integrations");
    expect(screen.getByText("Connect your AI editor")).toBeInTheDocument();
    expect(screen.getByText(/Manual setup/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open agent tools" }));
    expect(onNavigate).toHaveBeenLastCalledWith("settings:integrations");

    fireEvent.click(screen.getByRole("button", { name: /Next: Integrations/i }));
    expect(onNavigate).toHaveBeenLastCalledWith("integrations");
    expect(screen.getByText("Connect your services")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Next: Dashboard/i }));
    expect(onNavigate).toHaveBeenLastCalledWith("dashboard");
    expect(screen.getByText("Your home base")).toBeInTheDocument();
  });

  it("finishes from the final Dashboard step", () => {
    const onClose = vi.fn();

    render(
      <FirstRunWalkthrough
        currentPage="issues"
        projectName="Example Site"
        onClose={onClose}
        onNavigate={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Open walkthrough step 6: Dashboard/i }));
    fireEvent.click(screen.getByRole("button", { name: "Finish" }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
