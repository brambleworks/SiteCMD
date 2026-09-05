import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { startDesktop, systemCommand } from "./desktop-session.mjs";
import { createWorkspace, mountDesktopWorkspace, closeWorkspace } from "./trial-workspace.mjs";
import { verifyControlIsolation } from "./trial-isolation.mjs";
import { initializeMcp, openMcp } from "./mcp-session.mjs";
import { createTrialBridge } from "./trial-bridge.mjs";
import { readCandidate } from "./trial-snapshot.mjs";
import { digest } from "../lib/workflow-plan.mjs";

const { id, item, files } = JSON.parse(readFileSync(0, "utf8"));
const workspace = createWorkspace(id, files);
const trace = [];
const submissions = [];
let mcp, server, bridge, mounted, desktop;
try {
  mounted = mountDesktopWorkspace(id, workspace);
  desktop = await startDesktop(id, "/usr/bin/sitecmd");
  verifyControlIsolation();
  const projectId = await desktop.invoke("add_project", {
    name: item.id,
    path: mounted.path,
    framework: null,
    urls: [{ url: "http://localhost:4173", environment: "local", source: "benchmark" }],
  });
  server = openMcp("/usr/lib/SiteCMD/sitecmd-mcp/sitecmd-mcp.mjs", desktop.database, (event) =>
    trace.push(event),
  );
  const channel = `/run/sitecmd-benchmark/${id}`;
  const publicTools = "/usr/local/lib/sitecmd-benchmark";
  mkdirSync("/run/sitecmd-benchmark", { recursive: true, mode: 0o755 });
  mkdirSync(publicTools, { recursive: true, mode: 0o755 });
  for (const name of ["bridge-files.mjs", "bridge-client.mjs", "mcp-proxy.mjs"])
    copyFileSync(new URL(`./${name}`, import.meta.url), `${publicTools}/${name}`);
  bridge = await createTrialBridge({
    channel,
    arm: "mcp",
    mcp: server,
    owner: {
      uid: Number(systemCommand("id", ["-u", "runner"])),
      gid: Number(systemCommand("id", ["-g", "runner"])),
    },
    submit: async (summary, kind, attemptId) => {
      const snapshot = readCandidate(workspace);
      assert.deepEqual(snapshot.violations, []);
      submissions.push({ summary, kind, attemptId, snapshotSha256: digest(snapshot.files) });
      return { recorded: true };
    },
  });
  mcp = openMcp(
    `${publicTools}/mcp-proxy.mjs`,
    desktop.database,
    (event) => trace.push({ proxy: true, ...event }),
    { user: "runner", args: [channel] },
  );
  const initialization = await initializeMcp(mcp);
  const scan = await mcp.call("run_scan", {
    project_id: projectId,
    url: "http://localhost:4173",
    scope: "code",
    wait: true,
  });
  if (scan.isError) throw new Error(JSON.stringify(scan));
  const issues = await mcp.call("get_issues", { url: "http://localhost:4173" });
  if (issues.isError) throw new Error(JSON.stringify(issues));
  const attempt = await mcp.call("start_fix", {
    project_id: projectId,
    url: "http://localhost:4173",
    check_id: `code_scan.${item.rule}`,
    wait: true,
  });
  if (attempt.isError) throw new Error(JSON.stringify(attempt));
  const attemptId = Number(/Fix attempt #(\d+) is briefed/.exec(attempt.content?.[0]?.text)?.[1]);
  if (!attemptId) throw new Error(`Fix attempt was not briefed: ${JSON.stringify(attempt)}`);
  const brief = await mcp.call("get_fix_brief", { attempt_id: attemptId });
  if (brief.isError) throw new Error(JSON.stringify(brief));
  for (const [name, contents] of Object.entries(item.reference))
    writeFileSync(path.join(workspace, name), contents);
  const verification = await mcp.call("request_verification", {
    attempt_id: attemptId,
    summary:
      "Applied the owned reference fix for the desktop integration smoke test; no agent participated.",
  });
  if (verification.isError) throw new Error(JSON.stringify(verification));
  assert.equal(submissions.length, 1);
  assert.equal(submissions[0].kind, "verification");
  assert.equal(submissions[0].attemptId, attemptId);
  assert.equal(submissions[0].snapshotSha256, digest(readCandidate(workspace).files));
  let final;
  for (let index = 0; index < 30; index++) {
    final = await mcp.call("get_fix_status", { attempt_id: attemptId });
    if (/^Status: verified$/m.test(final.content?.[0]?.text ?? "")) break;
    await delay(1000);
  }
  if (!/^Status: verified$/m.test(final.content?.[0]?.text ?? ""))
    throw new Error(`Verification did not succeed: ${JSON.stringify(final)}`);
  console.log(
    JSON.stringify({
      id,
      projectId,
      workspace,
      initialization,
      scan,
      issues,
      attempt,
      brief,
      verification,
      final,
      trace,
      submissions,
      transport: "per-trial-file-channel",
    }),
  );
} finally {
  mcp?.close();
  await bridge?.close();
  server?.close();
  try {
    desktop?.close();
  } finally {
    try {
      mounted?.close();
    } finally {
      closeWorkspace(workspace);
    }
  }
}
