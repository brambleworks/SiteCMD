#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const root = path.join(import.meta.dirname, "..", "..");
const dist = path.join(root, "apps/desktop/dist");
const budgets = JSON.parse(readFileSync(path.join(root, "apps/desktop/.size-limit.json"), "utf8"));

const entry = budgets[0];
if (!entry || !Array.isArray(entry.path)) {
  console.error(
    "The first .size-limit.json entry must be the initial-page budget with a path list.",
  );
  process.exit(1);
}

let html;
try {
  html = readFileSync(path.join(dist, "index.html"), "utf8");
} catch {
  // size-limit reports a missing build; parity has nothing additional to check.
  console.log("Eager-bundle parity skipped: apps/desktop/dist/index.html is not built.");
  process.exit(0);
}

/** Every JS asset index.html references, in load order, deduplicated. */
const eager = new Set(
  [...html.matchAll(/(?:src|href)="[^"]*assets\/([A-Za-z0-9_.-]+\.js)"/g)].map((m) => m[1]),
);

const includes = entry.path.filter((glob) => !glob.startsWith("!"));
const excludes = entry.path.filter((glob) => glob.startsWith("!")).map((glob) => glob.slice(1));

/** A size-limit glob against one asset file name. Only `*` is used here. */
function matches(glob, file) {
  const pattern = glob.replace(/^dist\/assets\//, "");
  const rx = new RegExp(`^${pattern.replaceAll(".", "\\.").replaceAll("*", "[^/]*")}$`);
  return rx.test(file);
}

const covered = (file) =>
  includes.some((glob) => matches(glob, file)) && !excludes.some((glob) => matches(glob, file));

const assets = readdirSync(path.join(dist, "assets")).filter((file) => file.endsWith(".js"));

const uncounted = [...eager].filter((file) => !covered(file)).sort();
const lazyCounted = assets.filter((file) => covered(file) && !eager.has(file)).sort();

const failures = [];
if (uncounted.length > 0) {
  failures.push(
    `index.html loads these before first paint, but the initial-page budget does not count them:\n  ${uncounted.join("\n  ")}`,
  );
}
if (lazyCounted.length > 0) {
  failures.push(
    `The initial-page budget counts these, but index.html never loads them (they are lazy):\n  ${lazyCounted.join("\n  ")}`,
  );
}

if (failures.length > 0) {
  console.error("Initial-page budget no longer measures the initial page:\n");
  console.error(failures.join("\n\n"));
  console.error(
    "\nUpdate the first entry's path list in apps/desktop/.size-limit.json to the chunks index.html actually loads, and re-measure the limit against it.",
  );
  process.exit(1);
}

console.log(`Eager-bundle parity passed (${eager.size} chunks counted).`);
