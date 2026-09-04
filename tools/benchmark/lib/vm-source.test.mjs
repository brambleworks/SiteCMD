import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { sourceSnapshot } from "./vm-source.mjs";

test("source export includes committed files without Git credentials or local files", (t) => {
  const root = mkdtempSync(path.join(tmpdir(), "sitecmd-source-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  git("init", "--quiet");
  writeFileSync(path.join(root, "package.json"), '{"name":"owned-fixture"}\n');
  git("add", "package.json");
  git(
    "-c",
    "user.name=Benchmark Test",
    "-c",
    "user.email=test@example.invalid",
    "commit",
    "--quiet",
    "-m",
    "Add owned fixture",
  );
  writeFileSync(path.join(root, ".env"), "PRIVATE=do-not-export\n");
  const snapshot = sourceSnapshot(root);
  assert.equal(snapshot.commit, git("rev-parse", "HEAD"));
  const listed = spawnSync("tar", ["-tf", "-"], { input: snapshot.archive, encoding: "utf8" });
  assert.equal(listed.stdout, "package.json\n");
  assert.match(snapshot.sha256, /^[a-f0-9]{64}$/);
});
