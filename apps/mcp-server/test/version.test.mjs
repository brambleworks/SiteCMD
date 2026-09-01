import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { assertSchemaContract } from "../scripts/lib/schema-contract.mjs";
import { MCP_SERVER_VERSION, SUPPORTED_SCHEMA_VERSIONS } from "../dist/version.js";
import { latestMigrationVersion } from "./helpers/schema-fixture.mjs";

test("the bundled MCP handshake follows the SiteCMD release version", async () => {
  const packageJson = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );
  const desktopManifest = await readFile(
    new URL("../../desktop/src-tauri/Cargo.toml", import.meta.url),
    "utf8",
  );
  const desktopVersion = desktopManifest.match(/^version = "([^"]+)"/m)?.[1];

  assert.equal(packageJson.version, desktopVersion);
  assert.equal(MCP_SERVER_VERSION, desktopVersion);
});

test("the supported schema range tops out at the desktop's latest migration", () => {
  assert.equal(SUPPORTED_SCHEMA_VERSIONS.max, latestMigrationVersion());
  assert.ok(SUPPORTED_SCHEMA_VERSIONS.min <= SUPPORTED_SCHEMA_VERSIONS.max);
});

test("the bundle guard rejects an unreviewed desktop migration", () => {
  const migrations = `
    (27, include_str!("migrations/027_agent_requests.sql")),
    (28, include_str!("migrations/028_cleanup.sql")),
  `;
  const version = "export const SUPPORTED_SCHEMA_VERSIONS = { min: 26, max: 27 } as const;";

  assert.throws(
    () => assertSchemaContract(migrations, version),
    /desktop migration 28 is registered, but the MCP server supports through 27/,
  );
});
