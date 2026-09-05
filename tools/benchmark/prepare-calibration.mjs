import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { calibrationCases, caseFiles, caseIdentity } from "./lib/calibration-cases.mjs";
import { digest } from "./lib/workflow-plan.mjs";
import { pilotPolicy, validatePilotStudy } from "./lib/workflow-pilot.mjs";
import { agentVersions, reasoning } from "./lib/trial-invocation.mjs";
import { createStudyRun, writeNewJson } from "./lib/workflow-store.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";

const [gradesPath, scansPath, output] = process.argv.slice(2);
if (!gradesPath || !scansPath || !output)
  throw new Error("Usage: prepare-calibration.mjs GRADES_JSON SCANS_JSON NEW_RUN_DIRECTORY");
const grades = JSON.parse(readFileSync(gradesPath));
const scans = JSON.parse(readFileSync(scansPath));
const cases = calibrationCases.map((item) => ({
  ...item,
  baselineFiles: caseFiles(item),
  referenceFiles: caseFiles(item, true),
  sourceSha256: caseIdentity(item),
  referenceSha256: caseIdentity(item, true),
}));
if (
  digest(cases) !== grades.corpusSha256 ||
  grades.results.length !== cases.length ||
  grades.results.some((item) => !item.passed)
)
  throw new Error("Current corpus does not have a passing validation receipt");
const harness = deployHarness();
const graderSha256 = digest(
  Object.fromEntries(
    [
      "calibration-grader.mjs",
      "candidate-sandbox.mjs",
      "node-candidate.mjs",
      "python-candidate.py",
    ].map((name) => [name, harness.files[`guest/${name}`]]),
  ),
);
if (grades.graderSha256 !== graderSha256)
  throw new Error("Independent grader changed; repeat case validation before freezing");
const protocol = readFileSync(
  new URL("../../docs/qa/agent-workflow-benchmark.md", import.meta.url),
);
const study = {
  schemaVersion: 1,
  id: "subscription-calibration",
  phase: pilotPolicy.phase,
  seed: 20260904,
  repeats: pilotPolicy.repeats,
  arms: pilotPolicy.arms,
  limits: pilotPolicy.limits,
  billing: pilotPolicy.billing,
  protocol: "agent-workflow-v1",
  protocolSha256: digest(protocol),
  runnerSha256: harness.id,
  corpusSha256: digest(cases),
  productSha256: digest(scans.product),
  sitecmd: {
    version: scans.product.version,
    commit: scans.product.commit,
    dirty: false,
    mcpSha256: scans.product.mcpSha256,
  },
  configurations: pilotPolicy.models.map(({ agent, model }) => ({
    id: `${agent}-${model.replaceAll(".", "-")}-${reasoning}`,
    agent,
    model,
    agentVersion: agentVersions[agent],
    reasoning,
    environment: `${scans.product.environment}; warm; controller ${harness.id}`,
  })),
  tasks: cases.map((item) => {
    if (scans.sources[item.id] !== item.sourceSha256)
      throw new Error(`Scan source changed: ${item.id}`);
    const scan = scans.results.find((result) => result.id === item.id);
    if (!scan) throw new Error(`Missing scan: ${item.id}`);
    return {
      id: item.id,
      repository: item.repository,
      kind: item.kind,
      runtime: item.runtime,
      entry: item.entry,
      rule: item.rule,
      surface: "code",
      category: item.category,
      prompt: item.prompt,
      requirements: item.requirements,
      provenance:
        "Owned seeded calibration case (repository Apache-2.0 license); not historical or held out. Scanner misses remain in the assigned population.",
      holdout: false,
      sourceSha256: item.sourceSha256,
      referenceSha256: item.referenceSha256,
      graderSha256,
      reportSha256: digest(scan.baseline.raw),
      baseline: { acceptancePass: item.kind === "negative_control", regressionsPass: true },
      reference: { acceptancePass: true, regressionsPass: true },
      validatedBy:
        "Automated independent behavior and existing-test checks, three baseline/reference repetitions; reference implementation inspected during preparation",
    };
  }),
};
validatePilotStudy(study);
const plan = createStudyRun(study, output);
mkdirSync(path.join(output, "inputs"), { mode: 0o700 });
writeNewJson(path.join(output, "inputs", "corpus.json"), cases);
writeNewJson(path.join(output, "inputs", "product.json"), scans.product);
writeNewJson(path.join(output, "inputs", "grades.json"), grades);
writeNewJson(path.join(output, "inputs", "runner.json"), harness.files);
writeNewJson(path.join(output, "inputs", "scans.json"), scans);
const emptyQuota = JSON.parse(readFileSync(new URL("./quota-template.json", import.meta.url)));
for (const name of ["quota-baseline.json", "quota-current.json"])
  writeNewJson(path.join(output, name), emptyQuota);
for (const scan of scans.results)
  writeFileSync(path.join(output, "inputs", `${scan.id}-report.json`), scan.baseline.raw, {
    flag: "wx",
    mode: 0o600,
  });
writeFileSync(path.join(output, "inputs", "protocol.md"), protocol, { flag: "wx", mode: 0o600 });
console.log(
  `Frozen ${plan.plannedTrials} assignments at ${path.resolve(output)}. No agent calls made. Logins and fresh quota evidence are still required.`,
);
