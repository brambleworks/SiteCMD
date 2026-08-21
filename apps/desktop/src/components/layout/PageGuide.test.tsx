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
});
