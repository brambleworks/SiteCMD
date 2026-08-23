import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const E2E = path.join(ROOT, "apps/desktop/e2e");

describe("every Playwright spec runs axe", () => {
  const specs = fs.readdirSync(E2E).filter((name) => name.endsWith(".spec.ts"));

  it("has specs to check", () => {
    expect(specs.length).toBeGreaterThanOrEqual(3);
  });

  it.each(specs)("%s asserts no accessibility violations in every test", (spec) => {
    const source = fs.readFileSync(path.join(E2E, spec), "utf8");
    expect(source).toContain('from "./fixtures/accessibility"');
    const tests = (source.match(/^\s*test\(/gm) ?? []).length;
    const assertions = (source.match(/await expectNoAccessibilityViolations\(/g) ?? []).length;
    expect(tests).toBeGreaterThan(0);
    expect(assertions).toBeGreaterThanOrEqual(tests);
  });
});
