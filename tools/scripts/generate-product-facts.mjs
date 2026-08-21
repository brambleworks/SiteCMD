#!/usr/bin/env node

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { PRODUCT_FACTS_FILE, productFacts } from "./lib/product-facts.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const read = (file) => readFileSync(join(ROOT, file), "utf8");

function listFiles(dir, predicate, files = []) {
  for (const entry of readdirSync(join(ROOT, dir), { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === "target") continue;
    const relative = `${dir}/${entry.name}`;
    if (entry.isDirectory()) listFiles(relative, predicate, files);
    else if (predicate(relative)) files.push(relative);
  }
  return files;
}

const facts = productFacts(read, listFiles);
const target = join(ROOT, PRODUCT_FACTS_FILE);
writeFileSync(target, `${JSON.stringify(facts, null, 2)}\n`);
console.log(`Wrote ${PRODUCT_FACTS_FILE}:`);
console.log(JSON.stringify(facts, null, 2));
