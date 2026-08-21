#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { aggregate, renderMarkdown } from "./lib/report.mjs";

const rawPath = process.argv[2];
if (!rawPath) {
  console.error("usage: render-report.mjs <path/to/raw.json>");
  process.exit(1);
}
const raw = JSON.parse(readFileSync(rawPath, "utf8"));
const perArm = aggregate(raw.targets, raw.config.arms);
const md = renderMarkdown({
  perArm,
  arms: raw.config.arms,
  targets: raw.targets,
  config: raw.config,
  stamp: raw.stamp,
});
const out = path.join(path.dirname(rawPath), "report.md");
writeFileSync(out, md);
console.log(`Wrote ${out}\n`);
console.log(md);
