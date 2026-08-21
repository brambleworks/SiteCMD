import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ScanFollowUpBannerHost } from "./ScanFollowUpBannerHost";

const banner = {
  id: 'regressed:{"page":"security"}',
  title: "A regression needs attention",
  description: "Resume 1 regressed item next.",
  actionLabel: "Verify Security",
  tone: "urgent" as const,
  target: {
    page: "issues" as const,
    projectId: 7,
    url: "https://example.com",
    reason: "changed-security-file",
  },
};

describe("ScanFollowUpBannerHost", () => {
  it("renders on the matching page and routes through the CTA", () => {
    const onOpenTarget = vi.fn();
    const onClearBanner = vi.fn();

    render(
      <ScanFollowUpBannerHost
        page="issues"
        scanState="idle"
        banner={banner}
        onOpenTarget={onOpenTarget}
        onClearBanner={onClearBanner}
      />,
    );

    expect(screen.getByText("A regression needs attention")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Verify Security" }));
    expect(onOpenTarget).toHaveBeenCalledWith(banner.target);
  });

  it("clears when the user dismisses the banner", () => {
    const onClearBanner = vi.fn();

    render(
      <ScanFollowUpBannerHost
        page="issues"
        scanState="idle"
        banner={banner}
        onOpenTarget={vi.fn()}
        onClearBanner={onClearBanner}
      />,
    );

    fireEvent.click(screen.getAllByRole("button", { name: "Dismiss" })[0]);
    expect(onClearBanner).toHaveBeenCalledTimes(1);
  });

  it("clears after the banner was visible and the user leaves that page", () => {
    const onClearBanner = vi.fn();

    const view = render(
      <ScanFollowUpBannerHost
        page="issues"
        scanState="idle"
        banner={banner}
        onOpenTarget={vi.fn()}
        onClearBanner={onClearBanner}
      />,
    );

    view.rerender(
      <ScanFollowUpBannerHost
        page="dashboard"
        scanState="idle"
        banner={banner}
        onOpenTarget={vi.fn()}
        onClearBanner={onClearBanner}
      />,
    );

    expect(onClearBanner).toHaveBeenCalledTimes(1);
  });

  it("clears when a new scan starts", () => {
    const onClearBanner = vi.fn();

    const view = render(
      <ScanFollowUpBannerHost
        page="issues"
        scanState="idle"
        banner={banner}
        onOpenTarget={vi.fn()}
        onClearBanner={onClearBanner}
      />,
    );

    view.rerender(
      <ScanFollowUpBannerHost
        page="issues"
        scanState="scanning"
        banner={banner}
        onOpenTarget={vi.fn()}
        onClearBanner={onClearBanner}
      />,
    );

    expect(onClearBanner).toHaveBeenCalledTimes(1);
  });
});
