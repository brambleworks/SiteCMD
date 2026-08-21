import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CATALOG_DEV_ENV } from "./lib/catalog-dev-env.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

describe("catalog dev environment", () => {
  it("matches the release workflow exactly, so dev tests what ships", () => {
    const releaseWorkflow = fs.readFileSync(
      path.join(ROOT, ".github/workflows/release.yml"),
      "utf8",
    );
    const live = releaseWorkflow
      .split("\n")
      .filter((line) => !line.trimStart().startsWith("#"))
      .join("\n");
    for (const [name, value] of Object.entries(CATALOG_DEV_ENV)) {
      expect(live, `${name} in release.yml`).toContain(`${name}: "${value}"`);
    }
  });

  it("rejects a commented-out setting, which ships a catalog-less build", () => {
    const real = fs.readFileSync(path.join(ROOT, ".github/workflows/release.yml"), "utf8");
    const line = `SITECMD_CATALOG_PUBLIC_KEY: "${CATALOG_DEV_ENV.SITECMD_CATALOG_PUBLIC_KEY}"`;
    expect(real, "fixture no longer matches release.yml").toContain(line);
    const mutated = real.replace(line, `# ${line}`);

    const live = mutated
      .split("\n")
      .filter((l) => !l.trimStart().startsWith("#"))
      .join("\n");

    expect(live).not.toContain(line);
  });

  it("carries exactly the three compile-time catalog settings", () => {
    expect(Object.keys(CATALOG_DEV_ENV).sort()).toEqual([
      "SITECMD_ACTIVATION_ENDPOINT",
      "SITECMD_CATALOG_ENDPOINT",
      "SITECMD_CATALOG_PUBLIC_KEY",
    ]);
  });
});
