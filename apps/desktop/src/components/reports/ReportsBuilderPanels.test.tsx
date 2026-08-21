import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ReportsSnapshotPanel } from "./ReportsBuilderPanels";

const SECTIONS = {
  health: true,
  code: true,
  analytics: false,
  search: false,
  uptime: false,
  deploys: false,
  updates: false,
};

function renderPanel(snapshotBusy: boolean) {
  return render(
    <ReportsSnapshotPanel
      hasLinkedFolder
      reportSnapshot={null}
      sections={SECTIONS as never}
      snapshotBusy={snapshotBusy}
      onRefreshSnapshot={vi.fn()}
    />,
  );
}

describe("ReportsSnapshotPanel", () => {
  it("shows in-flight feedback while a refresh is running", () => {
    const { container } = renderPanel(true);

    const refresh = screen.getByRole("button", { name: /refresh/i });
    expect(refresh).toBeDisabled();
    expect(container.querySelector(".animate-spin")).toBeInTheDocument();
  });

  it("leaves the refresh button actionable when idle", () => {
    const { container } = renderPanel(false);

    expect(screen.getByRole("button", { name: /refresh/i })).toBeEnabled();
    expect(container.querySelector(".animate-spin")).not.toBeInTheDocument();
  });
});
