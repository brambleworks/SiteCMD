import assert from "node:assert/strict";
import { test } from "node:test";
import { trialInvocation } from "./trial-invocation.mjs";
import { pilotPolicy } from "./workflow-pilot.mjs";

const options = {
  workspace: "/srv/sitecmd-benchmark/workspaces/abc",
  channel: "/run/sitecmd-benchmark/abc",
  proxy: "/run/sitecmd-benchmark/proxy.mjs",
  arm: "normal",
};

test("Codex trials force subscription authentication, fresh state and no delegated agents", () => {
  const { args } = trialInvocation({ ...options, agent: "codex", model: "gpt-5.6-sol" });
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
    const { args, env } = trialInvocation({
      ...options,
      agent: "claude",
      model: "claude-opus-5",
      arm,
    });
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

test("Claude can edit only its case without interactive approval", () => {
  for (const arm of ["normal", "report", "mcp"]) {
    const { args } = trialInvocation({ ...options, agent: "claude", model: "claude-opus-5", arm });
    const index = args.indexOf("--allowedTools");
    assert.notEqual(index, -1);
    assert.deepEqual(args[index + 1].split(","), [
      "Bash",
      `Read(/${options.workspace}/**)`,
      `Edit(/${options.workspace}/**)`,
      `Write(/${options.workspace}/**)`,
      ...(arm === "mcp" ? ["mcp__sitecmd__*"] : []),
    ]);
    const settings = JSON.parse(args[args.indexOf("--settings") + 1]);
    assert.equal(settings.sandbox.failIfUnavailable, true);
    assert.equal(settings.sandbox.allowUnsandboxedCommands, false);
  }
});

test("every approved model is requested exactly, without provider or model fallback", () => {
  for (const configuration of pilotPolicy.models) {
    const { args } = trialInvocation({ ...options, ...configuration });
    assert.equal(args[args.indexOf("--model") + 1], configuration.model);
    assert.throws(
      () => trialInvocation({ ...options, ...configuration, model: "latest" }),
      /Unsupported/,
    );
    assert.throws(
      () =>
        trialInvocation({
          ...options,
          ...configuration,
          agent: configuration.agent === "codex" ? "claude" : "codex",
        }),
      /Unsupported/,
    );
  }
  assert.throws(() => trialInvocation({ ...options, agent: "codex" }), /Unsupported/);
});

test("file transport grants writes only to this trial's inbox and never opens Unix sockets", () => {
  for (const configuration of pilotPolicy.models) {
    const { args } = trialInvocation({ ...options, ...configuration, arm: "mcp" });
    assert.ok(!args.some((arg) => /allowUnixSockets|allowAllUnixSockets|unix_sockets/.test(arg)));
    if (configuration.agent === "claude") {
      const { sandbox } = JSON.parse(args[args.indexOf("--settings") + 1]);
      assert.deepEqual(sandbox.filesystem.allowWrite, [`${options.channel}/requests`]);
      assert.ok(sandbox.filesystem.allowRead.includes(options.channel));
      assert.equal(sandbox.network.strictAllowlist, true);
      assert.deepEqual(sandbox.network.allowedDomains, []);
    } else {
      const permissions = args.find((arg) => arg.startsWith("permissions.benchmark="));
      assert.ok(permissions.includes(`${JSON.stringify(options.channel)}="read"`));
      assert.ok(permissions.includes(`${JSON.stringify(`${options.channel}/requests`)}="write"`));
    }
  }
});
