import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";

import { ensurePinnedClone, removeRunWorkspace, validateBenchmarkConfig } from "./workspaces.mjs";

const REF = "179ae84efec61b14206d0305d941daed6c6d07f9";
const REPO = "https://github.com/hagopj13/node-express-boilerplate.git";

function config(overrides = {}) {
  return {
    model: "claude-opus-4-8",
    maxTurns: 60,
    repeats: 1,
    arms: ["blind", "categories", "brief"],
    targets: [{ name: "node-express-boilerplate", repo: REPO, ref: REF }],
    ...overrides,
  };
}

test("benchmark config rejects paths, unknown arms, and floating refs", () => {
  assert.throws(
    () =>
      validateBenchmarkConfig(
        config({ targets: [{ name: "../../outside", repo: REPO, ref: REF }] }),
      ),
    /target name/,
  );
  assert.throws(() => validateBenchmarkConfig(config({ arms: ["../../outside"] })), /arm/);
  assert.throws(
    () =>
      validateBenchmarkConfig(config({ targets: [{ name: "floating", repo: REPO, ref: null }] })),
    /40-character commit SHA/,
  );
  assert.throws(
    () =>
      validateBenchmarkConfig(
        config({
          targets: [
            {
              name: "credentialed",
              repo: "https://token@github.com/example/project.git",
              ref: REF,
            },
          ],
        }),
      ),
    /public GitHub HTTPS URL/,
  );
  assert.throws(
    () =>
      validateBenchmarkConfig(
        config({
          targets: [
            {
              name: "alternate-port",
              repo: "https://github.com:444/example/project.git",
              ref: REF,
            },
          ],
        }),
      ),
    /public GitHub HTTPS URL/,
  );
});

test("recursive cleanup cannot delete outside its run root", () => {
  const outer = mkdtempSync(path.join(tmpdir(), "sitecmd-benchmark-cleanup-test-"));
  try {
    const root = path.join(outer, "runs");
    const inside = path.join(root, "run-1");
    const outside = path.join(outer, "outside");
    mkdirSync(inside, { recursive: true });
    mkdirSync(outside);

    assert.throws(() => removeRunWorkspace(root, outside), /escapes its workspace/);
    assert.equal(existsSync(outside), true);

    removeRunWorkspace(root, inside);
    assert.equal(existsSync(inside), false);
  } finally {
    rmSync(outer, { recursive: true, force: true });
  }
});

test("cached clones always fetch and reset to the configured commit", () => {
  const root = mkdtempSync(path.join(tmpdir(), "sitecmd-benchmark-test-"));
  try {
    const target = config().targets[0];
    const destination = path.join(realpathSync(root), target.name);
    mkdirSync(path.join(destination, ".git"), { recursive: true });
    const calls = [];
    const runGit = (args, cwd) => {
      calls.push({ args, cwd });
      if (args.join(" ") === "remote get-url origin") return REPO;
      if (args.join(" ") === "rev-parse HEAD") return REF;
      return "";
    };

    const resolved = ensurePinnedClone({ target, reposRoot: root, runGit });

    assert.equal(resolved.dest, destination);
    assert.equal(resolved.sha, REF);
    assert.equal(
      calls.some(({ args }) => args.includes("fetch") && args.includes(REF)),
      true,
    );
    assert.equal(
      calls.some(({ args }) => args.includes("checkout") && args.includes(REF)),
      true,
    );
    assert.equal(
      calls.some(({ args }) => args.includes("clean")),
      true,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("cached clones reject an origin changed behind a reused name", () => {
  const root = mkdtempSync(path.join(tmpdir(), "sitecmd-benchmark-test-"));
  try {
    const target = config().targets[0];
    const destination = path.join(realpathSync(root), target.name);
    mkdirSync(path.join(destination, ".git"), { recursive: true });
    assert.throws(
      () =>
        ensurePinnedClone({
          target,
          reposRoot: root,
          runGit: (args) =>
            args.join(" ") === "remote get-url origin"
              ? "https://github.com/example/other.git"
              : "",
        }),
      /cached origin/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
