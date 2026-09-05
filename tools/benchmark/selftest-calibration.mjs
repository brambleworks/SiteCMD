import { randomBytes } from "node:crypto";
import path from "node:path";
import { mkdirSync } from "node:fs";
import { calibrationCases, caseFiles, caseIdentity } from "./lib/calibration-cases.mjs";
import { guestCommand, workRoot } from "./lib/vm-guest.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";
import { createStudyRun } from "./lib/workflow-store.mjs";
import { fixtureStudy } from "./lib/workflow-fixture.mjs";
import { pilotPolicy } from "./lib/workflow-pilot.mjs";
import { digest } from "./lib/workflow-plan.mjs";
import { exportGuestTrial } from "./lib/vm-trial-export.mjs";

const harness = deployHarness();
process.stdout.write(
  guestCommand(
    [
      "sudo",
      "flock",
      "-n",
      "/run/sitecmd-benchmark-execution.lock",
      "node",
      `${harness.directory}/sandbox-selftest.mjs`,
    ],
    { capture: true, timeout: 75000 },
  ),
);
const item = calibrationCases.find((item) => item.id === "credentialed-cors");
for (const mode of ["repair", "timeout"]) {
  const study = fixtureStudy();
  study.id = `executor-fixture-${randomBytes(8).toString("hex")}`;
  study.billing = pilotPolicy.billing;
  study.limits = { ...pilotPolicy.limits, trialSeconds: mode === "timeout" ? 1 : 60 };
  study.configurations = [
    {
      id: "scripted",
      agent: "codex",
      agentVersion: "fixture-process",
      model: "fixture-model",
      reasoning: "none",
      environment: "isolated guest; scripted process, no model calls",
    },
  ];
  study.tasks = [
    {
      ...study.tasks[0],
      id: item.id,
      sourceSha256: caseIdentity(item),
      referenceSha256: caseIdentity(item, true),
      graderSha256: digest(harness.files["guest/calibration-grader.mjs"]),
    },
  ];
  const run = path.join(workRoot, study.id);
  const plan = createStudyRun(study, run);
  mkdirSync(path.join(run, "inputs"), { mode: 0o700 });
  const assignment = plan.assignments.find((item) => item.arm === "normal");
  const result = JSON.parse(
    guestCommand(
      [
        "sudo",
        "flock",
        "-n",
        "/run/sitecmd-benchmark-execution.lock",
        "node",
        `${harness.directory}/executor-selftest.mjs`,
      ],
      {
        input: JSON.stringify({
          plan,
          assignment,
          item,
          mode,
          files: caseFiles(item),
          reference: item.reference,
        }),
        capture: true,
        timeout: 120000,
      },
    ),
  );
  exportGuestTrial(run, plan, assignment, harness);
  console.log(
    `PASS ${mode}: actual guest supervisor, sandboxed grading and evidence import; synthetic client only (${result.status})`,
  );
}
