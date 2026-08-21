#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  liveRepositoryLabelFailures,
  repositoryLabelFailures,
} from "./lib/repository-label-rules.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const live = process.argv.slice(2).includes("--live");
const unknown = process.argv.slice(2).filter((argument) => argument !== "--live");
if (unknown.length > 0) {
  process.stderr.write(`Unknown argument: ${unknown[0]}\n`);
  process.exit(2);
}

const read = (relativePath) => fs.readFileSync(path.join(ROOT, relativePath), "utf8");
const contract = JSON.parse(read(".github/repository-labels.json"));
const issueTemplateDir = path.join(ROOT, ".github/ISSUE_TEMPLATE");
const yamlSources = [read(".github/dependabot.yml")];
for (const entry of fs.readdirSync(issueTemplateDir).sort()) {
  if (entry.endsWith(".yml") || entry.endsWith(".yaml")) {
    yamlSources.push(fs.readFileSync(path.join(issueTemplateDir, entry), "utf8"));
  }
}

const failures = repositoryLabelFailures(contract, read("renovate.json"), yamlSources);
if (live && failures.length === 0) {
  try {
    const output = execFileSync(
      "gh",
      [
        "label",
        "list",
        "--repo",
        contract.repository,
        "--limit",
        "1000",
        "--json",
        "name,color,description",
      ],
      { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    );
    failures.push(...liveRepositoryLabelFailures(contract, JSON.parse(output)));
  } catch (error) {
    failures.push(`Could not read live GitHub labels: ${error.message}`);
  }
}

if (failures.length > 0) {
  process.stderr.write("Repository label check failed:\n");
  for (const failure of failures) process.stderr.write(`- ${failure}\n`);
  process.exit(1);
}

process.stdout.write(
  live
    ? `Repository label contract matches ${contract.repository}.\n`
    : "Repository label contract covers configured automation.\n",
);
