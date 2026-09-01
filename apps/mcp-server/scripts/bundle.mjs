import { build } from "esbuild";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { assertRepositorySchemaContract } from "./lib/schema-contract.mjs";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = join(pkgRoot, "src");
const outDir = join(pkgRoot, "dist-bundle");
assertRepositorySchemaContract();
mkdirSync(outDir, { recursive: true });

await build({
  entryPoints: [join(srcDir, "index.ts")],
  outfile: join(outDir, "sitecmd-mcp.mjs"),
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node22",
  external: ["node:sqlite"],
  banner: {
    js: "// SiteCMD MCP server - generated bundle, do not edit. Source: apps/mcp-server/src.",
  },
});

for (const file of [
  "causal_graph.json",
  "fix_locations.json",
  "impact_score.json",
  "license_constants.json",
]) {
  copyFileSync(join(srcDir, file), join(outDir, file));
}

console.log("Bundled -> apps/mcp-server/dist-bundle/sitecmd-mcp.mjs (+ data JSON)");
