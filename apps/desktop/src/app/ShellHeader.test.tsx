import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ShellPageHeader, ShellPageLoading } from "./ShellHeader";

describe("ShellPageHeader", () => {
  it("shows the page guide action in standard shell headers", () => {
    render(<ShellPageHeader page="dashboard" showScanHeader={false} />);

    expect(screen.getByRole("button", { name: "Open Site Dashboard Guide" })).toBeInTheDocument();
  });

  it("shows the issues title with the guide action", () => {
    render(<ShellPageHeader page="issues" showScanHeader={false} />);

    expect(screen.getByRole("button", { name: "Open Issues Guide" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Issues" })).toBeInTheDocument();
  });
});

describe("ShellPageLoading", () => {
  it.each([
    ["dashboard", "Loading Site Dashboard", ".skeleton-page-stats"],
    ["issues", "Loading Issues", ".skeleton-page-split"],
    ["events", "Loading Activity", ".skeleton-page-list"],
    ["settings", "Loading Project Settings", ".skeleton-page-card-grid"],
  ] as const)("uses the %s page's real layout geometry", (page, label, selector) => {
    const { container } = render(<ShellPageLoading page={page} />);

    expect(screen.getByRole("status", { name: label })).toHaveAttribute("aria-busy", "true");
    expect(container.querySelector(selector)).not.toBeNull();
  });
});
