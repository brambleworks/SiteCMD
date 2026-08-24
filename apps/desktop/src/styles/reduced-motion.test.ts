/// <reference types="node" />

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const STYLES = path.dirname(fileURLToPath(import.meta.url));
const REDUCED_MOTION = "@media (prefers-reduced-motion: reduce)";

function cssFiles(dir: string, files: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) cssFiles(full, files);
    else if (entry.name.endsWith(".css")) files.push(full);
  }
  return files;
}

/** The real cascade order: index.css's @import list, then any stray file appended after. */
function cssFilesInCascadeOrder(): string[] {
  const indexCss = readFileSync(path.join(STYLES, "..", "index.css"), "utf8");
  const imported = [...indexCss.matchAll(/@import\s+"\.\/styles\/([^"]+)";/g)].map((m) =>
    path.join(STYLES, m[1]!),
  );
  const all = cssFiles(STYLES);
  const stray = all.filter((file) => !imported.includes(file));
  return [...imported.filter((file) => all.includes(file)), ...stray];
}

// Same trap focus-visible.test.ts documents: an unstripped `/* ... */` in front of
// a rule merges into the selector capture below and quietly turns every check
// vacuous, because nothing in the rule regex stops at a comment boundary.
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Body and end offset of the brace-delimited block opened at or after `from`. */
function blockAt(source: string, from: number): { body: string; end: number } {
  const open = source.indexOf("{", from);
  if (open < 0) throw new Error(`no block opens at offset ${from}`);
  let depth = 0;
  for (let i = open; i < source.length; i += 1) {
    if (source[i] === "{") depth += 1;
    else if (source[i] === "}") {
      depth -= 1;
      if (depth === 0) return { body: source.slice(open + 1, i), end: i + 1 };
    }
  }
  throw new Error(`unterminated block at offset ${from}`);
}

interface StyleRule {
  selectors: string[];
  body: string;
}

