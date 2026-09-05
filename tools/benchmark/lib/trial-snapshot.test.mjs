import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { readCandidate, compareCandidate } from "../guest/trial-snapshot.mjs";

test("snapshots retain untracked binary additions and ignore only Git metadata", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "sitecmd-snapshot-"));
  mkdirSync(path.join(directory, ".git"));
  writeFileSync(path.join(directory, ".git", "ignored"), "metadata");
  writeFileSync(path.join(directory, "data.bin"), Buffer.from([0, 255, 5]));
  const result = readCandidate(directory);
  assert.deepEqual(Object.keys(result.files), ["data.bin"]);
  assert.deepEqual(result.files["data.bin"], Buffer.from([0, 255, 5]));
  assert.deepEqual(result.violations, []);
});

test(
  "candidate named pipes cannot block the snapshot controller",
  { skip: process.platform === "win32" },
  () => {
    const directory = mkdtempSync(path.join(tmpdir(), "sitecmd-snapshot-"));
    execFileSync("mkfifo", [path.join(directory, "blocked.pipe")]);
    const source = `import { readCandidate } from ${JSON.stringify(new URL("../guest/trial-snapshot.mjs", import.meta.url).href)}; console.log(JSON.stringify(readCandidate(process.argv[1])));`;
    const result = spawnSync(process.execPath, ["--input-type=module", "-e", source, directory], {
      encoding: "utf8",
      timeout: 1000,
    });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0);
    assert.match(JSON.parse(result.stdout).violations[0], /Non-regular.*blocked.pipe/);
  },
);

test("snapshots do not follow symlinks or accept suppression-only changes", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "sitecmd-snapshot-"));
  symlinkSync("/outside/private", path.join(directory, "escape"));
  const result = readCandidate(directory);
  assert.equal(result.files.escape, undefined);
  assert.ok(result.violations[0].includes("escape"));
  const comparison = compareCandidate(
    { "app.mjs": "ok" },
    { ".sitecmd/config.json": Buffer.from("{}") },
  );
  assert.equal(comparison.passed, false);
});
