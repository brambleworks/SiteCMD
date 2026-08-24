import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { CommandPalette } from "./CommandPalette";

beforeAll(() => {
  // jsdom lacks scrollIntoView.
  Element.prototype.scrollIntoView = vi.fn();
});

describe("CommandPalette navigation targets", () => {
  it("sends Reports to the real Reports page, not a settings tab", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<CommandPalette open onClose={vi.fn()} onNavigate={onNavigate} />);

    const input = screen.getByRole("textbox");
    await user.clear(input);
    await user.type(input, "Reports");
    await user.click(screen.getByText("Reports"));

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

  it("never emits settings targets for tabs the settings page no longer has", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<CommandPalette open onClose={vi.fn()} onNavigate={onNavigate} />);

    const input = screen.getByRole("textbox");
    for (const label of ["Overview", "Dashboard", "Issues", "Integrations", "Settings"]) {
      await user.clear(input);
      await user.type(input, label);
      await user.click(screen.getByText(label));
    }

    for (const call of onNavigate.mock.calls) {
      expect(String(call[0])).not.toMatch(/^settings:(reports|danger-zone|data-support)$/);
    }
  });
});
