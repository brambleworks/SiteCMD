import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ScoreRing } from "./score-ring";

describe("ScoreRing", () => {
  it("renders the value and `/ total` label", () => {
    render(<ScoreRing value={87} total={100} />);
    expect(screen.getByText("87")).toBeInTheDocument();
    expect(screen.getByText("/100")).toBeInTheDocument();
  });

  it("renders custom total under the value", () => {
    render(<ScoreRing value={14} total={20} />);
    expect(screen.getByText("14")).toBeInTheDocument();
    expect(screen.getByText("/20")).toBeInTheDocument();
  });

  it("renders just the empty track when value is null (no dash placeholder)", () => {
    const { container } = render(<ScoreRing value={null} />);
    expect(screen.queryByText("-")).not.toBeInTheDocument();
    // Only the background track should render; the progress arc is omitted
    // (SVG has exactly one <circle> element instead of two).
    const circles = container.querySelectorAll("circle");
    expect(circles.length).toBe(1);
  });

  it("renders both background and progress arcs when value is set", () => {
    const { container } = render(<ScoreRing value={50} />);
    const circles = container.querySelectorAll("circle");
    expect(circles.length).toBe(2);
  });

  it("picks the excellent score tone at 90+", () => {
    const { container } = render(<ScoreRing value={95} />);
    const valueSpan = screen.getByText("95");
    expect(valueSpan.getAttribute("style") ?? "").toContain("--score-excellent");
    expect(container.querySelectorAll("circle").length).toBe(2);
  });

  it("picks the critical score tone below 30", () => {
    render(<ScoreRing value={15} />);
    const valueSpan = screen.getByText("15");
    expect(valueSpan.getAttribute("style") ?? "").toContain("--score-critical");
  });

  it("honours an explicit toneVar override (bare variable name)", () => {
    render(<ScoreRing value={50} toneVar="--severity-high" />);
    const valueSpan = screen.getByText("50");
    expect(valueSpan.getAttribute("style") ?? "").toContain("--severity-high");
  });

  it("accepts toneVar already wrapped in var(...) and does not double-wrap", () => {
    render(<ScoreRing value={50} toneVar="var(--severity-critical)" />);
    const valueSpan = screen.getByText("50");
    const style = valueSpan.getAttribute("style") ?? "";
    // Must contain exactly one var wrapper, not var(var(...))
    expect(style).toContain("var(--severity-critical)");
    expect(style).not.toContain("var(var(");
  });

  it("derives the ring fill from an explicit percent when provided", () => {
    // Some score surfaces decouple the displayed value from the arc fill.
    // Both should land in the DOM correctly.
    render(<ScoreRing value={7} total={10} percent={70} />);
    expect(screen.getByText("7")).toBeInTheDocument();
    expect(screen.getByText("/10")).toBeInTheDocument();
  });

  it("renders score labels with a small denominator and without percent symbols", () => {
    render(<ScoreRing value={87} labelMode="percent" />);
    expect(screen.getByText("87")).toBeInTheDocument();
    expect(screen.queryByText("%")).not.toBeInTheDocument();
    expect(screen.getByText("/100")).toBeInTheDocument();
  });

  it("renders value-only labels without the denominator", () => {
    render(<ScoreRing value={87} labelMode="value" />);
    expect(screen.getByText("87")).toBeInTheDocument();
    expect(screen.queryByText("%")).not.toBeInTheDocument();
    expect(screen.queryByText("/100")).not.toBeInTheDocument();
  });

  it("can hide the center label for external score text", () => {
    const { container } = render(<ScoreRing value={87} labelMode="none" />);
    expect(screen.queryByText("87")).not.toBeInTheDocument();
    expect(screen.queryByText("/100")).not.toBeInTheDocument();
    expect(container.querySelectorAll("circle").length).toBe(2);
  });

  it("respects a custom size prop", () => {
    const { container } = render(<ScoreRing value={50} size={64} />);
    const svg = container.querySelector("svg");
    expect(svg?.getAttribute("width")).toBe("64");
    expect(svg?.getAttribute("height")).toBe("64");
  });
});
