import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const STYLES = path.dirname(fileURLToPath(import.meta.url));
const tokensCss = readFileSync(path.join(STYLES, "tokens.css"), "utf8");

function cssFiles(dir: string, files: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) cssFiles(full, files);
    else if (entry.name.endsWith(".css")) files.push(full);
  }
  return files;
}

// Isolate each theme's block so a token name shared by both (e.g. --brand-button)
// resolves to the right value instead of whichever occurrence regex finds first.
const LIGHT_THEME = tokensCss.slice(0, tokensCss.indexOf(".dark {"));
const DARK_THEME = tokensCss.slice(
  tokensCss.indexOf(".dark {"),
  tokensCss.indexOf("/* Shared palette and motion */"),
);

function oklchToken(theme: string, name: string): [number, number, number] {
  const match = new RegExp(`${name}:\\s*oklch\\(([^)/]+)\\)`).exec(theme);
  if (!match) throw new Error(`${name} not found as a plain oklch() token`);
  const [l, c, h] = match[1].trim().split(/\s+/).map(Number);
  return [l!, c!, h!];
}

/**
 * OKLCH -> linear-light sRGB (Bjorn Ottosson's reference conversion), then WCAG
 * relative luminance. The matrix multiplication below already yields linear-light
 * sRGB primaries, which is exactly what the WCAG luminance weights expect: no
 * gamma encode/decode round trip is needed (or correct) in between.
 */
function oklchLuminance([L, C, Hdeg]: [number, number, number]): number {
  const h = (Hdeg * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);

  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ ** 3;
  const m = m_ ** 3;
  const s = s_ ** 3;

  const clamp = (channel: number) => Math.max(0, Math.min(1, channel));
  const r = clamp(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s);
  const g = clamp(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s);
  const bch = clamp(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s);

  return 0.2126 * r + 0.7152 * g + 0.0722 * bch;
}

function stateStep(a: [number, number, number], b: [number, number, number]): number {
  const [hi, lo] = [oklchLuminance(a), oklchLuminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe("primary button hover/focus state contrast", () => {
  it.each([
    ["light", LIGHT_THEME],
    ["dark", DARK_THEME],
  ])("--brand-button-hover is a visible step from --brand-button in %s theme", (_theme, block) => {
    const rest = oklchToken(block, "--brand-button");
    const hover = oklchToken(block, "--brand-button-hover");

    // Below this, the background swap that .btn--default:hover/:focus-visible/:active
    // relies on is imperceptible, and with box-shadow suppressed on :focus-visible
    // (see the second test below), it would be the only indicator a keyboard user gets.
    expect(stateStep(rest, hover)).toBeGreaterThanOrEqual(1.3);
  });
});

describe("focus-visible never loses its ring", () => {
  it("has no :focus-visible selector that declares box-shadow: none", () => {
    const offenders: string[] = [];
    for (const file of cssFiles(STYLES)) {
      const source = readFileSync(file, "utf8");
      const ruleRe = /([^{}]+)\{([^{}]*)\}/g;
      let match: RegExpExecArray | null;
      while ((match = ruleRe.exec(source))) {
        const [, selector, body] = match;
        if (!selector!.includes(":focus-visible")) continue;
        if (/box-shadow\s*:\s*none/i.test(body!)) {
          offenders.push(
            `${path.relative(STYLES, file)}: ${selector!.trim().replace(/\s+/g, " ")}`,
          );
        }
      }
    }
    expect(offenders, "a :focus-visible rule is suppressing the keyboard focus ring").toEqual([]);
  });
});
