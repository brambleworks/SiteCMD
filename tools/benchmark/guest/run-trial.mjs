import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { digest, validatePlan } from "../lib/workflow-plan.mjs";
import { probeAgentAccounts } from "../lib/workflow-preflight.mjs";
import { evaluateQuota } from "../lib/workflow-quota.mjs";
import { writeNewJson } from "../lib/workflow-store.mjs";
import { agentVersions, trialInvocation } from "../lib/trial-invocation.mjs";
import { createTrialBridge } from "./trial-bridge.mjs";
import { startDesktop, systemCommand } from "./desktop-session.mjs";
import { openMcp } from "./mcp-session.mjs";
import {
  createWorkspace,
  mountDesktopWorkspace,
  closeWorkspace,
  protectPreviousWorkspaces,
} from "./trial-workspace.mjs";
import { verifyControlIsolation } from "./trial-isolation.mjs";
import { createEvidence } from "./trial-evidence.mjs";
import { launchAgent } from "./trial-supervisor.mjs";
import { prepareProject, trialUrl } from "./trial-setup.mjs";
import { readFix, observeVerification } from "./product-observation.mjs";
import { readCandidate } from "./trial-snapshot.mjs";
import { closingQuota } from "./closing-quota.mjs";

if (process.platform !== "linux" || process.getuid() !== 0)
  throw new Error("Guest controller required");
const input = JSON.parse(readFileSync(0, "utf8"));
const { assignment, item, files, product, baseline, current } = input;
const plan = validatePlan(input.plan);
if (!plan.assignments.some((entry) => digest(entry) === digest(assignment)))
  throw new Error("Unknown assignment");
const task = plan.study.tasks.find((task) => task.id === assignment.task);
if (digest(files) !== task.sourceSha256 || item.id !== task.id)
  throw new Error("Case source differs from the frozen study");
for (const key of ["id", "kind", "runtime", "entry", "rule"])
  if (item[key] !== task[key]) throw new Error(`Case ${key} differs from the frozen study`);
if (digest(product) !== plan.study.productSha256)
  throw new Error("Product receipt differs from the frozen study");
const configuration = plan.study.configurations.find(
  (entry) => entry.id === assignment.configuration,
);
if (
  configuration.agentVersion !== agentVersions[configuration.agent] ||
  configuration.reasoning !== "high"
)
  throw new Error("Agent configuration differs from the executor");
const accounts = probeAgentAccounts({
  run: (command, args, options) => spawnSync("sudo", ["-u", "runner", command, ...args], options),
});
if (
  !accounts.subscriptionAccountsVerified ||
  accounts.accounts.some((entry) => entry.version !== agentVersions[entry.agent])
)
  throw new Error(
    "Guest subscription logins and pinned client versions must be verified before execution",
  );
const quota = evaluateQuota(baseline, current, plan.study.billing);
if (!quota.quotaAllowed) throw new Error(quota.blockers.join("; "));
const budgets = "/srv/sitecmd-benchmark/budgets";
mkdirSync(budgets, { recursive: true, mode: 0o700 });
const budgetFile = `${budgets}/${plan.studySha256}.json`;
if (!existsSync(budgetFile)) writeNewJson(budgetFile, baseline);
if (digest(JSON.parse(readFileSync(budgetFile))) !== digest(baseline))
  throw new Error("Study allowance baseline changed");
for (const [file, hash] of [
  [product.binary, product.binarySha256],
  [product.mcp, plan.study.sitecmd.mcpSha256],
  [product.cli, product.cliSha256],
])
  if (digest(readFileSync(file)) !== hash)
    throw new Error("Installed product changed after freezing");
