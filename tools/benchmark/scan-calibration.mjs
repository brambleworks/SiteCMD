import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { calibrationCases, caseFiles, caseIdentity } from "./lib/calibration-cases.mjs";
import { guestCommand, workRoot } from "./lib/vm-guest.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";
import { sourceSnapshot } from "./lib/vm-source.mjs";
import { writeNewJson } from "./lib/workflow-store.mjs";

const snapshot = sourceSnapshot(process.cwd());
const product = JSON.parse(readFileSync(path.join(workRoot, `product-${snapshot.commit}.json`)));
const harness = deployHarness();
const cases = calibrationCases.map((item) => ({
  ...item,
  baselineFiles: caseFiles(item),
  referenceFiles: caseFiles(item, true),
}));
const results = JSON.parse(
  guestCommand(["sudo", "node", `${harness.directory}/scan-cases.mjs`], {
    input: JSON.stringify({ cases, product }),
    capture: true,
    timeout: 1200000,
  }),
);
const file = path.join(workRoot, `calibration-scans-${randomBytes(16).toString("hex")}.json`);
writeNewJson(file, {
  product,
  sources: Object.fromEntries(calibrationCases.map((item) => [item.id, caseIdentity(item)])),
  results,
});
for (const item of results)
  console.log(
    `${item.id}: ${item.baseline.report.issues.map((issue) => issue.checkId ?? issue.id).join(", ")}`,
  );
console.log(`Evidence: ${file}`);
