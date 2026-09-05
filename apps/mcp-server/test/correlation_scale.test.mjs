import test from "node:test";
import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";

import { getActiveIssueGroupsEnriched } from "../dist/db.js";
import { ensureProject, makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const db = openSchemaFixtureDb("sitecmd-mcp-correlation-scale-");
const { addWorkItem } = makeSeeders(db);

test("cross-environment correlation stays responsive with thousands of occurrences", () => {
  const projectId = 2001;
  const checkId = "security.hsts";
  const staging = "https://staging.example.com";
  const production = "https://example.com";
  const firstSeen = Date.parse("2026-08-01T00:00:00Z");
  const day = 24 * 60 * 60 * 1000;
  ensureProject(db, projectId);
  const environment = db.prepare(
    "INSERT INTO environments (project_id, url, label, environment) VALUES (?, ?, ?, ?)",
  );
  environment.run(projectId, staging, "Staging", "staging");
  environment.run(projectId, production, "Production", "production");
  db.transaction(() => {
    for (let index = 0; index < 4000; index += 1) {
      for (const [envUrl, offset] of [
        [staging, 0],
        [production, 7 * day],
      ]) {
        addWorkItem({
          projectId,
          checkId,
          envUrl,
          signalId: `${envUrl}:${index}`,
          firstSeenAt: firstSeen + offset + index,
        });
      }
    }
    addWorkItem({
      projectId,
      checkId,
      envUrl: staging,
      signalId: "old-staging",
      firstSeenAt: firstSeen - day,
      resolvedAt: firstSeen,
    });
    addWorkItem({
      projectId,
      checkId,
      envUrl: production,
      signalId: "old-production",
      firstSeenAt: firstSeen - 2 * day,
      resolvedAt: firstSeen,
    });
  })();

  const started = performance.now();
  const groups = getActiveIssueGroupsEnriched(projectId, []);
  const elapsed = performance.now() - started;
  assert.deepEqual(groups[0].crossEnvSignal, {
    stagingObservedAt: new Date(firstSeen - day).toISOString(),
    daysBeforeProd: 8,
  });
  assert.ok(elapsed < 2000, `correlating 8,000 occurrences took ${elapsed.toFixed(0)}ms`);
});
