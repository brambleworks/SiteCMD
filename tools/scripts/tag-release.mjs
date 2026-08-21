#!/usr/bin/env node

import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { extractReleaseNotes } from "./check-changelog-notes.mjs";
import { VERSION_FILES } from "./lib/version-files.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const expectedVersion = process.argv.slice(2).find((argument) => argument !== "--");

function die(message) {
  console.error(`release:tag: ${message}`);
  process.exit(1);
}

function gitCapture(...args) {
  const result = spawnSync("git", args, {
    cwd: ROOT,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    const detail = result.stderr.trim() || result.stdout.trim() || `exit ${result.status}`;
    die(`git ${args[0]} failed: ${detail}`);
  }
  return result.stdout.trim();
}

const versions = VERSION_FILES.map((entry) => {
  const source = fs.readFileSync(path.join(ROOT, entry.file), "utf8");
  const version = entry.read(source);
  if (!version) die(`could not find the version in ${entry.file}`);
  return { file: entry.file, version };
});
const distinct = [...new Set(versions.map(({ version }) => version))];
if (distinct.length !== 1) {
  die(
    "version files are out of sync:\n" +
      versions.map(({ file, version }) => `  ${file} = ${version}`).join("\n"),
  );
}

const version = distinct[0];
if (expectedVersion && expectedVersion !== version && expectedVersion !== `v${version}`) {
  die(`requested ${expectedVersion}, but the protected source version is ${version}`);
}
const tag = `v${version}`;

const branch = gitCapture("rev-parse", "--abbrev-ref", "HEAD");
if (branch !== "main") die(`on branch "${branch}", not main`);
if (gitCapture("status", "--porcelain")) die("working tree is dirty");

const head = gitCapture("rev-parse", "HEAD");
const originMain = gitCapture("rev-parse", "--verify", "refs/remotes/origin/main");
if (head !== originMain) {
  die(
    "local main does not exactly match origin/main; fetch or pull, inspect the result, and retry",
  );
}
if (gitCapture("tag", "--list", tag)) die(`tag ${tag} already exists`);

let releaseNotes;
try {
  releaseNotes = extractReleaseNotes({
    source: fs.readFileSync(path.join(ROOT, "CHANGELOG.md"), "utf8"),
    version,
  });
} catch (error) {
  die(error.message);
}

try {
  execFileSync(
    "git",
    ["tag", "-s", "--cleanup=verbatim", "-m", `Release ${tag}`, "-m", releaseNotes, tag],
    { cwd: ROOT, stdio: "inherit" },
  );
} catch {
  die("signed tag creation failed; no commit or push was attempted");
}

console.log(`release:tag: created signed tag ${tag} on ${head.slice(0, 12)}.`);
console.log(`Verify it with: git tag --verify ${tag}`);
console.log(`Publishing is a separate explicit action: git push origin ${tag}`);
