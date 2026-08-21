import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { CrossPageBadge } from "./CrossPageBadge";

describe("CrossPageBadge", () => {
  it("renders nothing when pages.length is 0", () => {
    const { container } = render(<CrossPageBadge pages={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing when pages.length is 1", () => {
    const { container } = render(<CrossPageBadge pages={["/home"]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders 'Affects N pages' when pages.length > 1", () => {
    render(<CrossPageBadge pages={["/home", "/about"]} />);
    expect(screen.getByText("Affects 2 pages")).toBeInTheDocument();
  });

  it("renders correct count for multiple pages", () => {
    render(<CrossPageBadge pages={["/a", "/b", "/c", "/d", "/e"]} />);
    expect(screen.getByText("Affects 5 pages")).toBeInTheDocument();
  });
});
