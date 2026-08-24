import test from "node:test";
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";

import { readDesktopHeartbeat } from "../dist/heartbeat.js";
import { deriveFixStatus } from "../dist/fix_status.js";
import { connectInMemory } from "./tools_list_snapshot.test.mjs";
import { ensureProject, makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-agent-loop-");
const { addWorkItem, addFixAttempt } = makeSeeders(fixtureDb);
const URL = "https://loop.test";
const heartbeatPath = join(dirname(process.env.SITECMD_DB_PATH), "desktop-heartbeat.json");

function beat(ageMs) {
  writeFileSync(
    heartbeatPath,
    JSON.stringify({ pid: 1, version: "1.0.0", updated_at_ms: Date.now() - ageMs }),
  );
}

function seedProject(projectId) {
  ensureProject(fixtureDb, projectId);
  fixtureDb
    .prepare(
      `INSERT OR IGNORE INTO environments (project_id, url, label, environment) VALUES (?, ?, 'Production', 'production')`,
    )
    .run(projectId, URL);
}

async function call(name, args) {
  const session = await connectInMemory();
  try {
    const result = await session.client.callTool({ name, arguments: args });
    return { text: result.content[0].text, isError: result.isError === true };
  } finally {
    await session.close();
  }
}

test("a fresh heartbeat means the app is running; missing or stale means it is not", () => {
  beat(1_000);
  assert.equal(readDesktopHeartbeat(Date.now()).alive, true);
  beat(31_000);
  assert.equal(readDesktopHeartbeat(Date.now()).alive, false);
});

test("a torn heartbeat file reads as unknown, never as not running", () => {
  writeFileSync(heartbeatPath, '{"pid": 1, "version": "1.0.0", "updated_at_m');
  const status = readDesktopHeartbeat(Date.now());
  assert.equal(status.alive, false);
  assert.equal(status.unknown, true);
});

test("a missing heartbeat file reads as not running, not unknown", () => {
  if (existsSync(heartbeatPath)) unlinkSync(heartbeatPath);
  const status = readDesktopHeartbeat(Date.now());
  assert.equal(status.alive, false);
  assert.equal(status.unknown, false);
});

test("deriveFixStatus reports awaiting_deploy only for remote web attempts still verifying", () => {
  const base = {
    status: "verifying",
    check_id: "security.csp",
    producer_rule: null,
    env_url: "https://loop.test",
  };
  assert.deepEqual(deriveFixStatus(base), {
    label: "verifying (awaiting_deploy)",
    awaitingDeploy: true,
  });
  assert.equal(
    deriveFixStatus({ ...base, env_url: "http://localhost:4321" }).awaitingDeploy,
    false,
  );
  assert.equal(
    deriveFixStatus({ ...base, check_id: "code_scan.hardcoded-secret" }).awaitingDeploy,
    false,
  );
  assert.equal(deriveFixStatus({ ...base, producer_rule: "hsts_missing" }).awaitingDeploy, false);
  assert.equal(deriveFixStatus({ ...base, status: "verified" }).label, "verified");
});

test("start_fix queues a request and returns the attempt once the desktop fulfils it", async () => {
  seedProject(1001);
  addWorkItem({
    projectId: 1001,
    envUrl: URL,
    checkId: "security.csp",
    severity: "high",
    title: "Missing CSP",
  });
  beat(1_000);
  const fulfil = setInterval(() => {
    const row = fixtureDb
      .prepare("SELECT id FROM agent_requests WHERE status = 'requested' AND kind = 'start_fix'")
      .get();
    if (!row) return;
    const attemptId = addFixAttempt({
      projectId: 1001,
      envUrl: URL,
      checkId: "security.csp",
      briefMd: "# SiteCMD Fix Brief: Missing CSP",
    });
    fixtureDb
      .prepare(
        "UPDATE agent_requests SET status = 'fulfilled', result_json = ?, updated_at = ? WHERE id = ?",
      )
      .run(JSON.stringify({ attempt_id: attemptId, status: "briefed" }), Date.now(), row.id);
  }, 50);
  try {
    const { text, isError } = await call("start_fix", { url: URL, check_id: "security.csp" });
    assert.equal(isError, false, text);
    assert.match(text, /Fix attempt #\d+ is briefed/);
    assert.match(text, /get_fix_brief/);
  } finally {
    clearInterval(fulfil);
  }
});

test("start_fix says the app is closed when the heartbeat is stale", async () => {
  seedProject(1002);
  addWorkItem({
    projectId: 1002,
    envUrl: URL,
    checkId: "security.hsts",
    severity: "medium",
    title: "Missing HSTS",
  });
  beat(60_000);
  const { text } = await call("start_fix", { url: URL, check_id: "security.hsts", wait: false });
  assert.match(text, /SiteCMD is not running/);
  assert.match(text, /request #\d+/);
  const row = fixtureDb
    .prepare(
      "SELECT kind, check_id, agent_tool, status FROM agent_requests ORDER BY id DESC LIMIT 1",
    )
    .get();
  assert.deepEqual(row, {
    kind: "start_fix",
    check_id: "security.hsts",
    agent_tool: "claude-code",
    status: "requested",
  });
});

test("request_verification tells the truth about the desktop and deploys", async () => {
  seedProject(1003);
  const attemptId = addFixAttempt({
    projectId: 1003,
    envUrl: URL,
    checkId: "security.csp",
    status: "briefed",
  });
  beat(60_000);
  const closed = await call("request_verification", {
    attempt_id: attemptId,
    summary: "Added CSP.",
  });
  assert.match(closed.text, /SiteCMD is not running; verification starts when it opens/);
  beat(1_000);
  const open = await call("request_verification", { attempt_id: attemptId, summary: "Added CSP." });
  assert.match(open.text, /re-run the check within about 5 seconds/);
  assert.match(open.text, /not live until you deploy/);
});

test("get_fix_status exposes failure detail and verify timing", async () => {
  seedProject(1004);
  const attemptId = addFixAttempt({
    projectId: 1004,
    envUrl: URL,
    checkId: "security.csp",
    status: "verify_failed",
  });
  fixtureDb
    .prepare(
      "UPDATE fix_attempts SET failure_detail = 'still missing', verify_started_at = 1700000000000 WHERE id = ?",
    )
    .run(attemptId);
  const { text } = await call("get_fix_status", { attempt_id: attemptId });
  assert.match(text, /Status: verify_failed/);
  assert.match(text, /Failure detail: still missing/);
  assert.match(text, /Verification started: 2023-11-14T22:13:20\.000Z/);
});

test("run_scan returns a request handle without waiting and get_scan_status reads it", async () => {
  seedProject(1005);
  beat(1_000);
  const { text } = await call("run_scan", { url: URL, scope: "web", wait: false });
  const requestId = Number(/request #(\d+)/.exec(text)[1]);
  fixtureDb
    .prepare(
      "UPDATE agent_requests SET status = 'fulfilled', result_json = ?, updated_at = ? WHERE id = ?",
    )
    .run(
      JSON.stringify({ execution_id: 77, reused: false, status: "complete" }),
      Date.now(),
      requestId,
    );
  const status = await call("get_scan_status", { request_id: requestId });
  assert.match(status.text, /fulfilled/);
  assert.match(status.text, /execution #77/);
  assert.match(status.text, /compare_scans/);
});

test("get_fix_status resolves a request_id to its attempt once the desktop fulfils it", async () => {
  seedProject(1006);
  addWorkItem({
    projectId: 1006,
    envUrl: URL,
    checkId: "security.referrer",
    severity: "low",
    title: "Missing Referrer-Policy",
  });
  beat(60_000);
  const queued = await call("start_fix", { url: URL, check_id: "security.referrer", wait: false });
  const requestId = Number(/request #(\d+)/.exec(queued.text)[1]);
  const attemptId = addFixAttempt({
    projectId: 1006,
    envUrl: URL,
    checkId: "security.referrer",
    briefMd: "# SiteCMD Fix Brief: Missing Referrer-Policy",
  });
  fixtureDb
    .prepare(
      "UPDATE agent_requests SET status = 'fulfilled', result_json = ?, updated_at = ? WHERE id = ?",
    )
    .run(JSON.stringify({ attempt_id: attemptId, status: "briefed" }), Date.now(), requestId);
  const { text, isError } = await call("get_fix_status", { request_id: requestId });
  assert.equal(isError, false, text);
  assert.match(text, new RegExp(`Fix attempt #${attemptId}\\b`));
  assert.match(text, /Check: security\.referrer/);
  assert.match(text, /Status: briefed/);
});

test("run_scan waits for the desktop to fulfil it when wait is true", async () => {
  seedProject(1007);
  beat(1_000);
  const fulfil = setInterval(() => {
    const row = fixtureDb
      .prepare("SELECT id FROM agent_requests WHERE status = 'requested' AND kind = 'run_scan'")
      .get();
    if (!row) return;
    fixtureDb
      .prepare(
        "UPDATE agent_requests SET status = 'fulfilled', result_json = ?, updated_at = ? WHERE id = ?",
      )
      .run(
        JSON.stringify({ execution_id: 88, reused: false, status: "complete" }),
        Date.now(),
        row.id,
      );
  }, 50);
  try {
    const { text, isError } = await call("run_scan", { url: URL, scope: "web", wait: true });
    assert.equal(isError, false, text);
    assert.match(text, /execution #88/);
    assert.match(text, /compare_scans/);
  } finally {
    clearInterval(fulfil);
  }
});

test("list_fix_attempts includes settled attempts only when include_settled is true", async () => {
  seedProject(1008);
  const activeId = addFixAttempt({
    projectId: 1008,
    envUrl: URL,
    checkId: "security.open.check",
    status: "briefed",
  });
  const settledId = addFixAttempt({
    projectId: 1008,
    envUrl: URL,
    checkId: "security.settled.check",
    status: "verified",
  });
  const defaultList = await call("list_fix_attempts", {});
  assert.match(defaultList.text, new RegExp(`#${activeId}\\b`));
  assert.doesNotMatch(defaultList.text, new RegExp(`#${settledId}\\b`));
  const withSettled = await call("list_fix_attempts", { include_settled: true });
  assert.match(withSettled.text, new RegExp(`#${activeId}\\b`));
  assert.match(withSettled.text, new RegExp(`#${settledId}\\b`));
});

test("run_scan refuses a URL the project does not own", async () => {
  seedProject(1009);
  beat(1_000);
  const foreignUrl = "https://attacker.example/?q=1";
  const rejected = await call("run_scan", {
    project_id: 1009,
    url: foreignUrl,
    wait: false,
  });
  assert.equal(rejected.isError, true, rejected.text);
  assert.match(rejected.text, /not an environment of project #1009/);
  assert.match(rejected.text, /loop\.test/);
  const queued = fixtureDb
    .prepare("SELECT COUNT(*) AS total FROM agent_requests WHERE env_url LIKE '%attacker%'")
    .get();
  assert.equal(queued.total, 0);

  const owned = await call("run_scan", { project_id: 1009, url: URL, wait: false });
  assert.equal(owned.isError, false, owned.text);
  assert.match(owned.text, /request #\d+/);

  const byUrlOnly = await call("run_scan", { url: URL, wait: false });
  assert.equal(byUrlOnly.isError, false, byUrlOnly.text);
  assert.match(byUrlOnly.text, /request #\d+/);
});
