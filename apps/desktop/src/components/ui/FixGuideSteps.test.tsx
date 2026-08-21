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
});
