import assert from "node:assert/strict";
import { test } from "node:test";
import { join } from "node:path";

import { resolveDbPath } from "../dist/db.js";

test("an explicit SiteCMD database path overrides every platform default", () => {
  assert.equal(
    resolveDbPath("linux", { SITECMD_DB_PATH: "/exact/sitecmd.db" }, "/home/tester"),
    "/exact/sitecmd.db",
  );
});

test("Windows matches the desktop LOCALAPPDATA then APPDATA authority", () => {
  assert.equal(
    resolveDbPath(
      "win32",
      { LOCALAPPDATA: "/windows/local", APPDATA: "/windows/roaming" },
      "/home/tester",
    ),
    join("/windows/local", "com.sitecmd.app", "sitecmd.db"),
  );
  assert.equal(
    resolveDbPath("win32", { APPDATA: "/windows/roaming" }, "/home/tester"),
    join("/windows/roaming", "com.sitecmd.app", "sitecmd.db"),
  );
});

test("Linux matches the desktop XDG_DATA_HOME then home fallback authority", () => {
  assert.equal(
    resolveDbPath("linux", { XDG_DATA_HOME: "/xdg/data" }, "/home/tester"),
    join("/xdg/data", "com.sitecmd.app", "sitecmd.db"),
  );
  assert.equal(
    resolveDbPath("linux", {}, "/home/tester"),
    join("/home/tester", ".local", "share", "com.sitecmd.app", "sitecmd.db"),
  );
});
