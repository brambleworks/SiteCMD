import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { RootCauseCallout } from "./RootCauseCallout";
import type { LikelyCause } from "@/lib/types";

describe("RootCauseCallout", () => {
  it("renders nothing when causes are empty", () => {
    const { container } = render(<RootCauseCallout causes={[]} onOpenCause={() => {}} />);
    expect(container.firstChild).toBeNull();
  });

  it("uses 'Likely caused by' for high confidence", () => {
    const causes: LikelyCause[] = [{ checkId: "performance.compression", confidence: "high" }];
    render(<RootCauseCallout causes={causes} onOpenCause={() => {}} />);
    expect(screen.getByText(/Likely caused by/)).toBeInTheDocument();
    expect(screen.getByText("performance.compression")).toBeInTheDocument();
  });

  it("uses 'May be caused by' for medium confidence", () => {
    const causes: LikelyCause[] = [
      { checkId: "performance.unused-javascript", confidence: "medium" },
    ];
    render(<RootCauseCallout causes={causes} onOpenCause={() => {}} />);
    expect(screen.getByText(/May be caused by/)).toBeInTheDocument();
  });
});
