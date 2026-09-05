import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  chmodSync,
  chownSync,
  copyFileSync,
  mkdirSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:net";
import { trialInvocation } from "../lib/trial-invocation.mjs";
import { createTrialBridge } from "./trial-bridge.mjs";
import { createWorkspace, closeWorkspace } from "./trial-workspace.mjs";
import { systemCommand } from "./desktop-session.mjs";

if (process.platform !== "linux" || process.getuid() !== 0)
  throw new Error("Guest controller required");
const runtime = "/opt/sitecmd-benchmark/sandbox-probe";
const packageName = "@anthropic-ai/sandbox-runtime";
const lock = JSON.parse(readFileSync(`${runtime}/package-lock.json`));
assert.equal(lock.packages[`node_modules/${packageName}`]?.version, "0.0.75");
assert.equal(
  lock.packages[`node_modules/${packageName}`]?.integrity,
  "sha512-oqAKi6QtkT2DpLwFoDCDD757zw2i6ftpLTyV8rNSV9QWF53q2m1JxEs0RYXv2CIXtCoje4RGYQylagn15RKmww==",
);
const id = randomBytes(12).toString("hex");
const channel = `/run/sitecmd-benchmark/${id}`;
const workspace = createWorkspace(id, {});
const canary = `/home/runner/.sitecmd-benchmark-sandbox-canary-${id}`;
writeFileSync(canary, "Owned permission probe, not an account credential", {
  flag: "wx",
  mode: 0o644,
});
const publicTools = "/usr/local/lib/sitecmd-benchmark";
const owner = {
  uid: Number(systemCommand("id", ["-u", "runner"])),
  gid: Number(systemCommand("id", ["-g", "runner"])),
};
mkdirSync("/run/sitecmd-benchmark", { recursive: true, mode: 0o755 });
mkdirSync(publicTools, { recursive: true, mode: 0o755 });
for (const name of ["bridge-files.mjs", "bridge-client.mjs", "channel-probe.mjs"])
  copyFileSync(new URL(`./${name}`, import.meta.url), `${publicTools}/${name}`);
let bridge, server;
const submitted = [];
const run = (command, args, env = {}) =>
  new Promise((resolve, reject) => {
    const child = spawn(
      "sudo",
      [
        "-u",
        "runner",
        "env",
        ...Object.entries(env).map(([key, value]) => `${key}=${value}`),
        command,
        ...args,
      ],
      { cwd: workspace, detached: true, stdio: ["ignore", "pipe", "pipe"] },
    );
    let output = "";
    const kill = () => {
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch (error) {
        if (error.code !== "ESRCH") reject(error);
      }
    };
    const timer = setTimeout(kill, 30000);
    const capture = (chunk) => {
      output += chunk;
      if (output.length > 1024 * 1024) kill();
    };
    child.stdout.on("data", capture);
    child.stderr.on("data", capture);
    child.once("error", reject);
    child.once("close", (code) => {
      clearTimeout(timer);
      if (code !== 0) reject(new Error(`Sandbox test failed (${code}): ${output.slice(-4000)}`));
      else resolve(output);
    });
  });
try {
  bridge = await createTrialBridge({
    channel,
    owner,
    arm: "normal",
    submit: async (summary) => {
      submitted.push(summary);
      return { recorded: true };
    },
    onError: (error) => console.error(error.message),
  });
  server = createServer((socket) => socket.end());
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(`${channel}/canary.sock`, resolve);
  });
  chownSync(`${channel}/canary.sock`, owner.uid, owner.gid);
  chmodSync(`${channel}/canary.sock`, 0o600);
  for (const agent of ["codex", "claude"]) {
    const invocation = trialInvocation({
      agent,
      model: agent === "codex" ? "gpt-5.6-sol" : "claude-opus-5",
      arm: "normal",
      workspace,
      channel,
    });
    const options = { channel, workspace, canary, uid: owner.uid, name: agent };
    let command = "codex",
      args;
    if (agent === "codex") {
      const config = invocation.args.flatMap((value, index, all) =>
        value === "-c" ? [value, all[index + 1]] : [],
      );
      args = [
        "sandbox",
        "--permission-profile",
        "benchmark",
        "--cd",
        workspace,
        ...config,
        "node",
        `${publicTools}/channel-probe.mjs`,
        JSON.stringify(options),
      ];
    } else {
      const { sandbox } = JSON.parse(invocation.args[invocation.args.indexOf("--settings") + 1]);
      options.anthropic = true;
      options.runtime = `${runtime}/node_modules/${packageName}/dist/index.js`;
      options.settings = {
        network: { ...sandbox.network, deniedDomains: [] },
        filesystem: {
          ...sandbox.filesystem,
          allowWrite: [workspace, ...sandbox.filesystem.allowWrite],
          denyWrite: [],
        },
      };
      command = "node";
      args = [`${publicTools}/channel-probe.mjs`, JSON.stringify(options)];
    }
    await run(command, args, invocation.env);
    assert.ok(
      submitted.includes(`Sandboxed ${agent} fixture submission`),
      `${agent}: probe never submitted`,
    );
    const receipt = JSON.parse(readFileSync(`${workspace}/${agent}-sandbox-receipt.json`));
    assert.deepEqual(receipt, {
      sandbox: agent,
      submission: true,
      responseForgeryDenied: true,
      directoryRenameDenied: true,
      credentialDirectoryDenied: true,
      unixSocketDenied: true,
      modelCalls: 0,
    });
    console.log(JSON.stringify(receipt));
  }
} finally {
  unlinkSync(canary);
  if (server) await new Promise((resolve) => server.close(resolve));
  try {
    await bridge?.close();
  } finally {
    closeWorkspace(workspace);
  }
}
