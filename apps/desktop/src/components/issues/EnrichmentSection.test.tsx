import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { EnrichmentSection } from "./EnrichmentSection";

describe("EnrichmentSection", () => {
  it("renders nothing when empty", () => {
    const { container } = render(<EnrichmentSection enrichments={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders fieldLcp as a human-readable fact", () => {
    render(
      <EnrichmentSection
        enrichments={[{ kind: "fieldLcp", p75_ms: 3200, url: "/pricing", source: "gsc" }]}
      />,
    );
    expect(screen.getByText(/3.2s/i)).toBeInTheDocument();
    expect(screen.getByText(/google search console/i)).toBeInTheDocument();
  });

  it("renders multiple enrichments", () => {
    render(
      <EnrichmentSection
        enrichments={[
          { kind: "certExpiresIn", days: 3, source: "uptimerobot" },
          { kind: "botTrafficPct", value: 0.42, source: "cloudflare" },
        ]}
      />,
    );
    expect(screen.getByText(/expires in 3d/i)).toBeInTheDocument();
    expect(screen.getByText(/42%/i)).toBeInTheDocument();
  });

  it("treats certExpiresIn days<=0 as expired", () => {
    render(
      <EnrichmentSection
        enrichments={[{ kind: "certExpiresIn", days: 0, source: "uptimerobot" }]}
      />,
    );
    expect(screen.getByText(/cert expired/i)).toBeInTheDocument();
  });
});
