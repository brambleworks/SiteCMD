#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isTestSourceFile } from "./test-file-classification.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const RUST_SRC = path.join(ROOT, "apps/desktop/src-tauri/src");
const SKIP = new Set(["node_modules", "dist", "target", ".next"]);

function read(p) {
  return fs.readFileSync(p, "utf8");
}

function walk(dir, ext, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP.has(e.name) || e.name.startsWith(".")) continue;
    const abs = path.join(dir, e.name);
    if (e.isDirectory()) walk(abs, ext, out);
    else if (e.isFile() && path.extname(e.name) === ext) out.push(abs);
  }
  return out;
}

function findFunctions(text) {
  const lines = text.split("\n");
  const sig =
    /^(\s*)(?:pub(?:\s*\([^)]+\))?\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]+"\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[<(]/;
  const fns = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const m = sig.exec(line);
    if (!m) continue;
    if (line.includes(";") && !line.includes("{")) continue;
    let j = i;
    while (j < lines.length && !lines[j].includes("{")) j++;
    if (j >= lines.length) continue;
    let depth = 0;
    let started = false;
    let end = j;
    for (let k = j; k < lines.length; k++) {
      for (const ch of lines[k]) {
        if (ch === "{") {
          depth++;
          started = true;
        } else if (ch === "}") depth--;
      }
      if (started && depth === 0) {
        end = k;
        break;
      }
    }
    fns.push({ name: m[2], startLine: i + 1, endLine: end + 1, length: end - i + 1 });
  }
  return fns;
}

function parseArgs(argv) {
  const a = { json: false, top: 30, threshold: 80 };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--json") a.json = true;
    else if (argv[i] === "--top") a.top = Number(argv[++i]);
    else if (argv[i] === "--threshold") a.threshold = Number(argv[++i]);
  }
  return a;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const all = [];
  for (const file of walk(RUST_SRC, ".rs")) {
    const rel = path.relative(ROOT, file).replaceAll(path.sep, "/");
    if (isTestSourceFile(rel)) continue;
    const text = read(file);
    for (const fn of findFunctions(text)) {
      all.push({ file: rel, ...fn, isTest: false });
    }
  }
  all.sort((a, b) => b.length - a.length);
  const overThreshold = all.filter((f) => f.length > args.threshold);

  if (args.json) {
    process.stdout.write(
      JSON.stringify(
        {
          threshold: args.threshold,
          totalFunctions: all.length,
          overThreshold: overThreshold.length,
          topFunctions: all.slice(0, args.top),
        },
        null,
        2,
      ) + "\n",
    );
    return;
  }

  process.stdout.write(
    `Rust function-length report (non-test)\n=====================================\n\n`,
  );
  process.stdout.write(`Total functions scanned:       ${all.length}\n`);
  process.stdout.write(`Functions longer than ${args.threshold}:    ${overThreshold.length}\n\n`);
  process.stdout.write(`Top ${args.top} longest functions:\n`);
  for (const fn of all.slice(0, args.top)) {
    process.stdout.write(
      `  ${String(fn.length).padStart(4)}  ${fn.file}:${fn.startLine}  ${fn.name}\n`,
    );
  }
}

main();
