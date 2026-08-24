import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const SCRIPT = path.join(ROOT, "tools/scripts/audit-frontend-filenames.mjs");

describe("frontend filename audit", () => {
  it("treats only use plus PascalCase as a hook filename", () => {
    // The regression: "user-facing-error.ts" starts with the letters "use"
    // and was misclassified as a hook; the matcher must require use[A-Z].
    const source = fs.readFileSync(SCRIPT, "utf8");
    expect(source).toContain("/^use[A-Z]/.test(baseName)");
    expect(source).not.toContain('baseName.startsWith("use")');
  });

  it("passes the real tree, which contains user-facing-error.ts", () => {
    expect(fs.existsSync(path.join(ROOT, "apps/desktop/src/lib/user-facing-error.ts"))).toBe(true);
    const result = spawnSync(process.execPath, [SCRIPT], { cwd: ROOT, encoding: "utf8" });
    expect(result.status, result.stdout + result.stderr).toBe(0);
  });
});
