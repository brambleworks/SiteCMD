import { randomBytes } from "node:crypto";
import path from "node:path";
import { calibrationCases, caseFiles } from "./lib/calibration-cases.mjs";
import { guestCommand, workRoot } from "./lib/vm-guest.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";
import { writeNewJson } from "./lib/workflow-store.mjs";

const harness = deployHarness();
const id = randomBytes(12).toString("hex");
const item = calibrationCases.find((item) => item.id === "credentialed-cors");
const receipt = JSON.parse(
  guestCommand(
    [
      "sudo",
      "flock",
      "-n",
      "/run/sitecmd-benchmark-execution.lock",
      "node",
      `${harness.directory}/desktop-smoke.mjs`,
    ],
    {
      input: JSON.stringify({ id, item, files: caseFiles(item) }),
      capture: true,
      timeout: 300000,
    },
  ),
);
const file = path.join(workRoot, `desktop-smoke-${id}.json`);
writeNewJson(file, { ...receipt, agentInvoked: false, harnessSha256: harness.id });
console.log(
  `Desktop scan, real MCP fix brief, reference repair and verification passed. No model calls. Evidence: ${file}`,
);
