import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const stylesDir = dirname(fileURLToPath(import.meta.url));
const baseCss = readFileSync(join(stylesDir, "base.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const rules = [...baseCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((match) => ({
  selectors: match[1].split(",").map((selector) => selector.trim()),
  declarations: match[2],
}));

describe("link cursors", () => {
  it.each(["a[href]", '[role="link"]'])(
    "%s gets a pointer without relying on a component class",
    (selector) => {
      expect(
        rules.some(
          (rule) =>
            rule.selectors.includes(selector) && /cursor:\s*pointer\s*;/.test(rule.declarations),
        ),
      ).toBe(true);
    },
  );

  it("keeps the disabled cursor override after the link default", () => {
    const linkRule = rules.findIndex((rule) => rule.selectors.includes('[role="link"]'));
    const disabledRule = rules.findIndex((rule) =>
      rule.selectors.includes('[aria-disabled="true"]'),
    );

    expect(linkRule).toBeGreaterThanOrEqual(0);
    expect(disabledRule).toBeGreaterThan(linkRule);
    expect(rules[disabledRule].declarations).toMatch(/cursor:\s*not-allowed\s*;/);
  });
});
