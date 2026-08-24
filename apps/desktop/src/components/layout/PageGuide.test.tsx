import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { PageGuideButton } from "./PageGuide";

describe("PageGuideButton", () => {
  it("opens a human-readable guide for the current page", async () => {
    const user = userEvent.setup();
    render(<PageGuideButton page="dashboard" />);

    await user.click(screen.getByRole("button", { name: "Open Site Dashboard Guide" }));

    expect(screen.getByRole("dialog", { name: "Site Dashboard Guide" })).toBeInTheDocument();
    expect(screen.getByText("What this page is for")).toBeInTheDocument();
    expect(screen.getByText("Look at first")).toBeInTheDocument();
    expect(
      screen.getByText(/The Dashboard is the fast answer to: is this site basically healthy/),
    ).toBeInTheDocument();
  });

  it("explains the SiteCMD Score from the Issues page strip", async () => {
    const user = userEvent.setup();
    render(<PageGuideButton page="score" />);

    await user.click(screen.getByRole("button", { name: "Open SiteCMD Score Guide" }));

    expect(screen.getByRole("dialog", { name: "SiteCMD Score Guide" })).toBeInTheDocument();
    expect(
      screen.getByText(/It starts at 100 and loses points for every open issue/),
    ).toBeInTheDocument();
  });

  it("closes on Escape and returns focus to the Guide button", async () => {
    const user = userEvent.setup();
    render(<PageGuideButton page="issues" />);
    const trigger = screen.getByRole("button", { name: "Open Issues Guide" });

    await user.click(trigger);
    expect(screen.getByRole("dialog", { name: "Issues Guide" })).toBeInTheDocument();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(trigger).toHaveFocus());
  });
});