const directory = `/srv/sitecmd-benchmark/trials/${assignment.id}`;
mkdirSync(directory, { recursive: true, mode: 0o700 });
writeNewJson(`${directory}/quota-baseline.json`, baseline);
writeNewJson(`${directory}/quota-current.json`, current);
const workspace = `/srv/sitecmd-benchmark/workspaces/${assignment.id}`;
const evidence = createEvidence(directory, plan, assignment, item, files, workspace);
let desktop, bridge, mcp, agent, mounted;
let workspaceCreated = false;
let result;
let finalSnapshot;
let agentInvoked = false;
let setupStage = "environment";
const socket = `/run/sitecmd-benchmark/${assignment.id}.sock`;
const assertNotCancelled = () => {
  if (existsSync(`/run/sitecmd-benchmark-cancel-${assignment.id}`))
    throw new Error("Trial cancelled by the operator");
};
const publicTools = "/usr/local/lib/sitecmd-benchmark";
try {
  assertNotCancelled();
  createWorkspace(assignment.id, files);
  workspaceCreated = true;
  protectPreviousWorkspaces(workspace);
  mounted = mountDesktopWorkspace(assignment.id, workspace);
  desktop = await startDesktop(assignment.id, product.binary);
  verifyControlIsolation();
  setupStage = "product";
  const prepared = await prepareProject(
    desktop,
    mounted,
    item,
    product,
    configuration,
    assignment.arm,
    evidence.log,
  );
  setupStage = "environment";
  mkdirSync("/run/sitecmd-benchmark", { recursive: true, mode: 0o755 });
  mkdirSync(publicTools, { recursive: true, mode: 0o755 });
  for (const name of ["bridge-client.mjs", "mcp-proxy.mjs", "submit.mjs"])
    copyFileSync(new URL(`./${name}`, import.meta.url), path.join(publicTools, name));
  mcp =
    assignment.arm === "mcp"
      ? openMcp(product.mcp, desktop.database, (event) => evidence.log("mcp.jsonl", event))
      : null;
  bridge = await createTrialBridge({
    socket,
    arm: assignment.arm,
    mcp,
    owner: {
      uid: Number(systemCommand("id", ["-u", "runner"])),
      gid: Number(systemCommand("id", ["-g", "runner"])),
    },
    submit: async (summary, kind, attemptId) => {
      if (!agent || typeof summary !== "string" || !summary.trim() || summary.length > 2000)
        throw new Error("A concise submission summary is required");
      if (evidence.submissions.length >= plan.study.limits.submissions) {
        agent.stop("Submission limit reached", "agent_error");
        throw new Error("Submission limit reached");
      }
      if (assignment.arm === "mcp" && item.kind === "repair" && kind !== "verification")
        throw new Error("Submit repairs through SiteCMD request_verification");
      if (
        kind === "verification" &&
        (!Number.isSafeInteger(attemptId) ||
          readFix(desktop.database, attemptId)?.project_id !== prepared.projectId)
      )
        throw new Error("Verification must refer to this trial's project");
      try {
        agent.quota();
      } catch (error) {
        agent.stop(error.message);
        throw error;
      }
      agent.freeze();
      try {
        const captured = evidence.submit(summary, agent.elapsed(), attemptId);
        finalSnapshot = captured.snapshotSha256;
        if (!captured.integrity.passed) {
          agent.stop(captured.integrity.reason, "agent_error");
          throw new Error(captured.integrity.reason);
        }
      } finally {
        agent.thaw();
      }
      return {
        recorded: evidence.submissions.length,
        remaining: plan.study.limits.submissions - evidence.submissions.length,
        message:
          "Candidate recorded. Independent grading feedback is withheld until the trial ends.",
      };
    },
  });
  const invocation = trialInvocation({
    agent: configuration.agent,
    arm: assignment.arm,
    workspace,
    socket,
    proxy: `${publicTools}/mcp-proxy.mjs`,
  });
  writeNewJson(`${directory}/configuration.json`, { configuration, invocation, accounts });
  const submission =
    assignment.arm === "mcp" && item.kind === "repair"
      ? "Submit each candidate with the SiteCMD request_verification tool. Read get_fix_brief first and use get_fix_status to check the result."
      : `Submit each candidate, including an intentional no-op, with: node ${publicTools}/submit.mjs ${socket} "short summary"`;
  const prompt = [
    task.prompt,
    task.requirements,
    `Work only in ${workspace}. Do not delegate, invoke another AI client, change tests or scanner suppressions, or access accounts and other workspaces. Use the existing tests and ordinary local tools.`,
    submission,
    "You may submit at most three candidates. Stop editing once you have submitted your final candidate. Explain the result and stop. Independent grader feedback is withheld in every workflow.",
    ...(assignment.arm === "report" ? ["Complete pretrial SiteCMD report:", input.report] : []),
    ...(assignment.arm === "mcp"
      ? [
          `SiteCMD project #${prepared.projectId}, URL ${trialUrl}. Desktop paths in briefs refer to a mount of your current working directory.`,
          prepared.handoff,
        ]
      : []),
  ].join("\n\n");
  writeFileSync(`${directory}/prompt.txt`, prompt, { flag: "wx", mode: 0o600 });
  assertNotCancelled();
  agent = launchAgent({
    id: assignment.id,
    invocation,
    workspace,
    prompt,
    directory,
    plan,
    baseline,
    currentQuota: `${directory}/quota-current.json`,
    requestedModel: configuration.model,
    log: evidence.log,
  });
  agentInvoked = true;
  result = await agent.done;
  const final = readCandidate(workspace);
  writeNewJson(`${directory}/final-candidate.json`, {
    files: Object.fromEntries(
      Object.entries(final.files).map(([name, bytes]) => [name, bytes.toString("base64")]),
    ),
    violations: final.violations,
  });
  const finalHash = digest(
    Object.fromEntries(
      Object.entries(final.files).map(([name, bytes]) => [name, bytes.toString("base64")]),
    ),
  );
  if (!evidence.submissions.length || finalHash !== finalSnapshot || final.violations.length)
    result = {
      ...result,
      status: "agent_error",
      failure: "No final submission, or unsubmitted changes remained after the final candidate",
    };
  if (mcp)
    for (const attempt of evidence.attempts) {
      const remaining = Math.max(0, plan.study.limits.trialSeconds * 1000 - agent.elapsed());
      const observed = await observeVerification(
        desktop.database,
        attempt,
        mcp,
        evidence.log,
        Date.now() + Math.min(120000, remaining),
      );
      if (!observed || ["briefed", "verify_requested", "verifying"].includes(observed.status))
        result = {
          ...result,
          status: "product_error",
          failure: "Desktop verification did not reach a terminal state",
        };
    }
  result.elapsedMs = agent.elapsed();
  if (result.elapsedMs > plan.study.limits.trialSeconds * 1000)
    result = {
      ...result,
      status: "timeout",
      failure: "Trial deadline reached before final verification completed",
    };
} catch (error) {
  agent?.stop(error.message);
  const stopped = agent ? await agent.done : {};
  result = {
    ...stopped,
    status:
      !agentInvoked &&
      setupStage === "product" &&
      !existsSync(`/run/sitecmd-benchmark-cancel-${assignment.id}`)
        ? "product_error"
        : "infrastructure_error",
    failure: error.message,
    elapsedMs: agent?.elapsed() ?? 0,
  };
} finally {
  for (const cleanup of [
    () => bridge?.close(),
    () => mcp?.close(),
    () => desktop?.close(),
    () => mounted?.close(),
    () => workspaceCreated && closeWorkspace(workspace),
  ]) {
    try {
      await cleanup();
    } catch (error) {
      result = {
        ...result,
        status: "infrastructure_error",
        failure: `${result?.failure ?? ""}; cleanup: ${error.message}`,
      };
    }
  }
}
let quotaAllowed;
let quotaFailure;
try {
  if (agentInvoked) {
    const endedAt = Date.now();
    console.error(
      "The agent has stopped. Refresh quota-current.json with new readings from both providers now; waiting up to five minutes before export.",
    );
    const closing = await closingQuota({
      baseline,
      currentPath: `${directory}/quota-current.json`,
      billing: plan.study.billing,
      endedAt,
      log: evidence.log,
    });
    quotaAllowed = closing.quotaAllowed;
    quotaFailure = closing.blockers.join("; ");
  } else
    quotaAllowed = evaluateQuota(
      baseline,
      JSON.parse(readFileSync(`${directory}/quota-current.json`)),
      plan.study.billing,
    ).quotaAllowed;
} catch {
  quotaAllowed = false;
}
if (!agentInvoked) {
  for (const name of [
    "transcript.jsonl",
    "stderr.log",
    "quota-events.jsonl",
    ...(assignment.arm === "mcp" ? ["mcp.jsonl"] : []),
  ])
    // Recording untrusted agent output is what this harness is for. The bytes
    // land in a per-trial directory inside the disposable VM at mode 0600, and
    // the grader reads them back as data, never as code.
    // codeql-allow: js/http-to-file-access
    writeFileSync(
      `${directory}/${name}`,
      `${JSON.stringify({ agentInvoked: false, failure: result.failure })}\n`,
      { flag: "a", mode: 0o600 },
    );
  if (!existsSync(`${directory}/configuration.json`))
    writeNewJson(`${directory}/configuration.json`, {
      configuration,
      accounts,
      agentInvoked: false,
    });
} else if (!existsSync(`${directory}/final-candidate.json`)) {
  writeNewJson(`${directory}/final-candidate.json`, { unavailable: result.failure });
}
const record = evidence.finish({
  ...result,
  failure: [result.failure, quotaFailure].filter(Boolean).join("; ") || null,
  configuration,
  quotaAllowed,
  agentInvoked,
});
console.log(JSON.stringify({ directory, record }));
