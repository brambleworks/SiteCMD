import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import Database from "better-sqlite3";
import { openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixture = openSchemaFixtureDb("sitecmd-mcp-health-");
const __dirname = dirname(fileURLToPath(import.meta.url));
const entrypoint = join(__dirname, "..", "dist", "index.js");

test("health-check mode opens the configured database read-only and exits", () => {
  const result = spawnSync(
    process.execPath,
    ["--disable-warning=ExperimentalWarning", entrypoint, "--sitecmd-health-check"],
    {
      encoding: "utf8",
      env: { ...process.env, SITECMD_DB_PATH: fixture.name },
      timeout: 10_000,
    },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), {
    marker: "SITECMD_MCP_HEALTH_V1",
    ok: true,
  });
  assert.equal(result.stderr, "");
});

test("health-check mode fails when the configured database cannot open", () => {
  const result = spawnSync(
    process.execPath,
    ["--disable-warning=ExperimentalWarning", entrypoint, "--sitecmd-health-check"],
    {
      encoding: "utf8",
      env: { ...process.env, SITECMD_DB_PATH: join(__dirname, "missing-sitecmd.db") },
      timeout: 10_000,
    },
  );

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /SiteCMD database not found/);
  assert.equal(result.stdout, "");
});

test("health-check mode rejects a readable non-SiteCMD SQLite database", () => {
  const dir = mkdtempSync(join(tmpdir(), "sitecmd-mcp-wrong-schema-"));
  const dbPath = join(dir, "not-sitecmd.db");
  new Database(dbPath).close();

  try {
    const result = spawnSync(
      process.execPath,
      ["--disable-warning=ExperimentalWarning", entrypoint, "--sitecmd-health-check"],
      {
        encoding: "utf8",
        env: { ...process.env, SITECMD_DB_PATH: dbPath },
        timeout: 10_000,
      },
    );

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /SiteCMD database schema health query failed/);
    assert.equal(result.stdout, "");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
