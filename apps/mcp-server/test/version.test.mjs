import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

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
