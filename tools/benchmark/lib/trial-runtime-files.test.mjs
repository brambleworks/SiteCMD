import assert from "node:assert/strict";
import { test } from "node:test";
import { captureRuntimeFiles, withoutRuntimeFiles } from "../guest/trial-runtime-files.mjs";

test("only unchanged empty protection files from client initialization are omitted", () => {
  const original = { "package.json": "{}", "app.mjs": "original" };
  const startup = {
    files: {
      "package.json": Buffer.from("{}"),
      "app.mjs": Buffer.from("original"),
      ".env": Buffer.alloc(0),
    },
    violations: [],
  };
  const runtime = captureRuntimeFiles(original, startup);
  assert.deepEqual(runtime, [".env"]);
  const candidate = { ...startup.files, "app.mjs": Buffer.from("fixed") };
  assert.deepEqual(Object.keys(withoutRuntimeFiles(candidate, runtime)), [
    "package.json",
    "app.mjs",
  ]);
  assert.equal(
    withoutRuntimeFiles({ ...candidate, ".env": Buffer.from("changed") }, runtime)[
      ".env"
    ].toString(),
    "changed",
  );
});

test("initialization cannot bless changed source, unknown files, contents or links", () => {
  const original = { "app.mjs": "original" };
  for (const snapshot of [
    { files: { "app.mjs": Buffer.from("changed") }, violations: [] },
    { files: {}, violations: [] },
    {
      files: { "app.mjs": Buffer.from("original"), ".sitecmd/config.json": Buffer.alloc(0) },
      violations: [],
    },
    {
      files: { "app.mjs": Buffer.from("original"), ".env": Buffer.from("SECRET=value") },
      violations: [],
    },
    { files: { "app.mjs": Buffer.from("original") }, violations: ["Symlink .env"] },
  ])
    assert.throws(() => captureRuntimeFiles(original, snapshot), /initialization/);
});
