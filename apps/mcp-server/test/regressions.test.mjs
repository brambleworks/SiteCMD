import test from "node:test";
import assert from "node:assert/strict";

import { listHistoricalRegressionsForCheckIds } from "../dist/db.js";
import { ensureProject, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

// db.js resolves SITECMD_DB_PATH lazily at first query, so seeding the
// fixture (which sets the env var) after the static imports is safe.
const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-regressions-");
ensureProject(fixtureDb, 1);

const regressionId = Number(
  fixtureDb
    .prepare(
      `INSERT INTO regressions (
        project_id, env_url, scan_type, prev_run_id, run_id,
        prev_score, score, commit_from, commit_to, commit_count,
        commits_json, fixed_check_ids_json, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      1,
      "https://example.com",
      "web",
      10,
      11,
      92,
      84,
      "abc1234",
      "def5678",
      3,
      "[]",
      "[]",
      1770000000000,
    ).lastInsertRowid,
);

fixtureDb
  .prepare("INSERT INTO regression_check_ids (regression_id, check_id) VALUES (?, ?)")
  .run(regressionId, "security.csp-header");

test("returns regressions matching the given check_ids", () => {
  const rows = listHistoricalRegressionsForCheckIds(1, ["security.csp-header", "unrelated.check"]);

  assert.equal(rows.length, 1);
  assert.equal(rows[0].checkId, "security.csp-header");
  assert.equal(rows[0].scoreDrop, 8);
  // created_at 1770000000000 ms pinned as the exact ISO string.
  assert.equal(rows[0].deployTimestamp, "2026-02-02T02:40:00.000Z");
});

test("returns [] when no check_ids match, scoped to the project", () => {
  assert.deepEqual(listHistoricalRegressionsForCheckIds(1, ["unrelated.check"]), []);
  // Same check_id but a different project must not leak across projects.
  assert.deepEqual(listHistoricalRegressionsForCheckIds(2, ["security.csp-header"]), []);
});

test("returns [] for an empty checkIds input", () => {
  assert.deepEqual(listHistoricalRegressionsForCheckIds(1, []), []);
});

test("orders multiple regressions for the same check_id newest-first", () => {
  const newerRegressionId = Number(
    fixtureDb
      .prepare(
        `INSERT INTO regressions (
          project_id, env_url, scan_type, prev_run_id, run_id,
          prev_score, score, commit_from, commit_to, commit_count,
          commits_json, fixed_check_ids_json, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .run(
        1,
        "https://example.com",
        "web",
        42,
        43,
        90,
        80,
        "def5678",
        "0a1b2c3",
        2,
        "[]",
        "[]",
        1770000100000,
      ).lastInsertRowid,
  );

  fixtureDb
    .prepare("INSERT INTO regression_check_ids (regression_id, check_id) VALUES (?, ?)")
    .run(newerRegressionId, "security.csp-header");

  const rows = listHistoricalRegressionsForCheckIds(1, ["security.csp-header"]);

  assert.equal(rows.length, 2);
  // ORDER BY created_at DESC: the newer regression must come first.
  assert.equal(rows[0].deployTimestamp, "2026-02-02T02:41:40.000Z");
  assert.equal(rows[0].scoreDrop, 10);
  assert.equal(rows[1].deployTimestamp, "2026-02-02T02:40:00.000Z");
  assert.equal(rows[1].scoreDrop, 8);
});
