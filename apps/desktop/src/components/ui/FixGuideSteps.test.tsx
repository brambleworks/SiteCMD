import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FixGuideSteps } from "./FixGuideSteps";

describe("FixGuideSteps", () => {
  it("uses the parent dossier heading instead of rendering a duplicate subhead", () => {
    render(
      <FixGuideSteps
        guide={{
          effort: "quick",
          effortMinutes: 5,
          steps: ["Add the canonical URL to the page head."],
        }}
      />,
    );

    expect(screen.queryByText("How to fix")).not.toBeInTheDocument();
    expect(screen.getByText(/Add the canonical URL/).closest("li")).toHaveClass("body-text");
  });

  it("opens with the plain-English lead before the numbered steps", () => {
    render(
      <FixGuideSteps
        guide={{
          effort: "quick",
          effortMinutes: 5,
          lead: "The page has no canonical address, so search engines may index copies.",
          steps: ["Add the canonical URL to the page head."],
        }}
      />,
    );

    const lead = screen.getByText(/The page has no canonical address/);
    expect(lead).toHaveClass("fix-guide-lead");
    expect(lead.compareDocumentPosition(screen.getByRole("list"))).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });
});
