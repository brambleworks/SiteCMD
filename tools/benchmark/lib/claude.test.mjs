import assert from "node:assert/strict";
import { test } from "node:test";

import { assertSandboxSupport, buildClaudeInvocation, parseClaudeVersion } from "./claude.mjs";

test("Claude runs with a fail-closed sandbox and no host-level tools", () => {
  const { args, env } = buildClaudeInvocation({
    cwd: "/workspace/run",
    model: "claude-opus-4-8",
    maxTurns: 20,
    environment: {
      HOME: "/Users/tester",
      PATH: "/usr/bin",
      LANG: "en_US.UTF-8",
      SECRET_TOKEN: "sensitive",
    },
  });

  assert.equal(args.includes("bypassPermissions"), false);
  assert.equal(args.includes("--dangerously-skip-permissions"), false);
  assert.equal(args[args.indexOf("--permission-mode") + 1], "dontAsk");
  assert.equal(args[args.indexOf("--tools") + 1], "Bash");
  assert.equal(args[args.indexOf("--setting-sources") + 1], "");
  assert.equal(args.includes("--safe-mode"), true);
  assert.equal(args.includes("--bare"), false);
  assert.equal(args.includes("--no-session-persistence"), true);
  assert.equal(args.includes("--strict-mcp-config"), true);

  const settings = JSON.parse(args[args.indexOf("--settings") + 1]);
  assert.deepEqual(settings.sandbox, {
    enabled: true,
    failIfUnavailable: true,
    autoAllowBashIfSandboxed: true,
    allowUnsandboxedCommands: false,
    filesystem: {
      denyRead: ["~/"],
      allowRead: ["/workspace/run"],
    },
    credentials: {
      envVars: [{ name: "SECRET_TOKEN", mode: "deny" }],
    },
    network: {
      allowedDomains: ["registry.npmjs.org"],
      strictAllowlist: true,
    },
  });
  assert.equal(env.CLAUDE_CODE_DISABLE_AUTO_MEMORY, "1");
  assert.equal(env.CLAUDE_CODE_SKIP_PROMPT_HISTORY, "1");
  assert.equal(env.CLAUDE_CODE_SUBPROCESS_ENV_SCRUB, "1");
  assert.equal(env.DISABLE_AUTOUPDATER, "1");
});

test("Claude sandbox support rejects old versions and native Windows", () => {
  assert.deepEqual(parseClaudeVersion("2.1.238 (Claude Code)"), [2, 1, 238]);
  assert.doesNotThrow(() => assertSandboxSupport("darwin", "2.1.238 (Claude Code)"));
  assert.throws(() => assertSandboxSupport("win32", "2.1.238 (Claude Code)"), /WSL2/);
  assert.throws(() => assertSandboxSupport("linux", "2.1.218 (Claude Code)"), /2\.1\.219/);
  assert.throws(
    () => assertSandboxSupport("linux", "2.1.238 (Claude Code)", "--settings --tools"),
    /--safe-mode/,
  );
});
