import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { vmEnvironment, VM_NAME } from "./vm-config.mjs";
import { verifyFrozenConfig } from "./vm-lifecycle.mjs";
import { verifyRuntime } from "./vm-runtime.mjs";

export const workRoot = fileURLToPath(new URL("../.work/", import.meta.url));

export function guestCommand(
  args,
  { input, capture = false, timeout = 30000, maxBuffer = 16 * 1024 * 1024 } = {},
) {
  verifyFrozenConfig(workRoot);
  const result = spawnSync(verifyRuntime(workRoot), ["shell", "--workdir=/", VM_NAME, ...args], {
    env: vmEnvironment(workRoot),
    input,
    encoding: "utf8",
    stdio: capture
      ? ["pipe", "pipe", "pipe"]
      : [input === undefined ? "ignore" : "pipe", "inherit", "inherit"],
    timeout,
    killSignal: "SIGKILL",
    maxBuffer,
  });
  if (result.status !== 0)
    throw new Error(
      `Guest command failed (${result.error?.code ?? result.status}): ${result.stderr ?? args[0]}`,
    );
  return result.stdout;
}

export function guestProcess(args, input) {
  verifyFrozenConfig(workRoot);
  const child = spawn(verifyRuntime(workRoot), ["shell", "--workdir=/", VM_NAME, ...args], {
    env: vmEnvironment(workRoot),
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdin.on("error", () => {});
  child.stdin.end(input);
  return child;
}
