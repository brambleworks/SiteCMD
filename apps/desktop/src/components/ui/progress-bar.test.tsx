import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { ProgressBar } from "./progress-bar";

const layoutCss = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../../styles/layout.css"),
  "utf8",
);

/** Background color declared for a `.progress-bar__fill--*` modifier in CSS. */
function fillColor(modifier: string): string {
  const body =
    layoutCss.match(new RegExp(`\\.progress-bar__fill--${modifier}\\s*\\{([^}]*)\\}`))?.[1] ?? "";
  return body.match(/background-color:\s*([^;]+);/)?.[1]?.trim() ?? "";
}

describe("ProgressBar (M6 regression)", () => {
  it("renders the supplied tone fill class on the inner bar", () => {
    const cases: Array<{
      tone: "primary" | "success" | "warning" | "destructive" | "muted";
      cssVar: string;
    }> = [
      { tone: "primary", cssVar: "--brand" },
      { tone: "success", cssVar: "--score-excellent" },
      { tone: "warning", cssVar: "--severity-medium" },
      { tone: "destructive", cssVar: "--severity-critical" },
      { tone: "muted", cssVar: "--muted-foreground" },
    ];

    for (const { tone, cssVar } of cases) {
      const { container } = render(<ProgressBar value={42} tone={tone} />);
      const fills = container.querySelectorAll("div");
      expect(fills.length).toBe(2);
      const inner = fills[1]!;
      expect(inner.className).toContain(`progress-bar__fill--${tone}`);
      expect(fillColor(tone)).toBe(`var(${cssVar})`);
    }
  });

  it("clamps the width style to the 0-100 range", () => {
    const above = render(<ProgressBar value={150} tone="primary" />);
    const innerAbove = above.container.querySelectorAll("div")[1]!;
    expect(innerAbove.getAttribute("style") ?? "").toContain("width: 100%");

    const below = render(<ProgressBar value={-12} tone="primary" />);
    const innerBelow = below.container.querySelectorAll("div")[1]!;
    expect(innerBelow.getAttribute("style") ?? "").toContain("width: 0%");

    const exact = render(<ProgressBar value={37} tone="primary" />);
    const innerExact = exact.container.querySelectorAll("div")[1]!;
    expect(innerExact.getAttribute("style") ?? "").toContain("width: 37%");
  });

  it("exposes accessibility metadata so screen readers can announce progress", () => {
    const { container } = render(<ProgressBar value={45} tone="success" />);
    const track = container.querySelector('[role="progressbar"]')!;
    expect(track.getAttribute("aria-valuemin")).toBe("0");
    expect(track.getAttribute("aria-valuemax")).toBe("100");
    expect(track.getAttribute("aria-valuenow")).toBe("45");
  });

  it("falls back to a primary tone when no tone is given", () => {
    const { container } = render(<ProgressBar value={20} />);
    const inner = container.querySelectorAll("div")[1]!;
    expect(inner.className).toContain("progress-bar__fill--primary");
    expect(fillColor("primary")).toBe("var(--brand)");
  });

  it("supports the legacy color prop using a CSS custom property", () => {
    const { container } = render(<ProgressBar percent={64} color="var(--severity-high)" />);
    const inner = container.querySelectorAll("div")[1]!;
    const style = inner.getAttribute("style") ?? "";
    expect(style).toContain("--progress-color: var(--severity-high)");
    expect(style).toContain("width: 64%");
  });

  it("treats non-finite values as zero", () => {
    const { container } = render(<ProgressBar value={Number.NaN} tone="primary" />);
    const inner = container.querySelectorAll("div")[1]!;
    expect(inner.getAttribute("style") ?? "").toContain("width: 0%");
  });
});
