#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(import.meta.dirname, "..", "..");
// main.tsx boots the shell through `import("./App")`, so the App chunk is
// loaded before any UI even though index.html never names it.
const BOOTSTRAP_IMPORT = 'import("./App")';
const BOOTSTRAP_CHUNK = /^App-[A-Za-z0-9_-]+\.js$/;
const EAGER_ASSET_RE = /(?:src|href)="[^"]*assets\/([A-Za-z0-9_.-]+\.(?:js|css))"/g;

/** A size-limit glob against one asset file name. Only `*` is used here. */
export function matchesAssetGlob(glob, file) {
  const pattern = glob.replace(/^dist\/assets\//, "");
  const rx = new RegExp(`^${pattern.replaceAll(".", "\\.").replaceAll("*", "[^/]*")}$`);
  return rx.test(file);
}

/** Eager assets the budget misses, and budgeted assets that are lazy. */
export function eagerBundleParityFailures({ html, mainSource, assets, budgetPaths }) {
  const failures = [];
  if (!mainSource.includes(BOOTSTRAP_IMPORT)) {
    failures.push(
      `apps/desktop/src/main.tsx no longer boots through ${BOOTSTRAP_IMPORT}; update BOOTSTRAP_CHUNK in check-eager-bundle-parity.mjs to whatever chunk main.tsx now loads before first paint.`,
    );
  }
  const eager = new Set([...html.matchAll(EAGER_ASSET_RE)].map((m) => m[1]));
  for (const file of assets) if (BOOTSTRAP_CHUNK.test(file)) eager.add(file);

  const includes = budgetPaths.filter((glob) => !glob.startsWith("!"));
  const excludes = budgetPaths.filter((glob) => glob.startsWith("!")).map((glob) => glob.slice(1));
  const covered = (file) =>
    includes.some((glob) => matchesAssetGlob(glob, file)) &&
    !excludes.some((glob) => matchesAssetGlob(glob, file));

  const uncounted = [...eager].filter((file) => !covered(file)).sort();
  const lazyCounted = assets.filter((file) => covered(file) && !eager.has(file)).sort();
  if (uncounted.length > 0) {
    failures.push(
      `Loaded before first UI paint, but the initial-page budget does not count them:\n  ${uncounted.join("\n  ")}`,
    );
  }
  if (lazyCounted.length > 0) {
    failures.push(
      `The initial-page budget counts these, but nothing loads them before first paint (they are lazy):\n  ${lazyCounted.join("\n  ")}`,
    );
  }
  return failures;
}

function main() {
  const dist = path.join(root, "apps/desktop/dist");
  const budgets = JSON.parse(
    readFileSync(path.join(root, "apps/desktop/.size-limit.json"), "utf8"),
  );
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
  const mainSource = readFileSync(path.join(root, "apps/desktop/src/main.tsx"), "utf8");
  const assets = readdirSync(path.join(dist, "assets")).filter(
    (file) => file.endsWith(".js") || file.endsWith(".css"),
  );

  const failures = eagerBundleParityFailures({
    html,
    mainSource,
    assets,
    budgetPaths: entry.path,
  });
  if (failures.length > 0) {
    console.error("Initial-page budget no longer measures the initial page:\n");
    console.error(failures.join("\n\n"));
    console.error(
      "\nUpdate the first entry's path list in apps/desktop/.size-limit.json to the assets loaded before first paint, and re-measure the limit against it.",
    );
    process.exit(1);
  }
  console.log("Eager-bundle parity passed.");
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
