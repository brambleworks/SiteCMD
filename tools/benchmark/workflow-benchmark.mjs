#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { parseArgs } from "node:util";
import {
  createStudyRun,
  importTrial,
  appendReview,
  loadPlan,
  loadResults,
} from "./lib/workflow-store.mjs";
import { analyzeStudy, renderWorkflowReport } from "./lib/workflow-report.mjs";
import { runFixture } from "./lib/workflow-fixture.mjs";
import { pilotPolicy, validatePilotStudy } from "./lib/workflow-pilot.mjs";
import { evaluateQuota } from "./lib/workflow-quota.mjs";
import { probeAgentAccounts } from "./lib/workflow-preflight.mjs";

const HELP = `Usage: pnpm benchmark <command> [options]

  pilot                              Print the subscription pilot policy (no trials)
  doctor                             Check CLI versions/auth without making model calls
  quota --baseline <json> --current <json>
                                     Check both account readings against the pilot policy
  fixture --out <new-directory>       Exercise the pipeline without agents or paid calls
  plan --study <json> --out <new-dir>  Freeze a study and randomized paired assignments
       [--pilot]                     Require the approved thirty-trial subscription policy
  record --run <directory> --input <trial.json>
                                     Import a trial and its evidence without overwriting
  review --run <directory> --trial <id> --input <review.json>
                                     Append a blinded patch review
  report --run <directory> [--json]   Validate evidence and print the current report

This tool does not launch agents, authorize spending, or grade untrusted code.
See docs/qa/agent-workflow-benchmark.md and tools/benchmark/README.md.`;

const COMMAND_OPTIONS = {
  pilot: [],
  doctor: [],
  quota: ["baseline", "current"],
  fixture: ["out"],
  plan: ["study", "out", "pilot"],
  record: ["run", "input"],
  review: ["run", "trial", "input"],
  report: ["run", "json"],
};
const BOOLEAN_OPTIONS = new Set(["json", "pilot"]);

function main(argv) {
  const [command, ...args] = argv;
  if (!command || command === "--help") return console.log(HELP);
  const allowed = COMMAND_OPTIONS[command];
  if (!allowed) throw new Error(`Unknown command: ${command}`);
  const { values } = parseArgs({
    args,
    options: Object.fromEntries(
      allowed.map((name) => [name, { type: BOOLEAN_OPTIONS.has(name) ? "boolean" : "string" }]),
    ),
  });
  for (const name of allowed.filter((name) => !BOOLEAN_OPTIONS.has(name))) {
    if (!values[name]?.trim()) throw new Error(`--${name} is required`);
  }
  const json = (file) => JSON.parse(readFileSync(file, "utf8"));
  if (command === "pilot") {
    console.log(JSON.stringify(pilotPolicy, null, 2));
  } else if (command === "doctor") {
    const result = probeAgentAccounts();
    console.log(JSON.stringify(result, null, 2));
    if (!result.readyToRun) process.exitCode = 2;
  } else if (command === "quota") {
    const result = evaluateQuota(json(values.baseline), json(values.current), pilotPolicy.billing);
    console.log(JSON.stringify(result, null, 2));
    if (!result.quotaAllowed) process.exitCode = 2;
  } else if (command === "plan") {
    const study = json(values.study);
    if (values.pilot) validatePilotStudy(study);
    const plan = createStudyRun(study, values.out);
    console.log(
      `Frozen ${plan.plannedTrials} assignments in ${values.out}. Maximum per-trial-cap exposure: $${plan.maximumConfiguredSpendUsd}; study stop cap: $${plan.study.limits.studyCostUsd}. No spending authorized or performed.`,
    );
    if (study.billing?.mode === "subscription")
      console.log(
        "Dollar caps cover additional charges only. Check account quota before each trial; this command does not enforce runtime limits.",
      );
  } else if (command === "record") {
    console.log(`Recorded ${importTrial(values.run, values.input)}`);
  } else if (command === "review") {
    appendReview(values.run, values.trial, json(values.input));
    console.log(`Appended review for ${values.trial}`);
  } else {
    const directory = command === "fixture" ? values.out : values.run;
    const plan = command === "fixture" ? runFixture(directory) : loadPlan(directory);
    const analysis = analyzeStudy(plan, loadResults(directory, plan));
    console.log(
      values.json ? JSON.stringify(analysis, null, 2) : renderWorkflowReport(plan, analysis),
    );
  }
}

try {
  main(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
