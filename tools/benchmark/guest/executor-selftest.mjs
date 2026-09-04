import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { writeNewJson } from "../lib/workflow-store.mjs";
import { collectEvidence } from "../lib/workflow-artifacts.mjs";
import { createEvidence } from "./trial-evidence.mjs";
import { launchAgent } from "./trial-supervisor.mjs";
import { createTrialBridge } from "./trial-bridge.mjs";
import { createWorkspace, closeWorkspace } from "./trial-workspace.mjs";
import { readCandidate } from "./trial-snapshot.mjs";
import { systemCommand } from "./desktop-session.mjs";

const { plan, assignment, item, files, reference, mode } = JSON.parse(readFileSync(0, "utf8"));
if (plan.study.phase !== "fixture" || !["repair", "timeout"].includes(mode))
  throw new Error("Executor self-tests require an explicitly synthetic fixture study");
const directory = `/srv/sitecmd-benchmark/trials/${assignment.id}`;
const workspace = createWorkspace(assignment.id, files);
const evidence = createEvidence(directory, plan, assignment, item, files, workspace);
const quota = {
  schemaVersion: 1,
  capturedAt: new Date().toISOString(),
  source: "Synthetic self-test only, not a provider reading or permission for model calls",
  accounts: ["codex", "claude"].map((provider) => ({
    provider,
    account: `${provider}-fixture`,
    authMode: "subscription",
    extraUsageEnabled: false,
    windows: [
      {
        id: "weekly",
        kind: "weekly",
        usedPercent: 0,
        resetsAt: new Date(Date.now() + 86400000).toISOString(),
      },
    ],
  })),
};
writeNewJson(`${directory}/quota-baseline.json`, quota);
writeNewJson(`${directory}/quota-current.json`, quota);
const configuration = plan.study.configurations[0];
writeNewJson(`${directory}/configuration.json`, {
  fixture: true,
  configuration,
  client: "owned Node script, not an AI agent",
});
writeFileSync(`${directory}/prompt.txt`, "Synthetic executor test; no model call", { mode: 0o600 });
mkdirSync("/run/sitecmd-benchmark", { recursive: true, mode: 0o755 });
const socket = `/run/sitecmd-benchmark/${assignment.id}.sock`;
let agent, bridge;
try {
  bridge = await createTrialBridge({
    socket,
    arm: "normal",
    mcp: null,
    owner: {
      uid: Number(systemCommand("id", ["-u", "runner"])),
      gid: Number(systemCommand("id", ["-g", "runner"])),
    },
    submit: async (summary) => {
      agent.quota();
      agent.freeze();
      try {
        evidence.submit(summary, agent.elapsed());
      } finally {
        agent.thaw();
      }
      return { recorded: true };
    },
  });
  const script =
    mode === "timeout"
      ? "setInterval(() => {}, 1000)"
      : `
    const fs = require('node:fs'), http = require('node:http');
    const { socket, reference } = JSON.parse(process.argv[1]);
    const submit = summary => new Promise((resolve, reject) => {
      const request = http.request({socketPath:socket,path:'/submit',method:'POST'}, response => {
        response.resume(); response.on('end', () => response.statusCode === 200 ? resolve() : reject(new Error('Submission failed')));
      });
      request.on('error', reject); request.end(JSON.stringify({summary}));
    });
    (async () => {
      console.log(JSON.stringify({type:'turn.started',model:'fixture-model',synthetic:true}));
      await submit('Owned baseline fixture, deliberately still broken');
      for (const [name, contents] of Object.entries(reference)) fs.writeFileSync(name, contents);
      await submit('Owned reference fixture');
      console.log(JSON.stringify({type:'turn.completed',synthetic:true,usage:{input_tokens:0,cached_input_tokens:0,output_tokens:0}}));
    })().catch(error => {console.error(error); process.exitCode = 1;});
  `;
  agent = launchAgent({
    id: assignment.id,
    workspace,
    directory,
    plan,
    baseline: quota,
    currentQuota: `${directory}/quota-current.json`,
    requestedModel: "fixture-model",
    log: evidence.log,
    prompt: "Synthetic self-test, not an AI prompt",
    invocation: {
      command: "node",
      args: ["-e", script, JSON.stringify({ socket, reference })],
      env: {},
    },
  });
  const result = await agent.done;
  writeNewJson(`${directory}/final-candidate.json`, readCandidate(workspace));
  const record = evidence.finish({ ...result, configuration, quotaAllowed: true });
  collectEvidence(record, assignment, plan, directory);
  assert.equal(record.status, mode === "repair" ? "completed" : "timeout");
  if (mode === "repair") {
    assert.equal(record.submissions.length, 2);
    assert.equal(record.submissions[0].acceptancePass, false);
    assert.equal(record.submissions[1].acceptancePass, true);
    assert.equal(record.submissions[1].regressionsPass, true);
  } else assert.equal(record.usage.includesAllAgents, false);
  console.log(JSON.stringify({ fixture: true, mode, status: record.status, passed: true }));
} finally {
  agent?.stop("Self-test teardown");
  if (agent) await agent.done;
  try {
    await bridge?.close();
  } finally {
    closeWorkspace(workspace);
  }
}
