import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const SCRIPT = path.join(ROOT, "tools/scripts/check-budget-ratchet.mjs");

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8" });
}

function makeRepo() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "budget-ratchet-"));
  git(dir, "init", "--quiet");
  git(dir, "config", "user.email", "test@example.com");
  git(dir, "config", "user.name", "Test");
  fs.mkdirSync(path.join(dir, "tools", "scripts", "lib"), { recursive: true });
  return dir;
}

function write(repoDir, relativePath, contents) {
  const full = path.join(repoDir, relativePath);
  fs.mkdirSync(path.dirname(full), { recursive: true });
  fs.writeFileSync(full, contents);
}

function commit(repoDir, message) {
  git(repoDir, "add", "-A");
  git(repoDir, "commit", "--quiet", "-m", message);
}

function stageOnly(repoDir, relativePath) {
  git(repoDir, "add", relativePath);
}

function runScript(repoDir, args = ["--staged"]) {
  return spawnSync("node", [SCRIPT, ...args], { cwd: repoDir, encoding: "utf8" });
}

function setCommitMessage(repoDir, message) {
  const file = path.join(repoDir, ".git", "COMMIT_EDITMSG");
  fs.writeFileSync(file, message);
}

const SAMPLE_BUDGET_FILE = `export const guardrailScriptLineBudgets = new Map([
  ["tools/scripts/check-repo-guardrails.mjs", 1800],
  ["tools/scripts/lib/guardrail-rust-rules.mjs", 120],
]);
`;

describe("check-budget-ratchet", () => {
  let repo;

  beforeEach(() => {
    repo = makeRepo();
    write(repo, "tools/scripts/lib/guardrail-script-budgets.mjs", SAMPLE_BUDGET_FILE);
    commit(repo, "initial budgets");
  });

  afterEach(() => {
    fs.rmSync(repo, { recursive: true, force: true });
  });

  it("passes when staged budget file has no threshold changes", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE + "// trailing comment, no threshold change\n",
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    const result = runScript(repo);
    expect(result.status).toBe(0);
    expect(result.stderr).toBe("");
  });

  it("passes when a threshold DECREASES (ratcheting tighter)", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 100',
      ),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    const result = runScript(repo);
    expect(result.status).toBe(0);
  });

  it("FAILS when a threshold value is raised", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 220',
      ),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    setCommitMessage(repo, "Refactor the scanner scheduler\n");
    const result = runScript(repo);
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/refusing to raise guardrail thresholds/);
    expect(result.stderr).toMatch(/guardrail-rust-rules\.mjs.*120 -> 220/);
  });

  it("FAILS when a new threshold entry is added", () => {
    const withNewOverride = SAMPLE_BUDGET_FILE.replace(
      '["tools/scripts/lib/guardrail-rust-rules.mjs", 120],',
      '["tools/scripts/lib/guardrail-rust-rules.mjs", 120],\n  ["tools/scripts/lib/guardrail-new-thing.mjs", 999],',
    );
    write(repo, "tools/scripts/lib/guardrail-script-budgets.mjs", withNewOverride);
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    setCommitMessage(repo, "wip\n");
    const result = runScript(repo);
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/new threshold/);
    expect(result.stderr).toMatch(/guardrail-new-thing\.mjs/);
  });

  it("does NOT accept the bypass token when it's the documentation placeholder", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 220',
      ),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    setCommitMessage(
      repo,
      "Document the budget override\n\nUse [budget-raised: <reason>] to allow legitimate raises.\n",
    );
    const result = runScript(repo);
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/refusing to raise/);
  });

  it("does NOT accept the bypass token when it appears mid-paragraph (prose, not intent)", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 220',
      ),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    setCommitMessage(
      repo,
      "Split the scanner scheduler\n\nThe phrase [budget-raised: example phrase] should NOT bypass the check when buried in prose.\n",
    );
    const result = runScript(repo);
    expect(result.status).toBe(1);
  });

  it("allows a raise when the commit message contains the bypass token + issue ref", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 220',
      ),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    setCommitMessage(
      repo,
      "refactor(guardrails): legitimate raise\n\n[budget-raised: needed to land the new ratchet helpers; split deferred (#42)]\n",
    );
    const result = runScript(repo);
    expect(result.status).toBe(0);
    expect(result.stderr).toMatch(/authorized via \[budget-raised:\]/);
  });

  it("REJECTS a bypass token that lacks an issue or PR reference", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 220',
      ),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    setCommitMessage(
      repo,
      "refactor(guardrails): raise\n\n[budget-raised: temporarily widening to land a refactor]\n",
    );
    const result = runScript(repo);
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/tracked issue or PR/);
  });

  it("accepts alternate issue-reference shapes (PR-123, GH-9, fixes #1)", () => {
    write(
      repo,
      "tools/scripts/lib/guardrail-script-budgets.mjs",
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 220',
      ),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-script-budgets.mjs");
    setCommitMessage(
      repo,
      "raise: bigger budget\n\n[budget-raised: needs follow-up split, see PR-42]\n",
    );
    const result = runScript(repo);
    expect(result.status).toBe(0);
  });

  it("ignores staged files that aren't budget files", () => {
    write(repo, "some/other/file.mjs", "export const X = 999;\n");
    stageOnly(repo, "some/other/file.mjs");
    const result = runScript(repo);
    expect(result.status).toBe(0);
    expect(result.stderr).toBe("");
  });

  it("detects raises in the maxLines object-property shape", () => {
    const objectBudgetFile = `const sourceSizeBudgets = [
  { file: "apps/desktop/src/foo.tsx", maxLines: 800 },
];
`;
    write(repo, "tools/scripts/check-repo-guardrails.mjs", objectBudgetFile);
    commit(repo, "initial check-repo-guardrails");
    write(
      repo,
      "tools/scripts/check-repo-guardrails.mjs",
      objectBudgetFile.replace("maxLines: 800", "maxLines: 1500"),
    );
    stageOnly(repo, "tools/scripts/check-repo-guardrails.mjs");
    setCommitMessage(repo, "wip\n");
    const result = runScript(repo);
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/foo\.tsx.*800 -> 1500/);
  });

  it("detects raises in top-level *_LIMIT constants", () => {
    const constFile = `const RUST_FILE_LINE_LIMIT = 800;
export function rustLineBudgetFailures() {}
`;
    write(repo, "tools/scripts/lib/guardrail-rust-loc-rules.mjs", constFile);
    commit(repo, "initial loc rules");
    write(
      repo,
      "tools/scripts/lib/guardrail-rust-loc-rules.mjs",
      constFile.replace("RUST_FILE_LINE_LIMIT = 800", "RUST_FILE_LINE_LIMIT = 1200"),
    );
    stageOnly(repo, "tools/scripts/lib/guardrail-rust-loc-rules.mjs");
    setCommitMessage(repo, "wip\n");
    const result = runScript(repo);
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/const:RUST_FILE_LINE_LIMIT.*800 -> 1200/);
  });

  it("supports a --range mode for CI hard-gate checks", () => {
    const tipPath = "tools/scripts/lib/guardrail-script-budgets.mjs";
    write(
      repo,
      tipPath,
      SAMPLE_BUDGET_FILE.replace(
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 120',
        '"tools/scripts/lib/guardrail-rust-rules.mjs", 999',
      ),
    );
    commit(repo, "bumped rust-rules budget");
    const result = runScript(repo, ["--range", "HEAD~1..HEAD"]);
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/120 -> 999/);
  });
});