/** Every innermost rule, with its comma-separated selectors split out. */
function styleRules(source: string): StyleRule[] {
  const found: StyleRule[] = [];
  const ruleRe = /([^{}]+)\{([^{}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = ruleRe.exec(source))) {
    found.push({
      selectors: match[1]!
        .split(",")
        .map((selector) => selector.trim().replace(/\s+/g, " "))
        .filter(Boolean),
      body: match[2]!,
    });
  }
  return found;
}

/** The cascade layer an offset sits in, or null when the rule is unlayered. */
function layerAt(source: string, offset: number): string | null {
  let enclosing: string | null = null;
  for (const match of source.matchAll(/@layer\s+([\w-]+)\s*\{/g)) {
    const { end } = blockAt(source, match.index);
    if (match.index < offset && offset < end) enclosing = match[1]!;
  }
  return enclosing;
}

/** Layer names in first-use order, which is the order the cascade sorts them in. */
function declaredLayerOrder(): string[] {
  const order: string[] = [];
  for (const file of cssFilesInCascadeOrder()) {
    const source = stripComments(readFileSync(file, "utf8"));
    for (const match of source.matchAll(/@layer\s+([\w-]+)\s*\{/g)) {
      if (!order.includes(match[1]!)) order.push(match[1]!);
    }
  }
  return order;
}

const animations = stripComments(readFileSync(path.join(STYLES, "animations.css"), "utf8"));
const blockStart = animations.indexOf(REDUCED_MOTION);
const block = blockStart >= 0 ? blockAt(animations, blockStart) : null;
const blockSelectors = new Set(
  block ? styleRules(block.body).flatMap((rule) => rule.selectors) : [],
);

interface MotionDeclaration {
  selector: string;
  property: string;
  value: string;
  file: string;
}

/**
 * Every `animation-*`/`transition-*` !important declaration outside the reset that
 * still asks for real time. `transition: none !important` and a `0s` duration are
 * left out: they stop motion, which is what reduced motion wants anyway.
 */
function importantMotionDeclarations(): MotionDeclaration[] {
  const found: MotionDeclaration[] = [];
  for (const file of cssFilesInCascadeOrder()) {
    let source = stripComments(readFileSync(file, "utf8"));
    if (path.basename(file) === "animations.css" && block) {
      source = source.slice(0, blockStart) + source.slice(block.end);
    }
    for (const rule of styleRules(source)) {
      for (const declaration of rule.body.split(";")) {
        const parsed = /^\s*([a-z-]+)\s*:\s*([\s\S]+)$/i.exec(declaration);
        if (!parsed) continue;
        const property = parsed[1]!;
        const value = parsed[2]!;
        if (!/^(?:animation|transition)(?:-|$)/i.test(property)) continue;
        if (!/!important/i.test(value)) continue;
        const times = [...value.matchAll(/(-?\d*\.?\d+)m?s\b/gi)];
        if (!times.some((time) => Number(time[1]) !== 0)) continue;
        for (const selector of rule.selectors) {
          found.push({
            selector,
            property,
            value: value.replace(/!important/i, "").trim(),
            file: path.relative(STYLES, file),
          });
        }
      }
    }
  }
  return found;
}

describe("reduced motion", () => {
  it("disables every animation and transition when the OS asks for reduced motion", () => {
    expect(block, "animations.css needs one global prefers-reduced-motion block").not.toBeNull();
    const body = block!.body;
    expect(body).toMatch(/^\s*\*,\s*\n\s*\*::before,\s*\n\s*\*::after \{/m);
    expect(body).toContain("animation-delay: -1ms !important;");
    expect(body).toContain("animation-duration: 0.01ms !important;");
    expect(body).toContain("animation-iteration-count: 1 !important;");
    expect(body).toContain("transition-delay: 0s !important;");
    expect(body).toContain("transition-duration: 0.01ms !important;");
    expect(body).toContain("scroll-behavior: auto !important;");
  });

  it("keeps every keyframe in animations.css so the global block covers it", () => {
    const strays = cssFiles(STYLES)
      .filter((file) => !file.endsWith("animations.css"))
      .filter((file) => /@keyframes\s/.test(readFileSync(file, "utf8")))
      .map((file) => path.relative(STYLES, file));
    expect(strays).toEqual([]);
  });

  it("has exactly one reduced-motion block, not per-component copies", () => {
    const copies = cssFiles(STYLES)
      .filter((file) => !file.endsWith("animations.css"))
      .filter((file) => readFileSync(file, "utf8").includes("prefers-reduced-motion"))
      .map((file) => path.relative(STYLES, file));
    expect(copies).toEqual([]);
    expect(animations.split(REDUCED_MOTION)).toHaveLength(2);
  });

  // --- Cascade-aware: nothing may outbid the reset -------------------------
  //
  // The reset is !important, so only two things can beat it: a cascade layer that
  // sorts earlier (!important reverses layer order, which is why an unlayered copy
  // of this block is the weakest author rule there is), or a higher-specificity
  // !important rule in the same layer. Both are checked below, because a guardrail
  // that only asserts the block's own text passes while the behavior it names is
  // false, which is how .scan-run-button kept its 180ms transition under reduce.

  it("has no @layer order statement, so first use decides layer order", () => {
    const statements = cssFilesInCascadeOrder()
      .filter((file) => /@layer\s+[^{};]+;/.test(stripComments(readFileSync(file, "utf8"))))
      .map((file) => path.relative(STYLES, file));
    expect(statements).toEqual([]);
  });

  it("resets from the first cascade layer, which !important sorts above the rest", () => {
    const order = declaredLayerOrder();
    expect(order.length).toBeGreaterThan(0);
    expect(layerAt(animations, blockStart)).toBe(order[0]);
  });

  it("sees the class-level !important motion rules it is meant to police", () => {
    // A canary: if the parser ever stops finding these, the check below is vacuous.
    const declarations = importantMotionDeclarations();
    expect(declarations.length).toBeGreaterThan(0);
    expect(declarations.map((declaration) => declaration.selector)).toContain(".scan-run-button");
  });

  it("names every selector that declares !important motion of its own", () => {
    const unnamed = importantMotionDeclarations().filter(
      (declaration) => !blockSelectors.has(declaration.selector),
    );
    expect(
      unnamed.map((d) => `${d.file}: ${d.selector} { ${d.property}: ${d.value} !important }`),
      "these rules outbid the reduced-motion reset; give the block a matching " +
        "selector so the reset wins at equal specificity",
    ).toEqual([]);
  });
});
