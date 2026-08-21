import test from "node:test";
import assert from "node:assert/strict";

import { makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-stale-schema-");
const { addWorkItem } = makeSeeders(fixtureDb);

import {
  getActiveIssueGroupsEnriched,
  getCodeScanHistoryForProject,
  getDismissedIssues,
  getEffectiveTier,
  getRecentEvents,
} from "../dist/db.js";

test("missing current-schema tables fail clearly instead of returning false-empty data", () => {
  addWorkItem({ projectId: 1, checkId: "security.hsts" });
  fixtureDb.pragma("foreign_keys = OFF");

  fixtureDb.exec("DROP TABLE scan_runs");
  assert.throws(
    () => getCodeScanHistoryForProject(1, "https://example.com"),
    /no such table: scan_runs/i,
  );

  fixtureDb.exec("DROP TABLE project_issue_states");
  assert.throws(
    () => getDismissedIssues(1, "https://example.com"),
    /no such table: project_issue_states/i,
  );

  fixtureDb.exec("DROP TABLE site_event_check_ids");
  assert.throws(() => getRecentEvents(1, 30), /no such table: site_event_check_ids/i);
  assert.throws(() => getActiveIssueGroupsEnriched(1, []), /no such table: site_event_check_ids/i);

  fixtureDb.exec("DROP TABLE license_state");
  assert.throws(() => getEffectiveTier(), /no such table: license_state/i);
});
