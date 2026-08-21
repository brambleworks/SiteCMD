import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const styles = dirname(fileURLToPath(import.meta.url));
const layoutCss = readFileSync(join(styles, "layout.css"), "utf8");
const dataCss = readFileSync(join(styles, "data.css"), "utf8");
const cardsCss = readFileSync(join(styles, "cards.css"), "utf8");
const tokensCss = readFileSync(join(styles, "tokens.css"), "utf8");

type Rgb = [number, number, number];

function hexToRgb(hex: string): Rgb {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function luminance([r, g, b]: Rgb): number {
  const f = (c: number) => {
    const v = c / 255;
    return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

/** Perceptual distance between two opaque surfaces, same formula as contrast. */
function step(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

function composite(fg: Rgb, alpha: number, bg: Rgb): Rgb {
  return fg.map((c, i) => Math.round(alpha * c + (1 - alpha) * bg[i])) as Rgb;
}

/** Reads a custom property out of the `.dark` block, which is the shipped theme. */
function darkToken(name: string): string {
  const dark = tokensCss.slice(tokensCss.indexOf(".dark {"));
  const match = new RegExp(`${name}:\\s*([^;]+);`).exec(dark);
  if (!match) throw new Error(`${name} not found in the .dark palette`);
  return match[1].trim();
}

function darkSurface(name: string): Rgb {
  const value = darkToken(name);
  if (!/^#[0-9a-f]{6}$/i.test(value)) {
    throw new Error(`${name} is not a plain hex surface: ${value}`);
  }
  return hexToRgb(value);
}

// Parse either the solid or translucent hover-token form.
function parseHoverBackground(): { token: string; alpha: number } {
  const rule = /\.list-row-hover:hover \{([^}]*)\}/.exec(layoutCss);
  if (!rule) throw new Error("no .list-row-hover:hover rule found");
  const body = rule[1];
  const mix = /background-color:\s*color-mix\([^)]*var\((--[a-z0-9-]+)\)\s+(\d+)%/i.exec(body);
  if (mix) return { token: mix[1], alpha: Number(mix[2]) / 100 };
  const solid = /background-color:\s*var\((--[a-z0-9-]+)\)/i.exec(body);
  if (solid) return { token: solid[1], alpha: 1 };
  throw new Error(`no background-color in .list-row-hover:hover: ${body}`);
}

describe("list row hover", () => {
  it("resolves to a different color than the panel it sits on", () => {
    expect(/\.list-row-hover \{([^}]*)\}/.exec(layoutCss)).not.toBeNull();

    expect(/\.panel--muted \{[^}]*var\(--muted\)/.test(cardsCss)).toBe(true);
    const panel = darkSurface("--muted");

    const { token, alpha } = parseHoverBackground();
    const hovered = composite(darkSurface(token), alpha, panel);

    // 1.10 is the minimum perceptible step on the dark surface.
    expect(step(panel, hovered)).toBeGreaterThanOrEqual(1.1);
  });

  it("stays subtle enough to read as a hover rather than a selection", () => {
    const { token, alpha } = parseHoverBackground();
    const panel = darkSurface("--muted");
    const hovered = composite(darkSurface(token), alpha, panel);

    expect(step(panel, hovered)).toBeLessThanOrEqual(1.3);
  });

  it("keeps the pointer affordance on the row", () => {
    const rule = /\.list-row-hover \{([^}]*)\}/.exec(layoutCss);
    expect(rule![1]).toMatch(/cursor:\s*pointer/);
    expect(rule![1]).toMatch(/transition:\s*var\(--transition-colors\)/);
  });
});

describe("list row title hover", () => {
  it("turns the title brand blue, from the same rule the Issues list uses", () => {
    // One selector list owns "a hovered row title goes blue" for Issues, Alerts,
    // and Updates. Updates joining it is what keeps the pages from drifting.
    const rule = /([^}]*)\{\s*color:\s*var\(--brand\)\s*!important;\s*\}/.exec(dataCss);
    expect(rule, "no rule paints a hovered row title with --brand").not.toBeNull();

    const selectors = rule![1];
    expect(selectors).toContain(".list-row--issue:hover .list-row__title");
    expect(selectors).toContain(".list-row-hover:hover .list-row__title");
  });

  it("nudges the chevron on the same rows", () => {
    const chevron = /([^}]*)\{[^}]*scale\(1\.18\)/.exec(dataCss);
    expect(chevron).not.toBeNull();
    expect(chevron![1]).toContain(".list-row-hover:hover .list-row__chevron");
  });
});
