import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

describe("repo test sweep scope", () => {
  it("excludes agent worktrees so their tree copies are never swept", () => {
    const config = fs.readFileSync(path.join(ROOT, "vitest.config.mjs"), "utf8");
    expect(config).toContain('"**/.claude/**"');
    expect(config).toContain("configDefaults.exclude");
  });
});
