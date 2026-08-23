import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PageGuideButton } from "./PageGuide";

describe("PageGuideButton", () => {
  it("opens a human-readable guide for the current page", () => {
    render(<PageGuideButton page="dashboard" />);

    fireEvent.click(screen.getByRole("button", { name: "Open Site Dashboard Guide" }));

    expect(screen.getByRole("dialog", { name: "Site Dashboard Guide" })).toBeInTheDocument();
    expect(screen.getByText("What this page is for")).toBeInTheDocument();
    expect(screen.getByText("Look at first")).toBeInTheDocument();
    expect(
      screen.getByText(/The Dashboard is the fast answer to: is this site basically healthy/),
    ).toBeInTheDocument();
  });

  it("explains the SiteCMD Score from the Issues page strip", () => {
    render(<PageGuideButton page="score" />);

    fireEvent.click(screen.getByRole("button", { name: "Open SiteCMD Score Guide" }));

    expect(screen.getByRole("dialog", { name: "SiteCMD Score Guide" })).toBeInTheDocument();
    expect(
      screen.getByText(/It starts at 100 and loses points for every open issue/),
    ).toBeInTheDocument();
  });
});
