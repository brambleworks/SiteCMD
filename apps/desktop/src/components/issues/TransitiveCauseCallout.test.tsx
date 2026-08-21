import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { TransitiveCauseCallout } from "./TransitiveCauseCallout";

describe("TransitiveCauseCallout", () => {
  it("renders nothing when empty", () => {
    const { container } = render(<TransitiveCauseCallout causes={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders the deepest chain", () => {
    const causes = [
      {
        checkId: "performance.lcp",
        path: ["analytics.conversion-drop", "performance.lcp"],
        confidence: "medium" as const,
        depth: 1,
      },
      {
        checkId: "performance.compression",
        path: ["analytics.conversion-drop", "performance.lcp", "performance.compression"],
        confidence: "medium" as const,
        depth: 2,
      },
    ];
    render(<TransitiveCauseCallout causes={causes} />);
    expect(screen.getByText(/compression.*lcp.*conversion-drop/)).toBeInTheDocument();
  });
});
