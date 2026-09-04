import { readFileSync } from "node:fs";
import path from "node:path";

export const VM_NAME = "sitecmd-bench";
export const runtimeLock = JSON.parse(
  readFileSync(new URL("../vm/runtime-lock.json", import.meta.url), "utf8"),
);

export function createVmConfig() {
  const asset = (name) => readFileSync(new URL(`../vm/${name}`, import.meta.url), "utf8");
  return {
    minimumLimaVersion: runtimeLock.lima.version,
    vmType: "vz",
    arch: "aarch64",
    plain: true,
    cpus: 4,
    memory: "6GiB",
    disk: "32GiB",
    images: [runtimeLock.ubuntu],
    mounts: [],
    portForwards: [],
    containerd: { system: false, user: false },
    ssh: { forwardAgent: false, forwardX11: false, loadDotSSHPubKeys: false },
    propagateProxyEnv: false,
    env: {},
    user: {
      name: "benchadmin",
      comment: "SiteCMD benchmark administrator",
      home: "/home/benchadmin",
      uid: 1001,
      shell: "/bin/bash",
      passwordlessSudo: true,
    },
    provision: [
      ...["runtime-lock.json", "network.nft", "grader-canary", "verify.sh", "verify-webkit.py"].map(
        (name) => ({
          mode: "data",
          path: `/opt/sitecmd-benchmark/${name}`,
          content: asset(name),
          owner: "root:root",
          permissions: name === "verify.sh" ? "755" : "644",
        }),
      ),
      {
        mode: "data",
        path: "/etc/systemd/system/sitecmd-benchmark-firewall.service",
        content: asset("firewall.service"),
      },
      { mode: "system", script: asset("provision.sh") },
    ],
  };
}

export function vmEnvironment(workRoot, environment = process.env) {
  const inherited = Object.fromEntries(
    ["HOME", "USER", "LOGNAME", "PATH", "TMPDIR", "LANG"]
      .filter((key) => environment[key] !== undefined)
      .map((key) => [key, environment[key]]),
  );
  return { ...inherited, LIMA_HOME: path.join(workRoot, "lima"), LIMA_SHELLENV_BLOCK: "*" };
}
