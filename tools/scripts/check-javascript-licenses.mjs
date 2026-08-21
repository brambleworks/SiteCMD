#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  allowedLicensesFromCargoDeny,
  javascriptLicenseFailures,
} from "./lib/javascript-license-policy.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const inventory = JSON.parse(
  readFileSync(path.join(ROOT, "THIRD_PARTY_DEPENDENCIES.json"), "utf8"),
);
const allowedLicenses = allowedLicensesFromCargoDeny(
  readFileSync(path.join(ROOT, "apps/desktop/src-tauri/deny.toml"), "utf8"),
);
const failures = javascriptLicenseFailures(inventory, allowedLicenses);

if (failures.length > 0) {
  console.error("JavaScript dependency license policy failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const packageCount = inventory.packages.filter((entry) => entry.ecosystem === "npm").length;
console.log(
  `JavaScript license policy passed (${packageCount} npm packages, ${allowedLicenses.size} allowed SPDX choices).`,
);
