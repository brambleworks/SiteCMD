import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import Database from "better-sqlite3";

import { listHistoricalRegressionsForCheckIds } from "../dist/db.js";

// This reduced schema deliberately tests compatibility with older app databases.

const fixtureDir = mkdtempSync(join(tmpdir(), "sitecmd-mcp-regressions-missing-"));
const fixtureDbPath = join(fixtureDir, "sitecmd.db");
process.env.SITECMD_DB_PATH = fixtureDbPath;

const fixtureDb = new Database(fixtureDbPath);
fixtureDb.exec(`
  CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
  );
`);

process.on("exit", () => {
  fixtureDb.close();
  rmSync(fixtureDir, { recursive: true, force: true });
});

test("degrades to [] instead of throwing when the regressions tables are missing", () => {
  assert.deepEqual(listHistoricalRegressionsForCheckIds(1, ["security.csp-header"]), []);
});

test("propagates non-missing-table errors instead of degrading to []", () => {
  fixtureDb.exec(`
    CREATE TABLE regressions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      project_id INTEGER NOT NULL,
      created_at INTEGER NOT NULL,
      score INTEGER NOT NULL
      -- prev_score deliberately missing: the query must fail, not fail open
    );
    CREATE TABLE regression_check_ids (
      regression_id INTEGER NOT NULL,
      check_id TEXT NOT NULL
    );
  `);
  assert.throws(
    () => listHistoricalRegressionsForCheckIds(1, ["security.csp-header"]),
    /no such column/,
  );
});
