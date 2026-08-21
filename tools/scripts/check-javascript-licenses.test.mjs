import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  allowedLicensesFromCargoDeny,
  javascriptLicenseFailures,
  licenseExpressionIsAllowed,
} from "./lib/javascript-license-policy.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const allowedLicenses = allowedLicensesFromCargoDeny(
  readFileSync(path.join(ROOT, "apps/desktop/src-tauri/deny.toml"), "utf8"),
);

function inventoryWith(license) {
  return {
    packages: [{ ecosystem: "npm", name: "example", version: "1.0.0", license }],
  };
}

describe("JavaScript dependency license policy", () => {
  it("accepts the current shipped npm inventory", () => {
    const inventory = JSON.parse(
      readFileSync(path.join(ROOT, "THIRD_PARTY_DEPENDENCIES.json"), "utf8"),
    );
    expect(javascriptLicenseFailures(inventory, allowedLicenses)).toEqual([]);
  });

  it("accepts an expression when at least one OR choice is allowed", () => {
    expect(licenseExpressionIsAllowed("GPL-3.0-only OR MIT", allowedLicenses)).toBe(true);
  });

  it("rejects a required disallowed license", () => {
    expect(
      javascriptLicenseFailures(inventoryWith("MIT AND GPL-3.0-only"), allowedLicenses),
    ).toEqual(['example@1.0.0: disallowed license expression "MIT AND GPL-3.0-only"']);
  });

  it("rejects missing and malformed expressions", () => {
    expect(javascriptLicenseFailures(inventoryWith(null), allowedLicenses)[0]).toContain("missing");
    expect(
      javascriptLicenseFailures(inventoryWith("MIT/Apache-2.0"), allowedLicenses)[0],
    ).toContain("invalid");
  });
});
