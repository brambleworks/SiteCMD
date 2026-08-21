import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { AnomalyBadge } from "./AnomalyBadge";

describe("AnomalyBadge", () => {
  it("renders nothing when score is null", () => {
    const { container } = render(<AnomalyBadge score={null} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders score with one decimal place", () => {
    render(<AnomalyBadge score={3.7} />);
    expect(screen.getByText(/3.7σ/)).toBeInTheDocument();
  });

  it("sets data-sev='warning' when |score| <= 5", () => {
    const { container } = render(<AnomalyBadge score={3.5} />);
    const badge = container.querySelector("[data-sev='warning']");
    expect(badge).toBeInTheDocument();
  });

  it("sets data-sev='warning' when |score| exactly equals 5", () => {
    const { container } = render(<AnomalyBadge score={5} />);
    const badge = container.querySelector("[data-sev='warning']");
    expect(badge).toBeInTheDocument();
  });

  it("sets data-sev='critical' when |score| > 5", () => {
    const { container } = render(<AnomalyBadge score={7.2} />);
    const badge = container.querySelector("[data-sev='critical']");
    expect(badge).toBeInTheDocument();
  });

  it("sets data-sev='critical' for negative scores with |score| > 5", () => {
    const { container } = render(<AnomalyBadge score={-6.5} />);
    const badge = container.querySelector("[data-sev='critical']");
    expect(badge).toBeInTheDocument();
    expect(screen.getByText(/-6.5σ/)).toBeInTheDocument();
  });

  it("sets data-sev='warning' for negative scores with |score| <= 5", () => {
    const { container } = render(<AnomalyBadge score={-4.2} />);
    const badge = container.querySelector("[data-sev='warning']");
    expect(badge).toBeInTheDocument();
    expect(screen.getByText(/-4.2σ/)).toBeInTheDocument();
  });

  it("renders zero score as warning", () => {
    const { container } = render(<AnomalyBadge score={0} />);
    expect(container.querySelector("[data-sev='warning']")).toBeInTheDocument();
    expect(screen.getByText(/0.0σ/)).toBeInTheDocument();
  });
});
