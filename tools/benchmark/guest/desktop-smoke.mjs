import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { startDesktop } from "./desktop-session.mjs";
import { createWorkspace, mountDesktopWorkspace, closeWorkspace } from "./trial-workspace.mjs";
import { verifyControlIsolation } from "./trial-isolation.mjs";
import { initializeMcp, openMcp } from "./mcp-session.mjs";

const { id, item, files } = JSON.parse(readFileSync(0, "utf8"));
const workspace = createWorkspace(id, files);
const trace = [];
let mcp, mounted, desktop;
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
  mcp = openMcp("/usr/lib/SiteCMD/sitecmd-mcp/sitecmd-mcp.mjs", desktop.database, (event) =>
    trace.push(event),
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
    }),
  );
} finally {
  mcp?.close();
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
