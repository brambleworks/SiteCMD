import { spawnSync } from "node:child_process";
import { existsSync, lstatSync, mkdirSync, readFileSync, statfsSync, writeFileSync } from "node:fs";
import path from "node:path";
import { createVmConfig, VM_NAME, vmEnvironment } from "./vm-config.mjs";
import { installRuntime, verifyRuntime } from "./vm-runtime.mjs";
import { digest } from "./workflow-plan.mjs";

export function assertPrivateState(workRoot) {
  for (const directory of [
    workRoot,
    ...["lima", "lima/_config", `lima/${VM_NAME}`, "vm-runtime"].map((name) =>
      path.join(workRoot, name),
    ),
  ]) {
    try {
      const entry = lstatSync(directory);
      if (entry.isSymbolicLink() || !entry.isDirectory())
        throw new Error(`VM state must be a real directory: ${directory}`);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  for (const name of ["default.yaml", "override.yaml", "base.yaml"])
    if (existsSync(path.join(workRoot, "lima", "_config", name)))
      throw new Error(`Unexpected global Lima configuration: ${name}`);
}

function lima(
  workRoot,
  args,
  { input, capture = false, timeout = 1800000, acceptedStatuses = [0] } = {},
) {
  assertPrivateState(workRoot);
  const result = spawnSync(verifyRuntime(workRoot), args, {
    env: vmEnvironment(workRoot),
    cwd: workRoot,
    input,
    encoding: "utf8",
    timeout,
    killSignal: "SIGKILL",
    maxBuffer: 8 * 1024 * 1024,
    stdio: capture || input !== undefined ? ["pipe", "pipe", "pipe"] : "inherit",
  });
  if (!acceptedStatuses.includes(result.status)) {
    if (result.stdout) console.error(result.stdout);
    if (result.stderr) console.error(result.stderr);
    throw new Error(`Lima ${args[0]} failed (${result.error?.code ?? result.status})`);
  }
  return result.stdout;
}

function verifyFrozenConfig(workRoot) {
  assertPrivateState(workRoot);
  const instance = path.join(workRoot, "lima", VM_NAME);
  const receipt = JSON.parse(readFileSync(path.join(instance, "sitecmd-config.json"), "utf8"));
  if (
    receipt.sourceSha256 !== digest(createVmConfig()) ||
    receipt.instanceSha256 !== digest(readFileSync(path.join(instance, "lima.yaml")))
  )
    throw new Error("VM configuration changed; inspect the frozen VM before restarting it");
}

export async function setupVm(workRoot) {
  if (process.platform !== "darwin" || process.arch !== "arm64")
    throw new Error("The benchmark VM requires Apple Silicon macOS");
  mkdirSync(workRoot, { recursive: true, mode: 0o700 });
  assertPrivateState(workRoot);
  const capacity = statfsSync(workRoot);
  if (capacity.bavail * capacity.bsize < 45 * 1024 ** 3)
    throw new Error("At least 45 GiB of free host storage is required for initial VM setup");
  await installRuntime(workRoot);
  mkdirSync(path.join(workRoot, "lima"), { recursive: true, mode: 0o700 });
  const instance = path.join(workRoot, "lima", VM_NAME);
  if (!existsSync(instance)) {
    const config = createVmConfig();
    const file = path.join(workRoot, `sitecmd-vm-${digest(config).slice(0, 16)}.yaml`);
    if (!existsSync(file))
      writeFileSync(file, JSON.stringify(config, null, 2), { flag: "wx", mode: 0o600 });
    if (digest(JSON.parse(readFileSync(file, "utf8"))) !== digest(config))
      throw new Error("A different VM template already exists; inspect it before setup");
    lima(workRoot, ["validate", file], { capture: true, timeout: 30000 });
    lima(workRoot, ["create", "--tty=false", "--name", VM_NAME, file]);
    writeFileSync(
      path.join(instance, "sitecmd-config.json"),
      JSON.stringify({
        sourceSha256: digest(config),
        instanceSha256: digest(readFileSync(path.join(instance, "lima.yaml"))),
      }),
      { flag: "wx", mode: 0o600 },
    );
  }
  startVm(workRoot);
}

export function startVm(workRoot) {
  verifyFrozenConfig(workRoot);
  lima(workRoot, ["start", "--tty=false", "--timeout=30m", VM_NAME]);
  verifyVm(workRoot);
}

export function stopVm(workRoot, { run = lima } = {}) {
  const status = () =>
    JSON.parse(run(workRoot, ["list", "--json", VM_NAME], { capture: true, timeout: 15000 }))
      .status;
  const initial = status();
  if (initial === "Stopped") return;
  if (initial !== "Running") throw new Error(`Cannot shut down a VM in state ${initial}`);
  // Poweroff may close SSH before its exit status arrives.
  run(workRoot, ["shell", "--workdir=/", VM_NAME, "sudo", "systemctl", "poweroff"], {
    timeout: 15000,
    acceptedStatuses: [0, 255],
  });
  if (status() === "Stopped") return;
  run(workRoot, ["stop", "--tty=false", VM_NAME], {
    capture: true,
    timeout: 45000,
    acceptedStatuses: [0, 1],
  });
  if (status() !== "Stopped") throw new Error("The benchmark VM did not stop");
}

export function vmStatus(workRoot) {
  const status = JSON.parse(
    lima(workRoot, ["list", "--json", VM_NAME], { capture: true, timeout: 30000 }),
  );
  return {
    name: status.name,
    status: status.status,
    cpus: status.cpus,
    memoryBytes: status.memory,
    diskBytes: status.disk,
    sshAddress: status.sshAddress,
    sshPort: status.sshLocalPort,
    plain: status.config.plain,
    hostMounts: status.config.mounts ?? [],
    agentForwarding: status.config.ssh.forwardAgent,
  };
}

export function verifyVm(workRoot) {
  verifyFrozenConfig(workRoot);
  return lima(
    workRoot,
    ["shell", "--workdir=/", VM_NAME, "sudo", "/opt/sitecmd-benchmark/verify.sh"],
    { timeout: 60000 },
  );
}

export function vmShell(workRoot) {
  verifyFrozenConfig(workRoot);
  lima(workRoot, ["shell", "--workdir=/", VM_NAME, "sudo", "-iu", "runner"], {
    timeout: 0,
  });
}
