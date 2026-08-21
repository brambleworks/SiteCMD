import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { CommandPalette } from "./CommandPalette";

beforeAll(() => {
  // jsdom lacks scrollIntoView.
  Element.prototype.scrollIntoView = vi.fn();
});

describe("CommandPalette navigation targets", () => {
  it("sends Reports to the real Reports page, not a settings tab", () => {
    const onNavigate = vi.fn();
    render(<CommandPalette open onClose={vi.fn()} onNavigate={onNavigate} />);

    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Reports" } });
    fireEvent.click(screen.getByText("Reports"));

    expect(onNavigate).toHaveBeenCalledWith("reports");
  });

  it("documents each command's keyboard shortcut in its row", () => {
    render(<CommandPalette open onClose={vi.fn()} onNavigate={vi.fn()} onAction={vi.fn()} />);

    expect(screen.getByText("⌘1")).toBeInTheDocument(); // Dashboard
    expect(screen.getByText("⌘5")).toBeInTheDocument(); // Issues
    expect(screen.getByText("⌘,")).toBeInTheDocument(); // Settings
    expect(screen.getByText("⌘R")).toBeInTheDocument(); // Run Scan
    expect(screen.getByText("⌘N")).toBeInTheDocument(); // Add Project
  });

  it("never emits settings targets for tabs the settings page no longer has", () => {
    const onNavigate = vi.fn();
    render(<CommandPalette open onClose={vi.fn()} onNavigate={onNavigate} />);

    const input = screen.getByRole("textbox");
    for (const label of ["Overview", "Dashboard", "Issues", "Integrations", "Settings"]) {
      fireEvent.change(input, { target: { value: label } });
      fireEvent.click(screen.getByText(label));
    }

    for (const call of onNavigate.mock.calls) {
      expect(String(call[0])).not.toMatch(/^settings:(reports|danger-zone|data-support)$/);
    }
  });
});
