import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";

import { getProjects, withBusyRetry, __test_readBusyTimeout } from "../dist/db.js";
import { ensureProject, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-busy-");
ensureProject(fixtureDb, 1);

const HOLD_LOCK = `
  const { DatabaseSync } = require("node:sqlite");
  const db = new DatabaseSync(process.env.SITECMD_DB_PATH);
  db.exec("BEGIN EXCLUSIVE");
  process.stdout.write("locked\\n");
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 400);
  db.exec("COMMIT");
`;

test("the read connection waits out a writer instead of failing with database is locked", async () => {
  const child = spawn(process.execPath, ["-e", HOLD_LOCK], {
    env: process.env,
    stdio: ["ignore", "pipe", "inherit"],
  });
  await new Promise((resolve) => child.stdout.once("data", resolve));
  const started = Date.now();
  const projects = getProjects();
  assert.ok(projects.some((project) => project.id === 1));
  assert.ok(
    Date.now() - started >= 250,
    "the read must have waited for the exclusive lock to clear",
  );
  await new Promise((resolve) => child.once("exit", resolve));
});

test("withBusyRetry retries exactly once on SQLITE_BUSY", () => {
  let calls = 0;
  const value = withBusyRetry(() => {
    calls += 1;
    if (calls === 1)
      throw Object.assign(new Error("database is locked"), {
        code: "ERR_SQLITE_ERROR",
        errcode: 5,
      });
    return "ok";
  });
  assert.equal(value, "ok");
  assert.equal(calls, 2);
  assert.throws(
    () =>
      withBusyRetry(() => {
        throw Object.assign(new Error("locked"), { errcode: 5 });
      }),
    /locked/,
  );
});

test("the read connection declares a busy timeout", () => {
  assert.equal(__test_readBusyTimeout(), 5000);
});
