import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ResolutionCard } from "./ResolutionCard";

function makeCorrelation(overrides = {}) {
  return {
    correlationType: "deploy_to_resolution",
    description: "Deploy v2.14 resolved 3 SEO issues",
    sourceTimestamp: "2026-05-01T14:30:00.000Z",
    confidence: "high",
    ...overrides,
  };
}

describe("ResolutionCard", () => {
  it("renders nothing for non-resolution correlations", () => {
    const { container } = render(
      <ResolutionCard correlation={makeCorrelation({ correlationType: "deploy_to_regression" })} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing for an unrelated correlation type", () => {
    const { container } = render(
      <ResolutionCard correlation={makeCorrelation({ correlationType: "score_correlation" })} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders title, body, and formatted timestamp for deploy_to_resolution", () => {
    render(<ResolutionCard correlation={makeCorrelation()} />);

    expect(screen.getByText("Resolved")).toBeInTheDocument();
    expect(screen.getByText("Deploy v2.14 resolved 3 SEO issues")).toBeInTheDocument();
    // The formatted timestamp should appear (locale-dependent but non-empty)
    const meta = document.querySelector(".resolution-card-meta");
    expect(meta?.textContent).toBeTruthy();
    expect(meta?.textContent).not.toBe("2026-05-01T14:30:00.000Z");
  });

  it("localizes the timestamp via toLocaleString", () => {
    const iso = "2026-03-15T09:00:00.000Z";
    render(<ResolutionCard correlation={makeCorrelation({ sourceTimestamp: iso })} />);
    const expected = new Date(iso).toLocaleString();
    const meta = document.querySelector(".resolution-card-meta");
    expect(meta?.textContent).toBe(expected);
  });

  it("falls back to the raw value for an invalid timestamp", () => {
    render(<ResolutionCard correlation={makeCorrelation({ sourceTimestamp: "not-a-date" })} />);
    expect(screen.getByText("not-a-date")).toBeInTheDocument();
  });
});
