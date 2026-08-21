#!/usr/bin/env node

import { spawnSync } from "node:child_process";

// Keep the export budget in source so the budget ratchet can enforce it.
const DEFAULT_MAX_ISSUES = 0;

function countIssues(report) {
  return report.issues.reduce((total, fileIssue) => {
    return (
      total +
      (fileIssue.exports?.length ?? 0) +
      (fileIssue.types?.length ?? 0) +
      (fileIssue.nsExports?.length ?? 0) +
      (fileIssue.nsTypes?.length ?? 0) +
      (fileIssue.enumMembers?.length ?? 0) +
      (fileIssue.namespaceMembers?.length ?? 0)
    );
  }, 0);
}

const maxIssues = DEFAULT_MAX_ISSUES;
const pnpmBin = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
// Keep pnpm status output out of the JSON stream.
const result = spawnSync(
  pnpmBin,
  ["--silent", "exec", "knip", "--exports", "--reporter", "json", "--no-config-hints"],
  { encoding: "utf8" },
);

if (result.error) {
  throw result.error;
}

// Knip emits a usable JSON report even when findings make it exit non-zero.
let report;
try {
  report = JSON.parse(result.stdout);
} catch (err) {
  process.stderr.write(
    `Could not parse knip JSON output (knip exited ${result.status}):\n${err instanceof Error ? err.message : String(err)}\n`,
  );
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  process.exit(result.status ?? 1);
}
const issueCount = countIssues(report);

if (issueCount > maxIssues) {
  const overBy = issueCount - maxIssues;
  console.error(
    `Unused export budget exceeded: ${issueCount} issues found, max is ${maxIssues} (+${overBy}).`,
  );
  // Print every offender, including findings that reproduce only in CI.
  for (const fileIssue of report.issues) {
    const named = [
      ...(fileIssue.exports ?? []),
      ...(fileIssue.types ?? []),
      ...(fileIssue.nsExports ?? []),
      ...(fileIssue.nsTypes ?? []),
      ...(fileIssue.enumMembers ?? []),
      ...(fileIssue.namespaceMembers ?? []),
    ];
    for (const sym of named) {
      const name = typeof sym === "string" ? sym : (sym?.name ?? JSON.stringify(sym));
      const kind = sym && typeof sym === "object" && sym.symbolType ? ` [${sym.symbolType}]` : "";
      console.error(`  ${fileIssue.file}: ${name}${kind}`);
    }
  }
  console.error("Remove unused exports/types or intentionally raise the ratchet.");
  process.exit(1);
}

console.log(`Unused export budget ok: ${issueCount}/${maxIssues} issues.`);
