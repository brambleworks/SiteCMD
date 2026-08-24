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
    // (see the cascade test below), it would be the only indicator a keyboard user gets.
    expect(stateStep(rest, hover)).toBeGreaterThanOrEqual(1.3);
  });
});

// --- Cascade-aware focus-ring check (fix-round-2 review) ------------------
//
// A selector-text check ("no :focus-visible rule declares box-shadow: none")
// caught the obvious case but missed two real ones: a *different* rule on the
// same base selector declaring `box-shadow: none !important` (which beats a
// non-important ring in every state, :focus-visible included, regardless of
// source order), and a base rule declaring plain `box-shadow: none` with the
// :focus-visible rule that is supposed to override it never actually
// re-declaring box-shadow at all. Both bugs shipped in fix round 1 even
// though its selector-text check passed. This walks every rule in the real
// stylesheet cascade order (index.css's @import order, then top-to-bottom
// within each file) and checks the two properties fix-round-2 asks for.

interface BoxShadowEvent {
  base: string;
  isFocusVisible: boolean;
  value: string | null; // null: box-shadow not declared in this rule
  important: boolean;
  file: string;
  order: number;
}

/** The real cascade order: index.css's @import list, then any stray file appended after. */
function cssFilesInCascadeOrder(): string[] {
  const indexCss = readFileSync(path.join(STYLES, "..", "index.css"), "utf8");
  const imported = [...indexCss.matchAll(/@import\s+"\.\/styles\/([^"]+)";/g)].map((m) =>
    path.join(STYLES, m[1]!),
  );
  const all = cssFiles(STYLES);
  const stray = all.filter((f) => !imported.includes(f));
  return [...imported.filter((f) => all.includes(f)), ...stray];
}

/** Every comma-separated selector that is a bare base selector or exactly `<base>:focus-visible`. */
function parseBoxShadowEvents(): BoxShadowEvent[] {
  const events: BoxShadowEvent[] = [];
  let order = 0;
  for (const file of cssFilesInCascadeOrder()) {
    // Strip comments first: an unstripped `/* ... */` immediately before a
    // selector merges into the regex's selector capture (nothing here stops
    // at `{`/`}`), which silently splits one real selector into several
    // never-matching "bases" and makes every check below vacuous.
    const source = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
    const ruleRe = /([^{}]+)\{([^{}]*)\}/g;
    let match: RegExpExecArray | null;
    while ((match = ruleRe.exec(source))) {
      const [, selectorList, body] = match;
      order += 1;
      const shadowMatch = /box-shadow\s*:\s*([^;]+);/i.exec(body!);
      const important = shadowMatch !== null && /!important/i.test(shadowMatch[1]!);
      // Strip !important so every later comparison (e.g. matching "none") checks
      // the actual shadow value; `important` above already captured the flag.
      const value = shadowMatch ? shadowMatch[1]!.replace(/!important/i, "").trim() : null;

      for (const rawSelector of selectorList!.split(",")) {
        const selector = rawSelector.trim().replace(/\s+/g, " ");
        if (!selector) continue;
        const focusMatch = /^(.*):focus-visible$/.exec(selector);
        if (focusMatch) {
          events.push({
            base: focusMatch[1]!,
            isFocusVisible: true,
            value,
            important,
            file: path.relative(STYLES, file),
            order,
          });
        } else if (!selector.includes(":")) {
          // A bare selector, i.e. the resting/base declaration, not a :hover
          // or :active variant; those states don't apply while a user is
          // simply tabbed to a control, so they are outside this check's scope.
          events.push({
            base: selector,
            isFocusVisible: false,
            value,
            important,
            file: path.relative(STYLES, file),
            order,
          });
        }
      }
    }
  }
  return events;
}

describe("focus-visible never loses its ring", () => {
  const events = parseBoxShadowEvents();
  const bases = [...new Set(events.map((e) => e.base))];

  function isRing(value: string | null): boolean {
    return value !== null && !/^none$/i.test(value);
  }

  for (const base of bases) {
    const forBase = events.filter((e) => e.base === base);
    const focusRingEvents = forBase.filter((e) => e.isFocusVisible && isRing(e.value));
    if (focusRingEvents.length === 0) continue; // this base never claims to have a ring

    it(`${base} :focus-visible ring is not defeated by an !important base rule`, () => {
      const importantNoneOnBase = forBase.filter(
        (e) => !e.isFocusVisible && e.value !== null && /^none$/i.test(e.value) && e.important,
      );
      expect(
        importantNoneOnBase,
        `${base} declares a :focus-visible ring, but a base rule sets ` +
          `box-shadow: none !important, which always wins over a non-important ring`,
      ).toEqual([]);
    });

    it(`${base}'s base box-shadow: none rules are each followed by the ring`, () => {
      const baseNoneEvents = forBase.filter(
        (e) => !e.isFocusVisible && e.value !== null && /^none$/i.test(e.value),
      );
      const lastRingOrder = Math.max(...focusRingEvents.map((e) => e.order));
      const unfollowed = baseNoneEvents.filter((e) => e.order >= lastRingOrder);
      expect(
        unfollowed,
        `${base} declares box-shadow: none with no later :focus-visible rule ` +
          `re-declaring a real box-shadow`,
      ).toEqual([]);
    });
  }

  it("found at least one selector with a real :focus-visible ring to check", () => {
    // A canary: if this drops to 0, the two checks above silently stop running.
    const withRing = bases.filter((base) =>
      events.some((e) => e.base === base && e.isFocusVisible && isRing(e.value)),
    );
    expect(withRing.length).toBeGreaterThan(0);
  });
});
