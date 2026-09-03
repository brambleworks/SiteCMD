#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const root = path.join(import.meta.dirname, "..", "..");
// main.tsx boots the shell through `import("./App")`, so the App chunk is
// loaded before any UI even though index.html never names it. Everything the
// App chunk statically imports loads with it, so the eager graph is the
// transitive static closure of the assets index.html names plus that bootstrap
// chunk. index.html also hand-links files Vite copies from public/ unhashed
// (dist/boot.css), so seeds are dist-relative paths, not just assets/ names.
const BOOTSTRAP_IMPORT = 'import("./App")';
const BOOTSTRAP_CHUNK = /^assets\/App-[A-Za-z0-9_-]+\.js$/;
const HTML_ASSET_RE = /(?:src|href)="([^"]+)"/g;
// Static edges only. `import(` and `import.meta` cannot match: the lookahead
// after the keyword rejects `(` and `.`, so dynamically imported chunks and the
// `__vite__mapDeps` preload tables that name them stay off the eager graph.
const STATIC_FROM_RE = /(?:^|[\s;}])(?:import|export)(?=[\s{*])[^;]*?\bfrom\s*["']([^"']+)["']/g;
const STATIC_SIDE_EFFECT_RE = /(?:^|[\s;}])import\s*["']([^"']+)["']/g;
const CSS_IMPORT_RE = /@import[^;'"]*["']([^"']+)["']/g;
// size-limit measures `gzip: true` entries as the sum of each file gzipped at
// level 9, and reports kB as 1000 bytes.
const GZIP_LEVEL = 9;
const BYTES_PER_KB = 1000;

/** Is this a JS or CSS file the budget can measure? */
function isBudgetedAsset(file) {
  return file.endsWith(".js") || file.endsWith(".css");
}

/**
 * A reference from `fromFile` (a dist-relative path) as a dist-relative path,
 * or null when it points outside the build (an absolute URL, or an escape
 * above dist).
 */
export function resolveAssetPath(fromFile, reference) {
  if (reference.includes("://") || reference.startsWith("//")) return null;
  const cleaned = reference.split("?")[0].split("#")[0];
  if (cleaned === "") return null;
  const resolved = cleaned.startsWith("/")
    ? path.posix.normalize(cleaned.slice(1))
    : path.posix.normalize(path.posix.join(path.posix.dirname(fromFile), cleaned));
  return resolved.startsWith("..") ? null : resolved;
}

/** A size-limit glob against one dist-relative asset path. Only `*` is used here. */
export function matchesAssetGlob(glob, file) {
  const pattern = glob.replace(/^dist\//, "");
  const rx = new RegExp(`^${pattern.replaceAll(".", "\\.").replaceAll("*", "[^/]*")}$`);
  return rx.test(file);
}

/** Dist-relative assets index.html loads directly (scripts, preloads, stylesheets). */
export function htmlAssetSeeds(html) {
  const seeds = new Set();
  HTML_ASSET_RE.lastIndex = 0;
  let match;
  while ((match = HTML_ASSET_RE.exec(html)) !== null) {
    const resolved = resolveAssetPath("index.html", match[1]);
    if (resolved !== null && isBudgetedAsset(resolved)) seeds.add(resolved);
  }
  return [...seeds];
}

/** Dist-relative assets one built asset pulls in over a static import edge. */
export function staticAssetEdges({ file, source, assetPaths }) {
  const patterns = file.endsWith(".css")
    ? [CSS_IMPORT_RE]
    : [STATIC_FROM_RE, STATIC_SIDE_EFFECT_RE];
  const edges = new Set();
  for (const pattern of patterns) {
    pattern.lastIndex = 0;
    let match;
    while ((match = pattern.exec(source)) !== null) {
      const resolved = resolveAssetPath(file, match[1]);
      if (resolved !== null && assetPaths.has(resolved)) edges.add(resolved);
    }
  }
  return [...edges];
}

/**
 * Every asset loaded before first paint, sorted.
 *
 * `assets` maps a dist-relative asset path to its built source. Seeds are the
 * assets index.html names (script, modulepreload, stylesheet, and hand-linked
 * files copied from public/) plus the bootstrap App chunk; from there the walk
 * follows static import edges only.
 */
export function collectEagerAssets({ html, assets }) {
  const assetPaths = new Set(Object.keys(assets));
  const queue = htmlAssetSeeds(html);
  for (const file of assetPaths) if (BOOTSTRAP_CHUNK.test(file)) queue.push(file);

  const eager = new Set();
  while (queue.length > 0) {
    const file = queue.pop();
    if (eager.has(file) || !assetPaths.has(file)) continue;
    eager.add(file);
    queue.push(...staticAssetEdges({ file, source: assets[file], assetPaths }));
  }
  return [...eager].sort();
}

/** Eager assets the budget misses, and budgeted assets that are lazy. */
export function eagerBundleParityFailures({ html, mainSource, assets, budgetPaths }) {
  const failures = [];
  if (!mainSource.includes(BOOTSTRAP_IMPORT)) {
    failures.push(
      `apps/desktop/src/main.tsx no longer boots through ${BOOTSTRAP_IMPORT}; update BOOTSTRAP_CHUNK in check-eager-bundle-parity.mjs to whatever chunk main.tsx now loads before first paint.`,
    );
  }
  const eager = new Set(collectEagerAssets({ html, assets }));

  const includes = budgetPaths.filter((glob) => !glob.startsWith("!"));
  const excludes = budgetPaths.filter((glob) => glob.startsWith("!")).map((glob) => glob.slice(1));
  const covered = (file) =>
    includes.some((glob) => matchesAssetGlob(glob, file)) &&
    !excludes.some((glob) => matchesAssetGlob(glob, file));

  const uncounted = [...eager].filter((file) => !covered(file)).sort();
  const lazyCounted = Object.keys(assets)
    .filter((file) => covered(file) && !eager.has(file))
    .sort();
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

/** A size-limit `limit` string ("206 kB") as a byte count. */
export function parseSizeLimit(limit) {
  const match = /^\s*([\d.]+)\s*(b|kb|mb)\s*$/i.exec(limit);
  if (!match) return null;
  const scale = { b: 1, kb: BYTES_PER_KB, mb: BYTES_PER_KB * BYTES_PER_KB };
  return Number(match[1]) * scale[match[2].toLowerCase()];
}

function formatKb(bytes) {
  return `${(bytes / BYTES_PER_KB).toFixed(2)} kB`;
}

/**
 * Why the measured eager graph fails the first budget entry, if it does.
 *
 * A limit this gate cannot parse is a failure, not a skip: silently dropping
 * the size check would print a pass line for an unmeasured graph, which is the
 * false pass this gate exists to prevent.
 */
export function eagerSizeFailures({ limit, eagerBytes, eager }) {
  const limitBytes = parseSizeLimit(limit);
  if (limitBytes === null) {
    return [
      `The initial-page entry in apps/desktop/.size-limit.json has the limit ${JSON.stringify(limit ?? null)}, which this gate cannot parse, so the ${formatKb(eagerBytes)} eager graph was never checked against a budget. Write the limit as a number and a unit, such as "206 kB".`,
    ];
  }
  if (eagerBytes > limitBytes) {
    return [
      `The eager graph is ${formatKb(eagerBytes)} gzipped, over the ${limit} initial-page budget:\n  ${eager.join("\n  ")}`,
      "Defer code that first paint does not need instead of raising the budget: lazy routes, or a lazily loaded heavy component.",
    ];
  }
  return [];
}

/** Every JS and CSS file under dist, as dist-relative paths. */
function readDistAssets(dist) {
  return Object.fromEntries(
    readdirSync(dist, { recursive: true, withFileTypes: true })
      .filter((entry) => entry.isFile() && isBudgetedAsset(entry.name))
      .map((entry) => {
        const file = path.relative(dist, path.join(entry.parentPath, entry.name));
        return [file.split(path.sep).join("/"), readFileSync(path.join(dist, file), "utf8")];
      }),
  );
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
  const assets = readDistAssets(dist);

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

  const eager = collectEagerAssets({ html, assets });
  const eagerBytes = eager.reduce(
    (total, file) =>
      total + gzipSync(readFileSync(path.join(dist, file)), { level: GZIP_LEVEL }).length,
    0,
  );
  const sizeFailures = eagerSizeFailures({ limit: entry.limit, eagerBytes, eager });
  if (sizeFailures.length > 0) {
    console.error(sizeFailures.join("\n\n"));
    process.exit(1);
  }
  console.log(
    `Eager-bundle parity passed: ${eager.length} assets, ${formatKb(eagerBytes)} gzipped against a ${entry.limit} budget.`,
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
