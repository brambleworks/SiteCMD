import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { createConnection } from "node:net";
import { fileURLToPath } from "node:url";
import { bridgeRequest } from "./bridge-client.mjs";

const options = JSON.parse(process.argv[2]);
if (options.anthropic) {
  const { SandboxManager } = await import(options.runtime);
  await SandboxManager.initialize(options.settings);
  try {
    const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
    const command = [
      "node",
      fileURLToPath(import.meta.url),
      JSON.stringify({ ...options, anthropic: false }),
    ]
      .map(quote)
      .join(" ");
    const wrapped = await SandboxManager.wrapWithSandbox(command);
    const child = spawn(wrapped, { shell: true, stdio: "inherit" });
    await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code) =>
        code === 0 ? resolve() : reject(new Error(`Sandbox probe exited ${code}`)),
      );
    });
  } finally {
    await SandboxManager.reset();
  }
} else {
  const { channel, workspace } = options;
  assert.equal(process.getuid(), options.uid);
  writeFileSync(`${workspace}/probe.txt`, "Workspace remains writable");
  const result = await bridgeRequest(
    channel,
    "/submit",
    { summary: `Sandboxed ${options.name} fixture submission` },
    { timeoutMs: 10000 },
  );
  assert.deepEqual(result, { recorded: true });
  assert.throws(
    () => writeFileSync(`${channel}/responses/forged.json`, "{}"),
    /EACCES|EPERM|EROFS/,
  );
  assert.throws(
    () => renameSync(`${channel}/requests`, `${channel}/renamed`),
    /EACCES|EPERM|EROFS/,
  );
  assert.throws(() => readFileSync(options.canary), /EACCES|EPERM|ENOENT/);
  await assert.rejects(
    new Promise((resolve, reject) => {
      const socket = createConnection(`${channel}/canary.sock`);
      socket.setTimeout(2000);
      socket.once("connect", () => {
        socket.destroy();
        resolve();
      });
      socket.once("error", reject);
      socket.once("timeout", () => {
        socket.destroy();
        reject(new Error("Socket timed out instead of being denied"));
      });
    }),
    /EPERM|EACCES/,
  );
  const receipt = {
    sandbox: options.name,
    submission: true,
    responseForgeryDenied: true,
    directoryRenameDenied: true,
    credentialDirectoryDenied: true,
    unixSocketDenied: true,
    modelCalls: 0,
  };
  writeFileSync(`${workspace}/${options.name}-sandbox-receipt.json`, JSON.stringify(receipt), {
    flag: "wx",
    mode: 0o600,
  });
  console.log(JSON.stringify(receipt));
}
