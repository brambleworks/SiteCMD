import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { NAV_PAGES, type NavPage } from "./nav-page";
import { PageGuideButton } from "./PageGuide";

const PAGE_TITLES: Record<NavPage, string> = {
  dashboard: "Site Dashboard",
  analytics: "Traffic & Uptime",
  issues: "Issues",
  alerts: "Alerts",
  deploys: "Deployments",
  events: "Activity",
  "search-console": "Search & SEO",
  updates: "Updates",
  settings: "Project Settings",
  reports: "Reports",
  integrations: "Integrations",
  sites: "Overview",
};

async function openGuide(page: NavPage) {
  const user = userEvent.setup();
  const title = PAGE_TITLES[page];
  render(<PageGuideButton page={page} />);

  await user.click(screen.getByRole("button", { name: `Open ${title} guide` }));

  return screen.getByRole("dialog", { name: title });
}

describe("PageGuideButton", () => {
  it.each(NAV_PAGES)("opens current guidance for %s", async (page) => {
    const dialog = await openGuide(page);

    expect(dialog).toBeInTheDocument();
    expect(within(dialog).queryByText("Page Guide")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("Operator tip")).not.toBeInTheDocument();
    expect(within(dialog).queryByText(/^0[1-9]$/)).not.toBeInTheDocument();
  });

  it("uses the current issue lifecycle and agent workflow", async () => {
    const dialog = await openGuide("issues");

    expect(within(dialog).getByText(/Fix with Agent or Batch prompt/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Ignore a finding only/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Reopen an ignored or blocked finding/)).toBeInTheDocument();
    expect(within(dialog).queryByText(/Dismiss only/)).not.toBeInTheDocument();
  });

  it("keeps technical SEO findings on Issues", async () => {
    const dialog = await openGuide("search-console");

    expect(within(dialog).getByText(/Google Search Visibility shows clicks/)).toBeInTheDocument();
    expect(
      within(dialog).getByText(/Web Scan findings for metadata.*remain in Issues/),
    ).toBeInTheDocument();
  });

  it("documents agent connections as part of Integrations", async () => {
    const dialog = await openGuide("integrations");

    expect(within(dialog).getByRole("heading", { name: "Connect an agent" })).toBeInTheDocument();
    expect(within(dialog).getByText(/Claude Code, Codex, Cursor, or Windsurf/)).toBeInTheDocument();
  });

  it("documents the report controls that exist", async () => {
    const dialog = await openGuide("reports");

    expect(within(dialog).getByText(/7-day, 30-day, or 90-day/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Export PDF or Save HTML/)).toBeInTheDocument();
    expect(within(dialog).queryByText(/launch readiness/)).not.toBeInTheDocument();
  });

  it("closes on Escape and returns focus to the Guide button", async () => {
    const user = userEvent.setup();
    render(<PageGuideButton page="issues" />);
    const trigger = screen.getByRole("button", { name: "Open Issues guide" });

    await user.click(trigger);
    expect(screen.getByRole("dialog", { name: "Issues" })).toBeInTheDocument();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(trigger).toHaveFocus());
  });
});
