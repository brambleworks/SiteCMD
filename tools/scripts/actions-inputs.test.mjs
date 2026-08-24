import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("../..", import.meta.url));
const read = (file) => fs.readFileSync(path.join(ROOT, file), "utf8");

describe("sitecmd-gate action inputs", () => {
  const action = read(".github/actions/sitecmd-gate/action.yml");
  it("accepts fail-on and keeps threshold as a deprecated alias", () => {
    expect(action).toMatch(/^ {2}fail-on:\n {4}description:/m);
    expect(action).toMatch(
      /^ {2}threshold:\n {4}description: >-\n {6}Deprecated alias of fail-on/m,
    );
    expect(action).toMatch(/--fail-on "\$SITECMD_FAIL_ON"/);
    expect(action).not.toMatch(/--threshold "\$SITECMD_THRESHOLD"/);
  });
});
