import test from "node:test";
import assert from "node:assert/strict";

import { getLiveScore } from "../dist/db.js";
import { ensureProject, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

// db.js resolves SITECMD_DB_PATH lazily at first query, so seeding the fixture
// (which sets the env var) after the static imports is safe.
const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-livescore-");

function seedEnvironment(projectId, url) {
  ensureProject(fixtureDb, projectId);
  fixtureDb
    .prepare(
      `INSERT OR IGNORE INTO environments (project_id, url, label, environment)
       VALUES (?, ?, 'Production', 'production')`,
    )
    .run(projectId, url);
}

function seedSnapshot(projectId, envUrl, overrides = {}) {
  fixtureDb
    .prepare(
      `INSERT INTO score_snapshots (
         project_id, environment_url, overall, critical_count, high_count,
         medium_count, low_count, exploitable_capped, computed_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      projectId,
      envUrl,
      overrides.overall ?? 90,
      overrides.critical ?? 0,
      overrides.high ?? 0,
      overrides.medium ?? 0,
      overrides.low ?? 0,
      overrides.capped ?? 0,
      overrides.computedAt ?? Date.parse("2026-07-20T00:00:00.000Z"),
    );
}

test("getLiveScore returns null when the project has no snapshot yet", () => {
  seedEnvironment(1, "https://no-snapshot.test");
  assert.equal(getLiveScore("https://no-snapshot.test"), null);
});

test("getLiveScore returns null for a URL that maps to no project", () => {
  assert.equal(getLiveScore("https://unknown-site.test"), null);
});

test("getLiveScore returns the newest snapshot for the URL's project", () => {
  seedEnvironment(2, "https://live.test");
  seedSnapshot(2, "https://live.test", {
    overall: 55,
    high: 3,
    computedAt: Date.parse("2026-07-19T00:00:00.000Z"),
  });
  // A later, higher score is the newest row (ordered by id, not computed_at).
  seedSnapshot(2, "https://live.test", {
    overall: 72,
    critical: 0,
    high: 1,
    medium: 2,
    low: 4,
    capped: 1,
    computedAt: Date.parse("2026-07-20T00:00:00.000Z"),
  });

  const score = getLiveScore("https://live.test");
  assert.ok(score, "expected a live score for a project with snapshots");
  assert.equal(score.overall, 72);
  assert.equal(score.high_count, 1);
  assert.equal(score.medium_count, 2);
  assert.equal(score.low_count, 4);
  assert.equal(score.exploitable_capped, true);
});

test("getLiveScore matches the trailing-slash URL variant", () => {
  seedEnvironment(3, "https://slashed.test");
  seedSnapshot(3, "https://slashed.test", { overall: 88 });

  // The stored env_url has no trailing slash; a caller passing one still resolves.
  const score = getLiveScore("https://slashed.test/");
  assert.ok(score);
  assert.equal(score.overall, 88);
});

test("getLiveScore does not bleed one project's snapshot into another", () => {
  seedEnvironment(4, "https://project-four.test");
  seedEnvironment(5, "https://project-five.test");
  seedSnapshot(4, "https://project-four.test", { overall: 40 });

  // Project 5 has an environment but no snapshot of its own.
  assert.equal(getLiveScore("https://project-five.test"), null);
  assert.equal(getLiveScore("https://project-four.test").overall, 40);
});
