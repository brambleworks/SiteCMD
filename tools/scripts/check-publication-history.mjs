#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  candidateHistoryShapeFailures,
  publicationHistoryPathFailures,
} from "./lib/publication-history-rules.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const arguments_ = process.argv.slice(2).filter((argument) => argument !== "--");
if (arguments_.length > 1) {
  process.stderr.write(
    "Publication history check failed: use --all, --candidate-main, or one Git ref.\n",
  );
  process.exit(2);
}
const requestedScope = arguments_[0] ?? "--all";
const scanAllRefs = requestedScope === "--all";
const candidateMain = requestedScope === "--candidate-main";
const requestedRef = candidateMain ? "refs/heads/main" : requestedScope;

function git(args) {
  const result = spawnSync("git", args, {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    const detail = result.stderr.trim() || result.stdout.trim() || `exit ${result.status}`;
    throw new Error(`git ${args[0]} failed: ${detail}`);
  }
  return result.stdout;
}

let commit;
try {
  commit = scanAllRefs ? null : git(["rev-parse", "--verify", `${requestedRef}^{commit}`]).trim();
} catch (error) {
  process.stderr.write(`Publication history check failed: ${error.message}\n`);
  process.exit(2);
}

const revisionArgs = scanAllRefs ? ["--all", "HEAD"] : [commit];
const paths = git(["log", "--format=", "--name-only", ...revisionArgs])
  .split("\n")
  .map((value) => value.trim())
  .filter(Boolean);
const pathFailures = publicationHistoryPathFailures(paths);
const shapeFailures = candidateMain
  ? candidateHistoryShapeFailures(
      git(["rev-list", "--count", commit]).trim(),
      git(["rev-list", "--parents", "--max-count=1", commit]).trim(),
    )
  : [];

const gitleaks = spawnSync(
  "gitleaks",
  [
    "git",
    "--redact",
    "--no-banner",
    "--config=.gitleaks.toml",
    `--log-opts=${scanAllRefs ? "--all HEAD" : commit}`,
  ],
  {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  },
);

if (gitleaks.error?.code === "ENOENT") {
  process.stderr.write(
    "Publication history check failed: the Gitleaks CLI is required to scan every reachable commit. Install it as documented in README.md.\n",
  );
  process.exit(2);
}

const secretScanFailed = gitleaks.status !== 0;
if (pathFailures.length > 0 || shapeFailures.length > 0 || secretScanFailed) {
  const scope = scanAllRefs
    ? "all refs"
    : `${candidateMain ? "candidate main" : requestedRef} (${commit.slice(0, 12)})`;
  process.stderr.write(`Publication history failed for ${scope}:\n`);
  for (const failure of pathFailures) process.stderr.write(`- ${failure}\n`);
  for (const failure of shapeFailures) process.stderr.write(`- ${failure}\n`);
  if (secretScanFailed) {
    process.stderr.write("- gitleaks found secret-like material in reachable history\n");
    const redactedOutput = `${gitleaks.stdout ?? ""}${gitleaks.stderr ?? ""}`.trim();
    if (redactedOutput) process.stderr.write(`${redactedOutput}\n`);
  }
  process.exit(1);
}

const scope = scanAllRefs
  ? "all refs"
  : `${candidateMain ? "candidate main" : requestedRef} (${commit.slice(0, 12)})`;
process.stdout.write(
  `Publication history passed for ${scope} (${new Set(paths).size} historical paths checked).\n`,
);
