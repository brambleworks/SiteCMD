import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const E2E = path.join(ROOT, "apps/desktop/e2e");
const TEST_CALL = /^[ \t]*test\(\s*"([^"]*)"/gm;
const AXE_CALL = "await expectNoAccessibilityViolations(";

/** Each `test(` call, with the source from that call up to the next one. */
function testBlocks(source) {
  const calls = [...source.matchAll(TEST_CALL)];
  return calls.map((call, index) => ({
    title: call[1],
    body: source.slice(call.index, calls[index + 1]?.index ?? source.length),
  }));
}

/** Titles of the tests that never call axe. */
function axeCoverageGaps(source) {
  return testBlocks(source)
    .filter((block) => !block.body.includes(AXE_CALL))
    .map((block) => block.title);
}

describe("every Playwright spec runs axe", () => {
  const specs = fs.readdirSync(E2E).filter((name) => name.endsWith(".spec.ts"));

  it("has specs to check", () => {
    expect(specs.length).toBeGreaterThanOrEqual(3);
  });

  it.each(specs)("%s asserts no accessibility violations in every test", (spec) => {
    const source = fs.readFileSync(path.join(E2E, spec), "utf8");
    expect(source).toContain('from "./fixtures/accessibility"');
    const blocks = testBlocks(source);
    expect(blocks.length).toBeGreaterThan(0);
    // A canary: a title the parser cannot read would silently drop that test from
    // the check below, so the block count has to match the raw `test(` count.
    expect(blocks).toHaveLength((source.match(/^[ \t]*test\(/gm) ?? []).length);
    expect(axeCoverageGaps(source)).toEqual([]);
  });

  it("counts axe assertions per test, not per file", () => {
    // Negative control: two tests, both axe calls in the first. The per-file count
    // this check replaced was satisfied by exactly this shape.
    const fixture = [
      'import { expectNoAccessibilityViolations } from "./fixtures/accessibility";',
      'test.describe("first run", () => {',
      '  test("walks the welcome screen", async ({ page }) => {',
      '    await expectNoAccessibilityViolations(page, "welcome");',
      '    await expectNoAccessibilityViolations(page, "add project");',
      "  });",
      '  test("walks the tour", async ({ page }) => {',
      '    await page.goto("/");',
      "  });",
      "});",
      "",
    ].join("\n");

    const tests = (fixture.match(/^[ \t]*test\(/gm) ?? []).length;
    const assertions = (fixture.match(/await expectNoAccessibilityViolations\(/g) ?? []).length;
    expect(assertions).toBeGreaterThanOrEqual(tests);

    expect(axeCoverageGaps(fixture)).toEqual(["walks the tour"]);
  });
});
