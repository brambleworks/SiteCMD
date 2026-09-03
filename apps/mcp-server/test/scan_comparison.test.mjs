import test from "node:test";
import assert from "node:assert/strict";

import { __test_resetScanHistoryLoads, __test_scanHistoryLoads } from "../dist/db.js";
import { connectInMemory } from "./tools_list_snapshot.test.mjs";
import { ensureProject, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-scan-comparison-");
const PROJECT_ID = 901;
const URL = "https://comparison.test";

/** Mirrors issue_reads.test.mjs's web scan insert, returning the scan id compare_scans takes. */
function seedWebScan(score, timestamp) {
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
        PROJECT_ID,
        URL,
        URL,
        `comparison-${timestamp}`,
        `v1:comparison-${timestamp}`,
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
    .run(executionId, PROJECT_ID, URL, URL, startedAt, startedAt + 1200, timestamp, score);
  return executionId;
}

ensureProject(fixtureDb, PROJECT_ID);
fixtureDb
  .prepare(
    `INSERT INTO environments (project_id, url, label, environment)
     VALUES (?, ?, 'Production', 'production')`,
  )
  .run(PROJECT_ID, URL);
const OLDEST = seedWebScan(61, "2026-08-19T09:00:00.000Z");
const MIDDLE = seedWebScan(72, "2026-08-20T09:00:00.000Z");
const NEWEST = seedWebScan(88, "2026-08-21T09:00:00.000Z");

/** Returns the tool output plus how many times it loaded scan history. */
async function compareScans(args) {
  const session = await connectInMemory();
  try {
    __test_resetScanHistoryLoads();
    const result = await session.client.callTool({ name: "compare_scans", arguments: args });
    return { text: result.content[0].text, loads: __test_scanHistoryLoads() };
  } finally {
    await session.close();
  }
}

test("comparing the two most recent scans loads history once", async () => {
  const { text, loads } = await compareScans({ url: URL });
  assert.equal(loads, 1);
  assert.match(
    text,
    new RegExp(`\\*\\*Scans:\\*\\* #${MIDDLE} \\(2026-08-20\\) to #${NEWEST} \\(2026-08-21\\)`),
  );
  assert.match(text, /\*\*Web scan score:\*\* 72\/100 to 88\/100 \(\+16 pts\)/);
});

// Each explicit id used to re-run the history query, and every history row
// aggregates findings with correlated subqueries, so this was three loads.
test("explicit scan ids resolve from the same single history load", async () => {
  const { text, loads } = await compareScans({
    url: URL,
    from_scan_id: OLDEST,
    to_scan_id: MIDDLE,
  });
  assert.equal(loads, 1);
  assert.match(
    text,
    new RegExp(`\\*\\*Scans:\\*\\* #${OLDEST} \\(2026-08-19\\) to #${MIDDLE} \\(2026-08-20\\)`),
  );
  assert.match(text, /\*\*Web scan score:\*\* 61\/100 to 72\/100 \(\+11 pts\)/);
});

test("one explicit scan id still loads history once", async () => {
  const { text, loads } = await compareScans({ url: URL, to_scan_id: MIDDLE });
  assert.equal(loads, 1);
  assert.match(text, new RegExp(`to #${MIDDLE} \\(2026-08-20\\)`));
});

test("an unknown scan id reports the id guidance without a second load", async () => {
  const { text, loads } = await compareScans({ url: URL, to_scan_id: NEWEST + 1000 });
  assert.equal(loads, 1);
  assert.match(text, /Could not find both scans/);
});
