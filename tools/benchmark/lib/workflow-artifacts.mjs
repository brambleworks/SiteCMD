import { closeSync, fstatSync, lstatSync, openSync, readFileSync, realpathSync } from "node:fs";
import path from "node:path";
import { requireCondition, requireText } from "./workflow-contract.mjs";
import { canonicalJson, digest } from "./workflow-plan.mjs";
import { validateResult } from "./workflow-results.mjs";

export function artifactPath(root, name, { directory = false } = {}) {
  requireCondition(
    typeof name === "string" && /^[a-zA-Z0-9][a-zA-Z0-9._/-]*$/.test(name),
    "invalid artifact path",
  );
  requireCondition(
    name.split("/").every((part) => part && part !== "." && part !== ".."),
    "artifact path traversal is not allowed",
  );
  let current = realpathSync(root);
  const parts = name.split("/");
  for (const [index, part] of parts.entries()) {
    current = path.join(current, part);
    const entry = lstatSync(current);
    requireCondition(!entry.isSymbolicLink(), "artifact symlinks are not allowed");
    const expectsDirectory = index < parts.length - 1 || directory;
    requireCondition(
      expectsDirectory ? entry.isDirectory() : entry.isFile(),
      "artifact must be a regular file or directory of the expected type",
    );
  }
  return current;
}

export function readArtifact(root, name) {
  const file = artifactPath(root, name);
  // Size the descriptor rather than the path, so the bytes measured and the
  // bytes read are the same inode even if the path is swapped underneath.
  const handle = openSync(file, "r");
  try {
    requireCondition(fstatSync(handle).size <= 64 * 1024 * 1024, "artifact exceeds 64 MiB");
    return readFileSync(handle);
  } finally {
    closeSync(handle);
  }
}

export function sameEvidence(actual, expected, label) {
  requireCondition(
    canonicalJson(actual) === canonicalJson(expected),
    `${label} disagrees with its receipt`,
  );
}

/** Check receipt consistency and hash referenced bytes without executing target code. */
export function collectEvidence(record, assignment, plan, root) {
  validateResult(record, assignment, plan.study);
  requireCondition(record.studySha256 === plan.studySha256, "trial belongs to a different study");
  const artifacts = new Map();
  let size = 0;
  const read = (name) => {
    if (!artifacts.has(name)) {
      const bytes = readArtifact(root, name);
      size += bytes.length;
      requireCondition(size <= 256 * 1024 * 1024, "trial evidence exceeds 256 MiB");
      artifacts.set(name, bytes);
    }
    return artifacts.get(name);
  };
  const json = (name) => JSON.parse(read(name).toString("utf8"));
  requireCondition(read(record.transcript).length > 0, "transcript is empty");
  if (record.mcp) requireCondition(read(record.mcp.trace).length > 0, "MCP trace is empty");
  const usageReceipt = json(record.usage.receipt);
  const { receipt: usagePath, ...usage } = record.usage;
  sameEvidence(usageReceipt.usage, usage, usagePath);
  requireText(usageReceipt.accountant, "usage accountant");
  requireText(usageReceipt.method, "usage accounting method");
  requireCondition(
    Array.isArray(usageReceipt.raw) && usageReceipt.raw.length > 0,
    "raw usage evidence is required, including for unavailable usage",
  );
  for (const name of usageReceipt.raw) read(name);
  const task = plan.study.tasks.find((item) => item.id === assignment.task);
  for (const submission of record.submissions) {
    requireCondition(
      digest(read(submission.patch)) === submission.patchSha256,
      "submitted patch digest does not match its bytes",
    );
    const grade = json(submission.receipt);
    for (const [key, value] of Object.entries({
      trialId: record.trialId,
      studySha256: plan.studySha256,
      sourceSha256: task.sourceSha256,
      patchSha256: submission.patchSha256,
      graderSha256: task.graderSha256,
    }))
      sameEvidence(grade[key], value, `grade ${key}`);
    requireText(grade.executor, "independent grader executor");
    requireText(grade.environment, "independent grader environment");
    for (const [checks, outcome] of [
      ["acceptance", "acceptancePass"],
      ["regressions", "regressionsPass"],
    ]) {
      requireCondition(
        Array.isArray(grade[checks]) && grade[checks].length > 0,
        `${checks} checks are required`,
      );
      for (const check of grade[checks]) {
        requireText(check.command, "grader check command");
        requireCondition(
          check.exitCode === null || Number.isInteger(check.exitCode),
          "check exitCode must be an integer or null on interruption",
        );
        requireCondition(read(check.log).length > 0, "grader log is empty");
      }
      sameEvidence(
        submission[outcome],
        grade[checks].every((check) => check.exitCode === 0),
        checks,
      );
    }
    sameEvidence(submission.integrityPass, grade.integrity?.passed, "integrity check");
    requireText(grade.integrity?.reason, "integrity check reason");
  }
  for (const review of record.reviews) {
    const { receipt, ...decision } = review;
    sameEvidence(json(receipt), decision, "blinded review");
  }
  return artifacts;
}
