import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const stylesDir = dirname(fileURLToPath(import.meta.url));
const utilitiesCss = readFileSync(join(stylesDir, "utilities.css"), "utf8");

/** The declaration block of `.sr-only`, or null when the rule is absent. */
function srOnlyBlock(): string | null {
  const match = utilitiesCss.match(/^\.sr-only\s*\{([^}]*)\}/m);
  return match ? match[1] : null;
}

describe(".sr-only", () => {
  it("is defined as a real rule", () => {
    expect(
      srOnlyBlock(),
      ".sr-only has no rule; every visually-hidden label renders",
    ).not.toBeNull();
  });

  it("clips the text instead of removing it from the accessibility tree", () => {
    const block = srOnlyBlock() ?? "";
    // Clipped to a 1px box and taken out of flow: sighted users see nothing,
    // screen readers still announce it.
    expect(block).toMatch(/position:\s*absolute/);
    expect(block).toMatch(/clip-path:\s*inset\(50%\)/);
    expect(block).toMatch(/overflow:\s*hidden/);
    expect(block).toMatch(/width:\s*1px/);
    expect(block).toMatch(/height:\s*1px/);
  });

  it("does not hide the text from assistive technology", () => {
    const block = srOnlyBlock() ?? "";
    expect(block, ".sr-only must not use display:none").not.toMatch(/display:\s*none/);
    expect(block, ".sr-only must not use visibility:hidden").not.toMatch(/visibility:\s*hidden/);
    expect(block, ".sr-only must not use aria-hidden-equivalent content-visibility").not.toMatch(
      /content-visibility:\s*hidden/,
    );
  });
});
