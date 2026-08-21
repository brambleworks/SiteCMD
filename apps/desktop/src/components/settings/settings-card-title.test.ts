import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const styles = join(dirname(fileURLToPath(import.meta.url)), "../../styles");
const settingsCss = readFileSync(join(styles, "pages/settings.css"), "utf8");
const tokensCss = readFileSync(join(styles, "tokens.css"), "utf8");

/** oklch -> sRGB (0-1 per channel), clamped to gamut. */
function oklchToSrgb(L: number, C: number, hueDeg: number): [number, number, number] {
  const h = (hueDeg * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const linear = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  return linear.map((c) => {
    const encoded = c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(Math.max(c, 0), 1 / 2.4) - 0.055;
    return Math.min(1, Math.max(0, encoded));
  }) as [number, number, number];
}

function hexToSrgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

function luminance([r, g, b]: [number, number, number]): number {
  const f = (c: number) => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

function contrast(fg: [number, number, number], bg: [number, number, number]): number {
  const [hi, lo] = [luminance(fg), luminance(bg)].sort((a, b) => b - a);
  return (hi + 0.05) / (lo + 0.05);
}

/** Reads a custom property out of the `.dark` block, which is the shipped theme. */
function darkToken(name: string): string {
  const dark = tokensCss.slice(tokensCss.indexOf(".dark {"));
  const match = new RegExp(`${name}:\\s*([^;]+);`).exec(dark);
  if (!match) throw new Error(`${name} not found in the .dark palette`);
  return match[1].trim();
}

describe("settings card title", () => {
  it("uses the accent orange rather than plain foreground", () => {
    const rule = /\.settings-card-title \{([^}]*)\}/.exec(settingsCss);
    expect(rule).not.toBeNull();
    expect(rule![1]).toContain("var(--brand-accent)");
    expect(rule![1]).not.toContain("var(--foreground)");
  });

  it("lets the destructive card keep its red title", () => {
    // Same specificity, so source order decides. `-critical` must come second.
    const base = settingsCss.indexOf(".settings-card-title {");
    const critical = settingsCss.indexOf(".settings-card-title-critical {");
    expect(base).toBeGreaterThan(-1);
    expect(critical).toBeGreaterThan(base);
  });

  it("clears WCAG AA on the card surface in the shipped dark theme", () => {
    const accent = darkToken("--brand-accent");
    const parsed = /oklch\(([\d.]+) ([\d.]+) ([\d.]+)\)/.exec(accent);
    expect(parsed, `--brand-accent is not a plain oklch triple: ${accent}`).not.toBeNull();
    const [, L, C, h] = parsed!;
    const fg = oklchToSrgb(Number(L), Number(C), Number(h));

    // 14px semibold is normal-size text under WCAG, so the bar is 4.5:1.
    for (const surface of ["--card", "--background"]) {
      const bg = darkToken(surface);
      expect(bg, `${surface} is not a hex value: ${bg}`).toMatch(/^#[0-9a-f]{6}$/i);
      expect(contrast(fg, hexToSrgb(bg))).toBeGreaterThanOrEqual(4.5);
    }
  });
});
