#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  liveRepositoryProtectionFailures,
  requiredCheckWorkflowFailures,
} from "./lib/repository-protection-rules.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const live = process.argv.slice(2).includes("--live");
const unknown = process.argv.slice(2).filter((argument) => argument !== "--live");
if (unknown.length > 0) {
  process.stderr.write(`Unknown argument: ${unknown[0]}\n`);
  process.exit(2);
}

const read = (relativePath) => fs.readFileSync(path.join(ROOT, relativePath), "utf8");
const listFiles = (dir, predicate) =>
  fs
    .readdirSync(path.join(ROOT, dir))
    .map((entry) => `${dir}/${entry}`)
    .filter(predicate);
const contract = JSON.parse(read(".github/repository-protection.json"));

const gh = (endpoint) =>
  JSON.parse(
    execFileSync("gh", ["api", endpoint], {
      cwd: ROOT,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }),
  );

const failures = requiredCheckWorkflowFailures(contract, read, listFiles);
if (live && failures.length === 0) {
  try {
    const repository = gh(`repos/${contract.repository}`);
    const rulesets = gh(`repos/${contract.repository}/rulesets`).map((ruleset) =>
      gh(`repos/${contract.repository}/rulesets/${ruleset.id}`),
    );
    failures.push(
      ...liveRepositoryProtectionFailures(contract, {
        privateVulnerabilityReporting: gh(
          `repos/${contract.repository}/private-vulnerability-reporting`,
        ).enabled,
        securityAndAnalysis: repository.security_and_analysis,
        rulesets,
      }),
    );
  } catch (error) {
    failures.push(`Could not read live GitHub protection settings: ${error.message}`);
  }
}

if (failures.length > 0) {
  process.stderr.write("Repository protection check failed:\n");
  for (const failure of failures) process.stderr.write(`- ${failure}\n`);
  process.exit(1);
}

process.stdout.write(
  live
    ? `Repository protection matches ${contract.repository}.\n`
    : "Repository protection contract names checks that run on every pull request.\n",
);
