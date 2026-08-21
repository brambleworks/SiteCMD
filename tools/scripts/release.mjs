#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { formatLocalReleaseDate, prepareChangelogRelease } from "./check-changelog-notes.mjs";
import { assertBumpAllowed } from "./check-release-bump.mjs";
import { VERSION_FILES } from "./lib/version-files.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const CHANGELOG_FILE = "CHANGELOG.md";

const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith("--")));

// Exclude the force-patch reason from positional argument parsing.
const forcePatchInline = args.find((a) => a.startsWith("--force-patch="));
const forcePatchIndex = args.indexOf("--force-patch");
const forcePatch = forcePatchInline
  ? forcePatchInline.slice("--force-patch=".length)
  : forcePatchIndex >= 0
    ? args[forcePatchIndex + 1]
    : undefined;
const reasonIndex = forcePatchInline || forcePatchIndex < 0 ? -1 : forcePatchIndex + 1;

const bumpArg = args.find((a, i) => !a.startsWith("--") && i !== reasonIndex);
const dryRun = flags.has("--dry-run");
const knownFlags = new Set(["--dry-run", "--force-patch"]);
const unknownFlag = [...flags].find(
  (flag) => !knownFlags.has(flag) && !flag.startsWith("--force-patch="),
);

function die(message) {
  console.error(`release: ${message}`);
  process.exit(1);
}

if (!bumpArg) {
  die("usage: pnpm release <patch|minor|major|X.Y.Z> [--dry-run]");
}
if (unknownFlag) die(`unknown option "${unknownFlag}"`);
if (forcePatchIndex >= 0 && (!forcePatch || forcePatch.startsWith("--"))) {
  die("--force-patch requires a quoted reason");
}

const FILES = VERSION_FILES;

const readFile = (rel) => fs.readFileSync(path.join(ROOT, rel), "utf8");

// Refuse to release from an already inconsistent version set.
const sources = FILES.map((entry) => {
  const source = readFile(entry.file);
  const version = entry.read(source);
  if (!version) die(`could not find the version in ${entry.file}`);
  return { ...entry, source, version };
});
const distinct = [...new Set(sources.map((e) => e.version))];
if (distinct.length !== 1) {
  die(
    "version files are out of sync; fix by hand before releasing:\n" +
      sources.map((e) => `  ${e.file} = ${e.version}`).join("\n"),
  );
}
const currentVersion = distinct[0];

function nextVersion(from, arg) {
  if (/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(arg)) return arg;
  const core = from.match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!core) die(`cannot bump non-semver current version "${from}"`);
  let [major, minor, patch] = core.slice(1, 4).map(Number);
  if (arg === "major") return `${major + 1}.0.0`;
  if (arg === "minor") return `${major}.${minor + 1}.0`;
  if (arg === "patch") return `${major}.${minor}.${patch + 1}`;
  die(`unknown bump "${arg}" (expected patch|minor|major|X.Y.Z)`);
  return undefined;
}

const version = nextVersion(currentVersion, bumpArg);
if (version === currentVersion) die(`already at ${version}; nothing to bump`);
const tag = `v${version}`;

console.log(`release: ${currentVersion} -> ${version}  (tag ${tag})`);

const gitCapture = (...a) => execFileSync("git", a, { cwd: ROOT, encoding: "utf8" }).trim();

const branch = gitCapture("rev-parse", "--abbrev-ref", "HEAD");
if (branch === "main" || !branch.startsWith("release/")) {
  die(`on branch "${branch}"; release preparation requires a clean release/* branch`);
}
if (gitCapture("status", "--porcelain")) {
  die("working tree is dirty; commit or stash existing changes before preparing the release");
}
const tagExists = execFileSync("git", ["tag", "--list", tag], {
  cwd: ROOT,
  encoding: "utf8",
}).trim();
if (tagExists) die(`tag ${tag} already exists`);

// Validate the requested bump before any release file changes.
let patchOverride;
try {
  const verdict = assertBumpAllowed({ currentVersion, nextVersion: version, forcePatch });
  patchOverride = verdict.forced;
  if (patchOverride) {
    console.log(`release: patch override recorded - ${patchOverride}`);
  }
} catch (error) {
  die(error.message);
}

// Prepare the shared changelog and tag body before writing release files.
let changelogRelease;
try {
  changelogRelease = prepareChangelogRelease({
    source: readFile(CHANGELOG_FILE),
    version,
    releaseDate: formatLocalReleaseDate(new Date()),
  });
} catch (error) {
  die(error.message);
}
for (const entry of sources) {
  const updated = entry.write(entry.source, version);
  if (updated === entry.source || !updated.includes(`"${version}"`)) {
    die(`failed to rewrite the version in ${entry.file}`);
  }
  if (dryRun) {
    console.log(`  would update ${entry.file}`);
  } else {
    fs.writeFileSync(path.join(ROOT, entry.file), updated);
    console.log(`  updated ${entry.file}`);
  }
}

if (dryRun) {
  console.log(`  would update ${CHANGELOG_FILE}`);
} else {
  fs.writeFileSync(path.join(ROOT, CHANGELOG_FILE), changelogRelease.source);
  console.log(`  updated ${CHANGELOG_FILE}`);
}

if (dryRun) {
  console.log(`release: dry run, no files written. Would prepare the ${tag} release PR diff.`);
  process.exit(0);
}

console.log("");
console.log(`release: prepared the ${tag} release pull-request diff.`);
if (patchOverride) {
  console.log(`Record this patch override in the pull request: ${patchOverride}`);
}
console.log("Review the diff, commit it, and merge it through the protected pull-request path.");
console.log("After updating local main, run: pnpm release:tag");
