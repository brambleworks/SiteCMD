import assert from "node:assert/strict";
import { test } from "node:test";
import { trialInvocation } from "./trial-invocation.mjs";

const options = {
  workspace: "/srv/sitecmd-benchmark/workspaces/abc",
  socket: "/run/sitecmd-benchmark/abc.sock",
  proxy: "/run/sitecmd-benchmark/proxy.mjs",
  arm: "normal",
};

test("Codex trials force subscription authentication, fresh state and no delegated agents", () => {
  const { args } = trialInvocation({ ...options, agent: "codex" });
  assert.ok(args.includes("gpt-5.6-sol"));
  assert.ok(args.includes("--ephemeral"));
  assert.ok(args.includes('forced_login_method="chatgpt"'));
  assert.ok(args.includes("features.multi_agent=false"));
  assert.ok(args.includes("--ignore-user-config"));
  assert.ok(
    !args.some((value) => value.includes("dangerously") || value.includes("mcp_servers.sitecmd")),
  );
});

test("Claude uses explicit subscription-compatible settings and only the assigned MCP server", () => {
  for (const arm of ["normal", "report", "mcp"]) {
    const { args, env } = trialInvocation({ ...options, agent: "claude", arm });
    assert.ok(args.includes("claude-opus-5"));
    assert.ok(args.includes("--strict-mcp-config"));
    assert.ok(args.includes("--no-session-persistence"));
    assert.ok(!args.some((value) => /fallback|bypass|--bare|--safe-mode/.test(value)));
    const mcp = JSON.parse(args[args.indexOf("--mcp-config") + 1]);
    assert.equal(Object.hasOwn(mcp.mcpServers, "sitecmd"), arm === "mcp");
    assert.equal(env.DISABLE_AUTOUPDATER, "1");
  }
});

test("unsupported agents and arms fail rather than choosing a fallback", () => {
  assert.throws(() => trialInvocation({ ...options, agent: "other" }), /Unsupported/);
  assert.throws(() => trialInvocation({ ...options, agent: "codex", arm: "brief" }), /Unsupported/);
});
