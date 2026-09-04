import assert from "node:assert/strict";
import { test } from "node:test";
import { probeAgentAccounts } from "./workflow-preflight.mjs";

function commandResponse(command, args) {
  const output =
    command === "codex"
      ? args.includes("--version")
        ? "codex-cli 0.153.0-alpha.5"
        : "Logged in using ChatGPT"
      : args.includes("--version")
        ? "2.1.259 (Claude Code)"
        : JSON.stringify({
            loggedIn: true,
            authMethod: "claude.ai",
            apiProvider: "firstParty",
            email: "not-for-output@example.com",
            subscriptionType: "max",
          });
  return { status: 0, stdout: output, stderr: "" };
}

test("preflight checks versions and subscription status without launching a model", () => {
  const calls = [];
  const result = probeAgentAccounts({
    environment: {},
    run: (command, args, options) => {
      calls.push([command, args]);
      assert.equal(options.shell, false);
      assert.ok(options.timeout <= 5000);
      return commandResponse(command, args);
    },
  });
  assert.deepEqual(calls, [
    ["codex", ["--version"]],
    ["codex", ["login", "status"]],
    ["claude", ["--version"]],
    ["claude", ["auth", "status", "--json"]],
  ]);
  assert.equal(result.subscriptionAccountsVerified, true);
  assert.equal(result.readyToRun, false);
  assert.doesNotMatch(JSON.stringify(result), /not-for-output/);
});

test("billing overrides, failed probes, and malformed auth responses fail closed", () => {
  const secret = "never-print-this-key";
  const result = probeAgentAccounts({
    environment: { ANTHROPIC_API_KEY: secret, OPENAI_BASE_URL: "https://example.com" },
    run: commandResponse,
  });
  assert.equal(result.subscriptionAccountsVerified, false);
  assert.match(result.blockers.join("\n"), /ANTHROPIC_API_KEY/);
  assert.doesNotMatch(JSON.stringify(result), new RegExp(secret));
  for (const response of [
    { status: 1, stdout: "Logged in using ChatGPT", stderr: secret },
    { status: null, error: new Error(secret) },
    { status: 0, stdout: "malformed" },
  ]) {
    const checked = probeAgentAccounts({ environment: {}, run: () => response });
    assert.equal(checked.subscriptionAccountsVerified, false);
    assert.doesNotMatch(JSON.stringify(checked), new RegExp(secret));
  }
});
