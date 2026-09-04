import { randomBytes } from "node:crypto";
import path from "node:path";
import { calibrationCases, caseFiles, caseIdentity } from "./lib/calibration-cases.mjs";
import { guestCommand, workRoot } from "./lib/vm-guest.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";
import { digest } from "./lib/workflow-plan.mjs";
import { writeNewJson } from "./lib/workflow-store.mjs";

const harness = deployHarness();
const id = randomBytes(16).toString("hex");
const output = `/srv/sitecmd-benchmark/validation/${id}`;
const cases = calibrationCases.map((item) => ({
  ...item,
  baselineFiles: caseFiles(item),
  referenceFiles: caseFiles(item, true),
  sourceSha256: caseIdentity(item),
  referenceSha256: caseIdentity(item, true),
}));
const receipt = JSON.parse(
  guestCommand(["sudo", "node", `${harness.directory}/validate-cases.mjs`], {
    input: JSON.stringify({ cases, output }),
    capture: true,
    timeout: 600000,
  }),
);
receipt.harnessSha256 = harness.id;
receipt.graderSha256 = digest(
  Object.fromEntries(
    [
      "calibration-grader.mjs",
      "candidate-sandbox.mjs",
      "node-candidate.mjs",
      "python-candidate.py",
    ].map((name) => [name, harness.files[`guest/${name}`]]),
  ),
);
receipt.corpusSha256 = digest(cases);
const file = path.join(workRoot, `calibration-grades-${id}.json`);
writeNewJson(file, receipt);
for (const result of receipt.results)
  console.log(
    `${result.passed ? "PASS" : "FAIL"} ${result.id}: three baseline/reference repetitions`,
  );
console.log(`Evidence: ${file}`);
if (receipt.results.some((item) => !item.passed)) process.exitCode = 1;
