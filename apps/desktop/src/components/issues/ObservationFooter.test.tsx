import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ObservationFooter } from "./ObservationFooter";

describe("ObservationFooter", () => {
  it("renders nothing when count is 0", () => {
    const { container } = render(<ObservationFooter count={0} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders singular form for 1", () => {
    render(<ObservationFooter count={1} />);
    expect(screen.getByText(/1 time before/i)).toBeInTheDocument();
  });

  it("renders plural form for >1", () => {
    render(<ObservationFooter count={4} />);
    expect(screen.getByText(/4 times before/i)).toBeInTheDocument();
  });
});
