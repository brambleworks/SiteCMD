#!/usr/bin/env node
/**
 * Run CodeQL's security-extended suite over the JavaScript and TypeScript
 * sources before a push, so an alert that would block the pull request is seen
 * here instead of one CI round trip at a time.
 *
 * Only findings on lines this branch actually added are reported. CodeQL sees
 * the whole tree, so without that filter every pre-existing alert on main would
 * fail the gate for whoever touched the file next.
 *
 * Rust is out of scope on purpose: its analysis runs past twelve minutes in CI,
 * longer than the entire local gate, and every alert this gate was written for
 * was JavaScript.
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path, { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const LANGUAGE = "javascript-typescript";
const SUITE = "codeql/javascript-queries:codeql-suites/javascript-security-extended.qls";
const BASE_REF = process.env.SITECMD_CODEQL_BASE ?? "origin/main";

/** Inline escape hatch, matching how the rest of the guardrails are silenced. */
const ALLOW_MARKER = "codeql-allow:";

function run(command, args, { capture = false } = {}) {
  return spawnSync(command, args, {
    cwd: ROOT,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
    maxBuffer: 64 * 1024 * 1024,
  });
}

function die(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

/** Line ranges this branch added, keyed by repository-relative path. */
function addedLines() {
  const merged = run("git", ["merge-base", BASE_REF, "HEAD"], { capture: true });
  if (merged.status !== 0) die(`check-codeql: cannot resolve ${BASE_REF}; fetch it first.`);
  const base = merged.stdout.trim();
  const diff = run("git", ["diff", "-U0", `${base}..HEAD`], { capture: true });
  if (diff.status !== 0) die("check-codeql: git diff failed.");

  const ranges = new Map();
  let file = null;
  for (const line of diff.stdout.split("\n")) {
    if (line.startsWith("+++ b/")) {
      file = line.slice(6);
      continue;
    }
    if (!file || !line.startsWith("@@")) continue;
    // @@ -old,count +new,count @@
    const hunk = /^@@ -\S+ \+(\d+)(?:,(\d+))? @@/.exec(line);
    if (!hunk) continue;
    const start = Number(hunk[1]);
    const count = hunk[2] === undefined ? 1 : Number(hunk[2]);
    if (count === 0) continue;
    if (!ranges.has(file)) ranges.set(file, []);
    ranges.get(file).push([start, start + count - 1]);
  }
  return ranges;
}

function touched(ranges, file, line) {
  const spans = ranges.get(file);
  if (!spans) return false;
  return spans.some(([from, to]) => line >= from && line <= to);
}

/**
 * True when the flagged line, or either line above it, carries
 * `codeql-allow: <rule id>`. A finding dismissed on GitHub stays dismissed
 * there; this marker is how the same decision is recorded in the source, where
 * a reviewer reading the code can see it.
 */
function allowed(sources, file, line, rule) {
  if (!sources.has(file)) {
    try {
      sources.set(file, readFileSync(join(ROOT, file), "utf8").split("\n"));
    } catch {
      sources.set(file, []);
    }
  }
  const lines = sources.get(file);
  return lines
    .slice(Math.max(0, line - 3), line)
    .some((text) => text.includes(`${ALLOW_MARKER} ${rule}`));
}

function main() {
  if (run("codeql", ["version", "--format=terse"], { capture: true }).status !== 0)
    die(
      "check-codeql: the CodeQL CLI is not installed.\n" +
        "  Install it with:  brew install --cask codeql\n" +
        "  It is the same engine the CodeQL workflow runs, so this gate\n" +
        "  reports the alerts that would otherwise block the pull request.",
    );

  const ranges = addedLines();
  if (ranges.size === 0) {
    console.log("check-codeql: no added lines to analyze.");
    return;
  }

  const work = mkdtempSync(join(tmpdir(), "sitecmd-codeql-"));
  try {
    // No paths-ignore config: the extractor already skips node_modules, and
    // analysis covers every tracked JavaScript and TypeScript file.
    const database = join(work, "db");
    const created = run("codeql", [
      "database",
      "create",
      database,
      `--language=${LANGUAGE}`,
      "--build-mode=none",
      `--source-root=${ROOT}`,
      "--overwrite",
    ]);
    if (created.status !== 0) die("check-codeql: database creation failed.");

    const sarif = join(work, "results.sarif");
    const analyzed = run("codeql", [
      "database",
      "analyze",
      database,
      SUITE,
      "--format=sarif-latest",
      `--output=${sarif}`,
      "--download",
    ]);
    if (analyzed.status !== 0) die("check-codeql: analysis failed.");

    const report = JSON.parse(readFileSync(sarif, "utf8"));
    const rules = new Map();
    for (const run_ of report.runs ?? [])
      for (const rule of run_.tool?.driver?.rules ?? [])
        rules.set(rule.id, rule.properties?.["security-severity"]);

    const sources = new Map();
    const findings = [];
    for (const run_ of report.runs ?? [])
      for (const result of run_.results ?? []) {
        const location = result.locations?.[0]?.physicalLocation;
        const file = location?.artifactLocation?.uri;
        const line = location?.region?.startLine;
        if (!file || !line || !touched(ranges, file, line)) continue;
        if (allowed(sources, file, line, result.ruleId)) continue;
        findings.push({
          rule: result.ruleId,
          severity: rules.get(result.ruleId),
          file,
          line,
          message: result.message?.text ?? "",
        });
      }

    if (findings.length === 0) {
      console.log(`check-codeql: no new alerts across ${ranges.size} changed file(s).`);
      return;
    }

    findings.sort((a, b) => Number(b.severity ?? 0) - Number(a.severity ?? 0));
    process.stderr.write(`check-codeql: ${findings.length} new alert(s) on added lines.\n\n`);
    for (const finding of findings)
      process.stderr.write(
        `  ${finding.file}:${finding.line}\n` +
          `    ${finding.rule}${finding.severity ? ` (severity ${finding.severity})` : ""}\n` +
          `    ${finding.message}\n\n`,
      );
    process.stderr.write(
      "These are the alerts the CodeQL check reports on the pull request.\n" +
        `Fix them, or record the decision in the source with a ${ALLOW_MARKER} <rule id> comment.\n`,
    );
    process.exit(1);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

main();
