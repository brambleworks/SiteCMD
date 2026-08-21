import test from "node:test";
import assert from "node:assert/strict";

import {
  getFixBrief,
  requestVerification,
  listFixAttempts,
  __test_isReadDbReadonly,
} from "../dist/db.js";
import { makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

// db.js resolves SITECMD_DB_PATH lazily at first query, so seeding the
// fixture (which sets the env var) after the static imports is safe.
const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-fixattempts-");
const { addFixAttempt } = makeSeeders(fixtureDb);

function readAttempt(id) {
  return fixtureDb.prepare("SELECT * FROM fix_attempts WHERE id = ?").get(id);
}

test("getFixBrief returns the stored brief and status", () => {
  const id = addFixAttempt({
    checkId: "security.csp",
    briefMd: "# Fix CSP\n\nAdd a Content-Security-Policy header in next.config.ts.",
  });

  const result = getFixBrief(id);

  assert.equal(
    result.briefMd,
    "# Fix CSP\n\nAdd a Content-Security-Policy header in next.config.ts.",
  );
  assert.equal(result.status, "briefed");
});

test("getFixBrief throws an actionable error for an unknown id", () => {
  assert.throws(() => getFixBrief(999999), /No fix attempt with id 999999/);
});

test("getFixBrief stamps brief_fetched_at and advances updated_at on first serve", () => {
  const id = addFixAttempt({ checkId: "stamp.first.fetch", updatedAt: 1000 });
  assert.equal(readAttempt(id).brief_fetched_at, null, "fresh attempts start unstamped");

  getFixBrief(id);

  const row = readAttempt(id);
  assert.ok(
    typeof row.brief_fetched_at === "number" && row.brief_fetched_at > 0,
    `first serve must stamp brief_fetched_at, got ${row.brief_fetched_at}`,
  );
  assert.equal(row.updated_at, row.brief_fetched_at, "the stamp also touches updated_at");
  assert.equal(row.status, "briefed", "the pickup stamp is a column, never a status change");
});

test("getFixBrief does not overwrite brief_fetched_at on a second fetch", () => {
  const id = addFixAttempt({ checkId: "stamp.second.fetch" });
  // Seed a distinctive past stamp; a re-fetch writing Date.now() over it
  // would be caught even when both fetches land in the same millisecond.
  fixtureDb.prepare("UPDATE fix_attempts SET brief_fetched_at = 12345 WHERE id = ?").run(id);

  getFixBrief(id);

  assert.equal(
    readAttempt(id).brief_fetched_at,
    12345,
    "the original pickup time must survive later fetches",
  );
});

test("requestVerification flips briefed to verify_requested and stores the summary", () => {
  const id = addFixAttempt({ checkId: "performance.compression", status: "briefed" });

  requestVerification(id, "Enabled gzip compression in the server config.");

  const row = readAttempt(id);
  assert.equal(row.status, "verify_requested");
  assert.equal(row.agent_summary, "Enabled gzip compression in the server config.");
  assert.ok(typeof row.updated_at === "number" && row.updated_at > 0);
});

test("requestVerification rejects attempts that are already terminal", () => {
  const id = addFixAttempt({ checkId: "seo.title", status: "verified" });

  assert.throws(
    () => requestVerification(id, "Tried again."),
    new RegExp(
      `Fix attempt ${id} is already 'verified'; verification can only be requested ` +
        `while it is 'briefed' or 'verify_requested'\\. ` +
        `Ask the user to start a new fix attempt from the issue in SiteCMD\\.`,
    ),
  );

  const row = readAttempt(id);
  assert.equal(row.status, "verified", "terminal status must not change");
});

test("requestVerification is idempotent for a row already in verify_requested", () => {
  const id = addFixAttempt({
    checkId: "security.referrer",
    status: "verify_requested",
    agentSummary: "First summary.",
  });

  // A retry (e.g. the agent's first call timed out client-side) must not throw
  // and must keep the latest summary.
  requestVerification(id, "Second summary after retry.");

  const row = readAttempt(id);
  assert.equal(row.status, "verify_requested");
  assert.equal(row.agent_summary, "Second summary after retry.");
});

test("requestVerification advances updated_at so the expiry sweep does not reap it", () => {
  const id = addFixAttempt({
    checkId: "performance.caching",
    status: "briefed",
    updatedAt: 1000,
  });

  requestVerification(id, "Added cache-control headers.");

  const row = readAttempt(id);
  assert.ok(
    row.updated_at > 1000,
    `updated_at must move past the seeded value, got ${row.updated_at}`,
  );
});

test("requestVerification throws when a concurrent cancel already won the race", () => {
  const id = addFixAttempt({ checkId: "security.xframe", status: "canceled" });

  assert.throws(
    () => requestVerification(id, "Set X-Frame-Options."),
    /already 'canceled'.*start a new fix attempt/,
  );

  const row = readAttempt(id);
  assert.equal(row.status, "canceled", "lost race must not overwrite the cancel");
  assert.equal(row.agent_summary, null, "summary must not be written on a lost race");
});

test("listFixAttempts returns active attempts only", () => {
  const activeId = addFixAttempt({ checkId: "list.active.check", status: "briefed" });
  const expiredId = addFixAttempt({ checkId: "list.expired.check", status: "expired" });

  const attempts = listFixAttempts();

  assert.ok(
    attempts.some((a) => a.id === activeId && a.checkId === "list.active.check"),
    "briefed attempt should be listed",
  );
  assert.ok(!attempts.some((a) => a.id === expiredId), "expired attempt should not be listed");
  for (const a of attempts) {
    assert.ok(
      a.status === "briefed" || a.status === "verify_requested" || a.status === "verifying",
      `only active statuses should be listed, got '${a.status}'`,
    );
    assert.equal(typeof a.id, "number");
    assert.equal(typeof a.projectId, "number");
    assert.equal(typeof a.checkId, "string");
    assert.equal(typeof a.agentTool, "string");
    assert.equal(typeof a.createdAt, "number");
  }
});

test("request_verification produces the exact status the desktop watcher polls", () => {
  const id = addFixAttempt({ checkId: "contract.pin.check", status: "briefed" });

  requestVerification(id, "Contract pin summary.");

  const row = readAttempt(id);
  assert.equal(row.status, "verify_requested");
});

test("the shared read connection stays readonly", () => {
  // Fix-attempt writes go through a dedicated write connection; every other
  // query path must keep using the readonly connection.
  assert.equal(__test_isReadDbReadonly(), true);
});
