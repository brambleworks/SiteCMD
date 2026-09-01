import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import Database from "better-sqlite3";
import { SUPPORTED_SCHEMA_VERSIONS } from "../dist/version.js";
import { openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixture = openSchemaFixtureDb("sitecmd-mcp-health-");
const __dirname = dirname(fileURLToPath(import.meta.url));
const entrypoint = join(__dirname, "..", "dist", "index.js");
const bundledEntrypoint = join(__dirname, "..", "dist-bundle", "sitecmd-mcp.mjs");

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

test("the packaged MCP artifact opens the latest desktop schema", () => {
  const result = spawnSync(
    process.execPath,
    ["--disable-warning=ExperimentalWarning", bundledEntrypoint, "--sitecmd-health-check"],
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
  assert.deepEqual(JSON.parse(result.stdout), {
    marker: "SITECMD_MCP_HEALTH_V1",
    ok: false,
    errorCode: "database_not_found",
  });
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
    assert.deepEqual(JSON.parse(result.stdout), {
      marker: "SITECMD_MCP_HEALTH_V1",
      ok: false,
      errorCode: "invalid_database",
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("health-check mode refuses a database newer than the supported schema range", () => {
  const newer = openSchemaFixtureDb("sitecmd-mcp-health-newer-");
  newer.prepare("INSERT INTO _schema_version (version) VALUES (999)").run();
  const result = spawnSync(
    process.execPath,
    ["--disable-warning=ExperimentalWarning", entrypoint, "--sitecmd-health-check"],
    {
      encoding: "utf8",
      env: { ...process.env, SITECMD_DB_PATH: newer.name },
      timeout: 10_000,
    },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /schema version 999 is newer than this MCP server supports/);
  assert.deepEqual(JSON.parse(result.stdout), {
    marker: "SITECMD_MCP_HEALTH_V1",
    ok: false,
    errorCode: "schema_too_new",
    databaseVersion: 999,
    supportedMin: SUPPORTED_SCHEMA_VERSIONS.min,
    supportedMax: SUPPORTED_SCHEMA_VERSIONS.max,
  });
});

test("health-check mode identifies an older database", () => {
  const older = openSchemaFixtureDb("sitecmd-mcp-health-older-");
  older
    .prepare("DELETE FROM _schema_version WHERE version >= ?")
    .run(SUPPORTED_SCHEMA_VERSIONS.min);
  const result = spawnSync(
    process.execPath,
    ["--disable-warning=ExperimentalWarning", entrypoint, "--sitecmd-health-check"],
    {
      encoding: "utf8",
      env: { ...process.env, SITECMD_DB_PATH: older.name },
      timeout: 10_000,
    },
  );

  assert.notEqual(result.status, 0);
  assert.deepEqual(JSON.parse(result.stdout), {
    marker: "SITECMD_MCP_HEALTH_V1",
    ok: false,
    errorCode: "schema_too_old",
    databaseVersion: SUPPORTED_SCHEMA_VERSIONS.min - 1,
    supportedMin: SUPPORTED_SCHEMA_VERSIONS.min,
    supportedMax: SUPPORTED_SCHEMA_VERSIONS.max,
  });
});

test("health-check mode identifies a missing schema version", () => {
  const unversioned = openSchemaFixtureDb("sitecmd-mcp-health-unversioned-");
  unversioned.exec("DELETE FROM _schema_version");
  const result = spawnSync(
    process.execPath,
    ["--disable-warning=ExperimentalWarning", entrypoint, "--sitecmd-health-check"],
    {
      encoding: "utf8",
      env: { ...process.env, SITECMD_DB_PATH: unversioned.name },
      timeout: 10_000,
    },
  );

  assert.notEqual(result.status, 0);
  assert.deepEqual(JSON.parse(result.stdout), {
    marker: "SITECMD_MCP_HEALTH_V1",
    ok: false,
    errorCode: "schema_version_missing",
    databaseVersion: null,
    supportedMin: SUPPORTED_SCHEMA_VERSIONS.min,
    supportedMax: SUPPORTED_SCHEMA_VERSIONS.max,
  });
});
