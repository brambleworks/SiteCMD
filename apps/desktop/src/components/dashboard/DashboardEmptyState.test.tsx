import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DashboardEmptyState } from "./DashboardEmptyState";

const baseProps = {
  url: "https://acme.test",
  projectName: "Acme",
  framework: "Next.js" as string | null,
  projectPath: null as string | null,
  onOpenScanConfig: vi.fn(),
  onAddFolder: vi.fn(),
  onNavigate: vi.fn(),
};

describe("DashboardEmptyState", () => {
  it("leads with a single dominant scan CTA", () => {
    const onOpenScanConfig = vi.fn();
    render(<DashboardEmptyState {...baseProps} onOpenScanConfig={onOpenScanConfig} />);

    const scanCta = screen.getByRole("button", { name: /run your first scan/i });
    fireEvent.click(scanCta);
    expect(onOpenScanConfig).toHaveBeenCalledTimes(1);
  });

  // One scan covers whichever halves exist, so the prompt must never tell a
  // code-only project to scan a site it does not have.
  it("describes the scan by what this project actually has", () => {
    const { unmount } = render(<DashboardEmptyState {...baseProps} projectPath={null} />);
    expect(screen.getByText(/Checks your live site for security/i)).toBeInTheDocument();
    unmount();

    render(<DashboardEmptyState {...baseProps} url="" projectPath="/Users/dev/app" />);
    expect(screen.getByText(/Checks your linked code for database/i)).toBeInTheDocument();
    expect(screen.queryByText(/live site/i)).not.toBeInTheDocument();
  });

  it("prompts for a folder only when the project has none", () => {
    const { unmount } = render(<DashboardEmptyState {...baseProps} projectPath={null} />);
    expect(screen.getByRole("button", { name: /link your project folder/i })).toBeInTheDocument();
    unmount();

    render(<DashboardEmptyState {...baseProps} projectPath="/Users/dev/app" />);
    expect(
      screen.queryByRole("button", { name: /link your project folder/i }),
    ).not.toBeInTheDocument();
  });

  it("demotes integrations to one footnote link instead of a checklist of CTAs", () => {
    const onNavigate = vi.fn();
    render(<DashboardEmptyState {...baseProps} onNavigate={onNavigate} />);

    expect(screen.queryByText(/build your command center/i)).not.toBeInTheDocument();

    const integrationLinks = screen.getAllByRole("button", { name: /^integrations$/i });
    expect(integrationLinks).toHaveLength(1);

    fireEvent.click(integrationLinks[0]);
    expect(onNavigate).toHaveBeenCalledWith("integrations");
    expect(onNavigate).toHaveBeenCalledTimes(1);
  });

  it("teaches the loop by naming what each scan surfaces", () => {
    render(<DashboardEmptyState {...baseProps} />);
    expect(screen.getByText("What each scan checks")).toBeInTheDocument();
    expect(screen.getByText(/rolls into your Issues list/i)).toBeInTheDocument();
  });
});
