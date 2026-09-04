import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { digest } from "./lib/workflow-plan.mjs";
import { evaluateQuota } from "./lib/workflow-quota.mjs";
import { validatePilotStudy } from "./lib/workflow-pilot.mjs";
import { loadPlan, loadResults } from "./lib/workflow-store.mjs";
import { exportGuestTrial } from "./lib/vm-trial-export.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";
import { guestCommand, guestProcess } from "./lib/vm-guest.mjs";

const supplied = process.argv[2];
if (!supplied) throw new Error("Usage: run-next.mjs RUN_DIRECTORY");
const run = path.resolve(supplied);
const plan = loadPlan(run);
validatePilotStudy(plan.study);
const recorded = new Set(loadResults(run, plan).map((record) => record.trialId));
const assignment = plan.assignments.find((item) => !recorded.has(item.id));
if (!assignment) {
  console.log("All assignments are recorded; independent review is next.");
  process.exit(0);
}
const baseline = JSON.parse(readFileSync(path.join(run, "quota-baseline.json")));
const currentPath = path.join(run, "quota-current.json");
const current = JSON.parse(readFileSync(currentPath));
const quota = evaluateQuota(baseline, current, plan.study.billing);
if (!quota.quotaAllowed) throw new Error(quota.blockers.join("; "));
const baselineHashFile = path.join(run, "quota-baseline.sha256");
if (!existsSync(baselineHashFile))
  writeFileSync(baselineHashFile, digest(baseline), { flag: "wx", mode: 0o600 });
if (readFileSync(baselineHashFile, "utf8") !== digest(baseline))
  throw new Error("The original quota baseline changed; do not rebase the approved allowance");
const harness = deployHarness();
if (harness.id !== plan.study.runnerSha256)
  throw new Error("Runner changed after registration; prepare a new study before any trials");
const corpus = JSON.parse(readFileSync(path.join(run, "inputs", "corpus.json")));
if (digest(corpus) !== plan.study.corpusSha256) throw new Error("Frozen corpus changed");
const item = corpus.find((item) => item.id === assignment.task);
const product = JSON.parse(readFileSync(path.join(run, "inputs", "product.json")));
if (digest(product) !== plan.study.productSha256) throw new Error("Frozen product receipt changed");
const report = readFileSync(path.join(run, "inputs", `${assignment.task}-report.json`), "utf8");
const task = plan.study.tasks.find((task) => task.id === assignment.task);
if (digest(report) !== task.reportSha256 || digest(item.baselineFiles) !== task.sourceSha256)
  throw new Error("Frozen input changed");
console.log(
  `Running ${assignment.task}, ${assignment.configuration}, ${assignment.arm}. Update quota-current.json from real readings before it becomes five minutes old.`,
);
const child = guestProcess(
  [
    "sudo",
    "flock",
    "-n",
    "/run/sitecmd-benchmark-execution.lock",
    "node",
    `${harness.directory}/run-trial.mjs`,
  ],
  JSON.stringify({
    plan,
    assignment,
    item: {
      id: item.id,
      entry: item.entry,
      runtime: item.runtime,
      rule: item.rule,
      kind: item.kind,
    },
    files: item.baselineFiles,
    product,
    report,
    baseline,
    current,
  }),
);
let output = "";
child.stdout.on("data", (chunk) => {
  output += chunk;
  if (output.length > 1024 * 1024) child.kill("SIGTERM");
});
child.stderr.pipe(process.stderr);
let previous = digest(current);
let syncing = false;
const timer = setInterval(() => {
  if (syncing) return;
  syncing = true;
  try {
    const snapshot = JSON.parse(readFileSync(currentPath));
    if (digest(snapshot) !== previous) {
      evaluateQuota(baseline, snapshot, plan.study.billing);
      guestCommand(["sudo", "node", `${harness.directory}/update-quota.mjs`], {
        input: JSON.stringify({
          directory: `/srv/sitecmd-benchmark/trials/${assignment.id}`,
          snapshot,
          billing: plan.study.billing,
        }),
        capture: true,
      });
      previous = digest(snapshot);
    }
  } catch (error) {
    console.error(`Quota update not accepted: ${error.message}`);
  } finally {
    syncing = false;
  }
}, 2000);
const interrupt = () => {
  try {
    guestCommand(["sudo", "node", `${harness.directory}/cancel-trial.mjs`, assignment.id], {
      capture: true,
    });
  } catch (error) {
    console.error(
      `Cancellation could not be confirmed: ${error.message}. Stop the dedicated VM if guest control is unavailable.`,
    );
  }
};
process.once("SIGINT", interrupt);
process.once("SIGTERM", interrupt);
const exitCode = await new Promise((resolve, reject) => {
  child.once("error", reject);
  child.once("close", resolve);
}).finally(() => {
  clearInterval(timer);
  process.removeListener("SIGINT", interrupt);
  process.removeListener("SIGTERM", interrupt);
});
if (exitCode !== 0)
  throw new Error(
    `Guest trial stopped with status ${exitCode}; retain its private evidence before retrying`,
  );
const outcome = JSON.parse(output);
exportGuestTrial(run, plan, assignment, harness);
console.log(
  `Recorded ${assignment.id}: ${outcome.record.status}. Independent reviews remain pending.`,
);
if (outcome.record.status !== "completed") process.exitCode = 2;
