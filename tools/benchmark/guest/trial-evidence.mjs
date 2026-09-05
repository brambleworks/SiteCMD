import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { digest } from "../lib/workflow-plan.mjs";
import { writeNewJson } from "../lib/workflow-store.mjs";
import { claudeUsage, codexUsage } from "../lib/workflow-usage.mjs";
import { gradeCase } from "./calibration-grader.mjs";
import { candidatePatch, compareCandidate, readCandidate } from "./trial-snapshot.mjs";
import { captureRuntimeFiles, withoutRuntimeFiles } from "./trial-runtime-files.mjs";

export function createEvidence(directory, plan, assignment, item, files, workspace) {
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  const submissions = [];
  const attempts = new Set();
  let runtimeFiles = [];
  const task = plan.study.tasks.find((task) => task.id === item.id);
  const log = (name, value) =>
    appendFileSync(path.join(directory, name), `${JSON.stringify(value)}\n`, { mode: 0o600 });
  const initialized = () => {
    runtimeFiles = captureRuntimeFiles(files, readCandidate(workspace));
    writeNewJson(path.join(directory, "runtime-files.json"), {
      capturedAt: new Date().toISOString(),
      stage: "Client initialized before first user prompt",
      emptyProtectionFiles: runtimeFiles,
    });
  };
  const normalize = (snapshot) => ({
    ...snapshot,
    files: withoutRuntimeFiles(snapshot.files, runtimeFiles),
  });
  const submit = (summary, elapsedMs, attemptId) => {
    const index = submissions.length + 1;
    const prefix = `submission-${index}`;
    const snapshot = normalize(readCandidate(workspace));
    const integrity = compareCandidate(files, snapshot.files, snapshot.violations);
    const staging = path.join(directory, prefix);
    const patch = candidatePatch(staging, files, snapshot.files);
    const patchSha256 = digest(patch);
    writeFileSync(path.join(directory, `${prefix}.diff`), patch, { flag: "wx", mode: 0o600 });
    const grade = integrity.passed
      ? gradeCase(item, path.join(staging, "candidate"))
      : { acceptancePass: false, regressionsPass: false, skipped: integrity.reason };
    writeNewJson(path.join(directory, `${prefix}-checks.json`), { ...grade, integrity, summary });
    const receipt = {
      trialId: assignment.id,
      studySha256: plan.studySha256,
      sourceSha256: task.sourceSha256,
      patchSha256,
      graderSha256: task.graderSha256,
      executor: "Independent guest behavioral assertions",
      environment: plan.study.configurations[0].environment,
      acceptance: [
        {
          command: "gradeCase acceptance assertions",
          exitCode: integrity.passed ? (grade.acceptancePass ? 0 : 1) : null,
          log: `${prefix}-checks.json`,
        },
      ],
      regressions: [
        {
          command: "gradeCase regression assertions and existing project tests",
          exitCode: integrity.passed ? (grade.regressionsPass ? 0 : 1) : null,
          log: `${prefix}-checks.json`,
        },
      ],
      integrity,
    };
    writeNewJson(path.join(directory, `${prefix}-grade.json`), receipt);
    submissions.push({
      patch: `${prefix}.diff`,
      patchSha256,
      elapsedMs,
      graderSha256: task.graderSha256,
      acceptancePass: grade.acceptancePass,
      regressionsPass: grade.regressionsPass,
      integrityPass: integrity.passed,
      receipt: `${prefix}-grade.json`,
    });
    if (attemptId) attempts.add(attemptId);
    return {
      integrity,
      snapshotSha256: digest(
        Object.fromEntries(
          Object.entries(snapshot.files).map(([name, data]) => [name, data.toString("base64")]),
        ),
      ),
    };
  };
  const finish = ({
    status,
    failure,
    elapsedMs,
    configuration,
    quotaAllowed,
    agentInvoked = true,
    evidenceComplete = true,
    providerCompleted = status === "completed",
    observedModels = [],
  }) => {
    const raw = readFileSync(path.join(directory, "transcript.jsonl"), "utf8");
    const events = raw
      .split("\n")
      .filter(Boolean)
      .flatMap((line) => {
        try {
          return [JSON.parse(line)];
        } catch {
          return [];
        }
      });
    const unknown = {
      inputTokens: null,
      outputTokens: null,
      cacheReadTokens: null,
      cacheWriteTokens: null,
      includesAllAgents: false,
      costUsd: null,
      costBasis: "subscription",
      incrementalCostUsd: null,
      apiEquivalentCostUsd: null,
      receipt: "usage.json",
    };
    let usage = unknown;
    if (!agentInvoked) {
      usage = {
        ...unknown,
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        includesAllAgents: true,
        incrementalCostUsd: 0,
      };
    } else if (configuration.agent === "claude" && providerCompleted && evidenceComplete) {
      const results = events.filter((event) => event.type === "result");
      if (results.length === 1)
        usage = claudeUsage(results[0], { noSubagents: true, billingMode: "subscription" });
    } else {
      const turns = events.filter((event) => event.type === "turn.completed");
      if (turns.length && providerCompleted && evidenceComplete) {
        const rows = turns.map((event) =>
          codexUsage(event, { noSubagents: true, billingMode: "subscription" }),
        );
        usage = { ...rows[0] };
        for (const key of ["inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens"])
          usage[key] = rows.some((row) => row[key] === null)
            ? null
            : rows.reduce((sum, row) => sum + row[key], 0);
      }
    }
    const { receipt, ...counts } = usage;
    writeNewJson(path.join(directory, receipt), {
      usage: counts,
      accountant: agentInvoked ? "Pinned CLI event normalizer" : "Guest setup controller",
      method:
        plan.study.phase === "fixture"
          ? "Synthetic provider-shaped events from an owned test process; not inference or spending evidence."
          : agentInvoked
            ? "Raw provider events; delegated tools disabled. Interrupted or truncated usage remains unknown. Additional charges require billing review."
            : "Setup failed before launching a model client; no inference calls or additional charges occurred.",
      raw: [
        "transcript.jsonl",
        "stderr.log",
        "configuration.json",
        "quota-events.jsonl",
        "quota-baseline.json",
        "quota-current.json",
        ...(agentInvoked ? ["prompt.txt", "final-candidate.json"] : []),
        ...(existsSync(path.join(directory, "runtime-files.json")) ? ["runtime-files.json"] : []),
        ...(assignment.arm === "mcp" ? ["mcp.jsonl"] : []),
      ],
    });
    const record = {
      schemaVersion: 1,
      trialId: assignment.id,
      studySha256: plan.studySha256,
      fixture: plan.study.phase === "fixture",
      agentInvoked,
      status: quotaAllowed ? status : "infrastructure_error",
      ...(status !== "completed" || !quotaAllowed
        ? { failure: failure || "Quota evidence failed after the trial; batch paused" }
        : {}),
      elapsedMs,
      humanActiveMs: null,
      setup: "warm",
      agentVersion: configuration.agentVersion,
      model: agentInvoked && observedModels.length === 1 ? observedModels[0] : null,
      ...(agentInvoked
        ? {
            modelSelection: {
              requested: configuration.model,
              observed: observedModels,
              source: "explicit-cli-request",
            },
          }
        : {}),
      transcript: "transcript.jsonl",
      submissions,
      reviews: [],
      usage,
      ...(assignment.arm === "mcp"
        ? { mcp: { serverSha256: plan.study.sitecmd.mcpSha256, trace: "mcp.jsonl" } }
        : {}),
    };
    writeNewJson(path.join(directory, "trial.json"), record);
    return record;
  };
  return { submissions, attempts, submit, log, finish, initialized, normalize };
}
