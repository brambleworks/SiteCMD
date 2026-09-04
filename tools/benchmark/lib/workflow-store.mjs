import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { createPlan, validatePlan, digest } from "./workflow-plan.mjs";
import {
  artifactPath,
  collectEvidence,
  readArtifact,
  sameEvidence,
} from "./workflow-artifacts.mjs";
import { requireCondition } from "./workflow-contract.mjs";
import { validateResult } from "./workflow-results.mjs";

export function writeNewJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx", mode: 0o600 });
}

export function createStudyRun(study, directory) {
  const plan = createPlan(study);
  mkdirSync(directory, { mode: 0o700 });
  mkdirSync(path.join(directory, "trials"), { mode: 0o700 });
  writeNewJson(path.join(directory, "plan.json"), plan);
  return plan;
}

export function loadPlan(directory) {
  return validatePlan(JSON.parse(readArtifact(directory, "plan.json").toString("utf8")));
}

export function importTrial(directory, input) {
  const plan = loadPlan(directory);
  const record = JSON.parse(readFileSync(input, "utf8"));
  const assignment = plan.assignments.find((item) => item.id === record.trialId);
  requireCondition(assignment, "unknown trial id");
  const artifacts = collectEvidence(record, assignment, plan, path.dirname(input));
  const destination = path.join(
    artifactPath(directory, "trials", { directory: true }),
    assignment.id,
  );
  mkdirSync(destination, { mode: 0o700 });
  mkdirSync(path.join(destination, "reviews"), { mode: 0o700 });
  for (const [name, bytes] of artifacts) {
    const file = path.join(destination, "artifacts", name);
    mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
    writeFileSync(file, bytes, { flag: "wx", mode: 0o600 });
  }
  const hashes = Object.fromEntries([...artifacts].map(([name, bytes]) => [name, digest(bytes)]));
  writeNewJson(path.join(destination, "record.json"), {
    record,
    recordSha256: digest(record),
    artifacts: hashes,
  });
  return assignment.id;
}

export function loadTrial(directory, plan, assignment) {
  const trialRoot = artifactPath(directory, `trials/${assignment.id}`, { directory: true });
  const stored = JSON.parse(readArtifact(trialRoot, "record.json").toString("utf8"));
  requireCondition(digest(stored.record) === stored.recordSha256, "trial record was modified");
  const artifacts = collectEvidence(
    stored.record,
    assignment,
    plan,
    artifactPath(trialRoot, "artifacts", { directory: true }),
  );
  sameEvidence(
    Object.fromEntries([...artifacts].map(([name, bytes]) => [name, digest(bytes)])),
    stored.artifacts,
    "artifact digests",
  );
  const record = stored.record;
  for (const name of readdirSync(artifactPath(trialRoot, "reviews", { directory: true })).sort()) {
    requireCondition(/^[a-f0-9]{64}\.json$/.test(name), "unexpected review file");
    const review = JSON.parse(readArtifact(trialRoot, `reviews/${name}`).toString("utf8"));
    requireCondition(digest(review) === name.slice(0, -5), "review receipt was modified");
    record.reviews.push({ ...review, receipt: `reviews/${name}` });
  }
  return validateResult(record, assignment, plan.study);
}

export function loadResults(directory, plan = loadPlan(directory)) {
  const assignments = new Map(plan.assignments.map((item) => [item.id, item]));
  return readdirSync(artifactPath(directory, "trials", { directory: true }))
    .sort()
    .map((name) => {
      requireCondition(assignments.has(name), `unknown stored trial ${name}`);
      return loadTrial(directory, plan, assignments.get(name));
    });
}

export function appendReview(directory, trialId, review) {
  const plan = loadPlan(directory);
  const assignment = plan.assignments.find((item) => item.id === trialId);
  requireCondition(assignment, "unknown trial id");
  const record = loadTrial(directory, plan, assignment);
  const name = `${digest(review)}.json`;
  requireCondition(!Object.hasOwn(review, "receipt"), "review input must not set a receipt path");
  record.reviews.push({ ...review, receipt: `reviews/${name}` });
  validateResult(record, assignment, plan.study);
  const reviewRoot = artifactPath(directory, `trials/${trialId}/reviews`, { directory: true });
  writeNewJson(path.join(reviewRoot, name), review);
}
