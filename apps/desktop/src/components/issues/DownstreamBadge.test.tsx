import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DownstreamBadge } from "./DownstreamBadge";

describe("DownstreamBadge", () => {
  it("renders nothing when count is 0", () => {
    const { container } = render(<DownstreamBadge count={0} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders '+N downstream' for non-zero count", () => {
    render(<DownstreamBadge count={3} />);
    expect(screen.getByText("+3 downstream")).toBeInTheDocument();
  });

  it("renders correctly for count=1", () => {
    render(<DownstreamBadge count={1} />);
    expect(screen.getByText("+1 downstream")).toBeInTheDocument();
  });

  it("renders correctly for count=12", () => {
    render(<DownstreamBadge count={12} />);
    expect(screen.getByText("+12 downstream")).toBeInTheDocument();
  });
});
