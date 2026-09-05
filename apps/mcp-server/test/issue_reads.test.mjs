import test from "node:test";
import assert from "node:assert/strict";

import { describeScanAge } from "../dist/freshness.js";
import { connectInMemory } from "./tools_list_snapshot.test.mjs";
import { ensureProject, makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-issue-reads-");
const { addWorkItem, addFixAttempt } = makeSeeders(fixtureDb);
const URL = "https://reads.test";

function seedProject(projectId) {
  ensureProject(fixtureDb, projectId);
  // Every test in this file shares one URL; rebind it to the project under test so
  // getProjectByUrl resolves to that project instead of whichever test ran first.
  fixtureDb.prepare(`DELETE FROM environments WHERE url = ?`).run(URL);
  fixtureDb
    .prepare(
      `INSERT INTO environments (project_id, url, label, environment)
       VALUES (?, ?, 'Production', 'production')`,
    )
    .run(projectId, URL);
}

/** Mirrors workspace.test.mjs's addProjectWithScanHistory web scan insert, parameterized on score and timestamp. */
function seedWebScan(projectId, url, score, timestamp) {
  seedProject(projectId);
  const startedAt = Date.parse(timestamp);
  const executionId = Number(
    fixtureDb
      .prepare(
        `INSERT INTO scan_executions (
          project_id, environment_url, environment_scope_key, requested_mode, web_focus,
          trigger, admission_class, status, idempotency_key, request_fingerprint,
          started_at, completed_at, web_status
        ) VALUES (?, ?, ?, 'web', 'health', 'manual', 'general_scan', 'complete', ?, ?, ?, ?, 'complete')`,
      )
      .run(
        projectId,
        url,
        url,
        `issue-reads-web-${projectId}`,
        `v1:issue-reads-web-${projectId}`,
        startedAt,
        startedAt + 1200,
      ).lastInsertRowid,
  );
  fixtureDb
    .prepare(
      `INSERT INTO scan_runs (
        execution_id, project_id, environment_url, environment_scope_key,
        source, run_kind, status, focus, started_at, completed_at, timestamp_text,
        raw_score, duration_ms, coverage_kind, coverage_json, mode
      ) VALUES (?, ?, ?, ?, 'web_scan', 'single', 'complete', 'health', ?, ?, ?, ?, 1200, 'site', '{"successful":true}', 'live')`,
    )
    .run(executionId, projectId, url, url, startedAt, startedAt + 1200, timestamp, score);
}

async function call(name, args) {
  const session = await connectInMemory();
  try {
    const result = await session.client.callTool({ name, arguments: args });
    return result.content[0].text;
  } finally {
    await session.close();
  }
}

test("describeScanAge names the date, the age, and staleness past seven days", () => {
  const now = Date.parse("2026-08-22T12:00:00.000Z");
  assert.equal(describeScanAge("2026-08-22T09:00:00Z", now), "Scanned 2026-08-22 (today)");
  assert.match(
    describeScanAge("2026-08-03T09:00:00Z", now),
    /^Scanned 2026-08-03 \(19 days ago\)\. These results are 19 days old/,
  );
});

test("get_issues prints a stable id, check id, confidence, and where each issue lives", async () => {
  seedProject(801);
  addWorkItem({
    projectId: 801,
    envUrl: URL,
    checkId: "performance.images.heavy",
    severity: "low",
    category: "performance",
    title: "Heavy image",
    pageUrl: `${URL}/pricing`,
    confidence: "confirmed",
  });
  addWorkItem({
    projectId: 801,
    envUrl: URL,
    source: "code_scan",
    signalId: "code:n1",
    checkId: "code_scan.possible-n-plus-one",
    severity: "medium",
    title: "Possible N+1",
    relativePath: "src/db/users.ts",
    line: 42,
    confidence: "needs_review",
  });

  const output = await call("get_issues", { url: URL });
  assert.match(output, /### \[MEDIUM\] Possible N\+1 \(#\d+\)/);
  assert.match(output, /\*\*Where:\*\* src\/db\/users\.ts:42/);
  assert.match(output, /\*\*Confidence:\*\* needs_review/);
  assert.match(output, /\*\*Where:\*\* https:\/\/reads\.test\/pricing/);
});

test("get_issues min_severity and min_confidence are thresholds", async () => {
  seedProject(802);
  addWorkItem({
    projectId: 802,
    envUrl: URL,
    checkId: "a.critical",
    severity: "critical",
    title: "Critical",
    confidence: "confirmed",
  });
  addWorkItem({
    projectId: 802,
    envUrl: URL,
    checkId: "b.high",
    severity: "high",
    title: "High heuristic",
    confidence: "needs_review",
  });
  addWorkItem({
    projectId: 802,
    envUrl: URL,
    checkId: "c.low",
    severity: "low",
    title: "Low",
    confidence: "high",
  });

  const bySeverity = await call("get_issues", { url: URL, min_severity: "high" });
  assert.match(bySeverity, /a\.critical/);
  assert.match(bySeverity, /b\.high/);
  assert.doesNotMatch(bySeverity, /c\.low/);

  const byConfidence = await call("get_issues", { url: URL, min_confidence: "high" });
  assert.doesNotMatch(byConfidence, /b\.high/);
  assert.match(byConfidence, /c\.low/);
});

test("get_issues rejects the removed severity and status inputs by name", async () => {
  seedProject(812);
  addWorkItem({
    projectId: 812,
    envUrl: URL,
    checkId: "a.critical",
    severity: "critical",
    title: "Critical",
  });
  addWorkItem({
    projectId: 812,
    envUrl: URL,
    checkId: "c.low",
    severity: "low",
    title: "Low",
  });

  const bySeverity = await call("get_issues", { url: URL, severity: "high" });
  assert.match(bySeverity, /min_severity/);
  assert.doesNotMatch(bySeverity, /a\.critical/);
  assert.doesNotMatch(bySeverity, /c\.low/);

  const byStatus = await call("get_issues", { url: URL, status: "fail" });
  assert.match(byStatus, /status/);
  assert.doesNotMatch(byStatus, /c\.low/);
});

test("get_issue returns one finding with guidance, occurrences, and the active attempt", async () => {
  seedProject(803);
  addWorkItem({
    projectId: 803,
    envUrl: URL,
    source: "code_scan",
    signalId: "rl:1",
    checkId: "code_scan.missing-rate-limit",
    severity: "medium",
    title: "No rate limit",
    description: "Public route without a limiter.",
    relativePath: "src/routes/login.ts",
    line: 12,
    fixPrompt: "Add a limiter.",
  });
  addWorkItem({
    projectId: 803,
    envUrl: URL,
    source: "code_scan",
    signalId: "rl:2",
    checkId: "code_scan.missing-rate-limit",
    severity: "medium",
    title: "No rate limit",
    description: "Public route without a limiter.",
    relativePath: "src/routes/signup.ts",
    line: 30,
  });
  const attemptId = addFixAttempt({
    projectId: 803,
    envUrl: URL,
    checkId: "code_scan.missing-rate-limit",
    status: "briefed",
  });

  const output = await call("get_issue", { url: URL, check_id: "code_scan.missing-rate-limit" });
  assert.match(output, /## \[MEDIUM\] No rate limit/);
  assert.match(output, /src\/routes\/login\.ts:12/);
  assert.match(output, /src\/routes\/signup\.ts:30/);
  assert.match(output, /### Fix prompt\n[\s\S]*Add a limiter\./);
  assert.match(output, new RegExp(`Fix attempt: #${attemptId} \\[briefed\\]`));
  // No causal_link_observations were seeded for this check, so the empty
  // causal block must be dropped entirely rather than leaving a blank section.
  assert.doesNotMatch(output, /\n\n\n/);
});

test("get_fix_prompts limits output and can target one check", async () => {
  seedProject(804);
  for (let index = 0; index < 8; index += 1) {
    addWorkItem({
      projectId: 804,
      envUrl: URL,
      checkId: `seo.prompt-${index}`,
      severity: "low",
      category: "seo",
      title: `Prompt ${index}`,
      fixPrompt: `Fix prompt ${index}`,
    });
  }
  const limited = await call("get_fix_prompts", { url: URL });
  assert.match(limited, /^5 of 8 fix prompt\(s\) for https:\/\/reads\.test/);
  const targeted = await call("get_fix_prompts", { url: URL, check_id: "seo.prompt-3" });
  assert.match(targeted, /^1 of 8 fix prompt\(s\)/);
  assert.match(targeted, /Fix prompt 3/);
  assert.doesNotMatch(targeted, /Fix prompt 4/);
  // The "raise limit" hint only makes sense when the caller isn't already
  // targeting one check_id.
  assert.doesNotMatch(targeted, /pass check_id/);
});

test("get_fix_prompts with zero prompts and no check_id never prints undefined", async () => {
  seedProject(808);
  const output = await call("get_fix_prompts", { url: URL });
  assert.doesNotMatch(output, /undefined/);
});

test("get_fix_prompts applies the requested limit to repeated occurrences of one check", async () => {
  seedProject(809);
  for (let index = 0; index < 100; index += 1) {
    addWorkItem({
      projectId: 809,
      envUrl: URL,
      signalId: `prompt-occurrence-${index}`,
      checkId: "seo.repeated",
      title: `Repeated prompt ${index}`,
      fixPrompt: `Fix occurrence ${index}`,
    });
  }
  const output = await call("get_fix_prompts", { url: URL, check_id: "seo.repeated", limit: 1 });
  assert.match(output, /^1 of 100 fix prompt\(s\)/);
  assert.equal((output.match(/^## Repeated prompt /gm) ?? []).length, 1);
});

test("get_fix_prompts bounds the complete response after escaping large prompts", async () => {
  seedProject(810);
  for (let index = 0; index < 20; index += 1) {
    addWorkItem({
      projectId: 810,
      envUrl: URL,
      signalId: `large-prompt-${index}`,
      checkId: "seo.large-prompt",
      title: `Large prompt ${index}`,
      fixPrompt: "<&>".repeat(10000),
    });
  }
  const output = await call("get_fix_prompts", { url: URL, limit: 20 });
  assert.ok(output.length < 65000, `response contains ${output.length} characters`);
  assert.match(output, /shortened/);
  assert.ok(output.endsWith("</sitecmd_untrusted_scan_data>"));
  assert.equal((output.match(/<\/sitecmd_untrusted_scan_data>/g) ?? []).length, 1);
});

test("get_scan_score prints one SiteCMD Score and mentions the web scan grade in one clause", async () => {
  seedProject(805);
  seedWebScan(805, URL, 62, "2026-08-15T09:00:00.000Z");
  fixtureDb
    .prepare(
      `INSERT INTO score_snapshots (project_id, environment_url, overall, critical_count, high_count, medium_count, low_count, exploitable_capped, computed_at)
       VALUES (?, ?, 58, 0, 3, 13, 45, 0, ?)`,
    )
    .run(805, URL, Date.now());
  const output = await call("get_scan_score", { url: URL });
  assert.match(output, /SiteCMD Score: 58\/100/);
  assert.doesNotMatch(output, /Live SiteCMD score/);
  assert.doesNotMatch(output, /scan artifact score/);
});
