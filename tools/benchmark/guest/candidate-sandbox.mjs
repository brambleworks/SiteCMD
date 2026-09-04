import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const directory = path.dirname(fileURLToPath(import.meta.url));

export function executeCandidate(item, candidate, input) {
  if (process.platform !== "linux" || process.getuid() !== 0)
    throw new Error("Candidate execution requires the isolated guest controller");
  const node = item.runtime === "node";
  const adapter = node ? "node-candidate.mjs" : "python-candidate.py";
  const args = [
    "--quiet",
    "--wait",
    "--pipe",
    "--collect",
    "--property=MemoryMax=512M",
    "--property=TasksMax=32",
    "--property=RuntimeMaxSec=8",
    "bwrap",
    "--unshare-user",
    "--unshare-pid",
    "--unshare-net",
    "--unshare-ipc",
    "--unshare-uts",
    "--unshare-cgroup",
    "--die-with-parent",
    "--new-session",
    "--cap-drop",
    "ALL",
    "--ro-bind",
    "/usr",
    "/usr",
    "--symlink",
    "usr/lib",
    "/lib",
    "--ro-bind",
    candidate,
    "/work",
    "--ro-bind",
    path.join(directory, adapter),
    "/adapter",
    ...(node ? ["--ro-bind", realpathSync("/usr/local/bin/node"), "/node"] : []),
    "--proc",
    "/proc",
    "--dev",
    "/dev",
    "--tmpfs",
    "/tmp",
    "--chdir",
    "/work",
    "--clearenv",
    "--setenv",
    "PATH",
    "/usr/bin:/usr/local/bin",
    "--uid",
    "65534",
    "--gid",
    "65534",
    "--",
    ...(node
      ? ["/node", "--max-old-space-size=256", "/adapter"]
      : ["/usr/bin/python3", "-B", "/adapter"]),
  ];
  const result = spawnSync("systemd-run", args, {
    input: JSON.stringify({ ...input, entry: item.entry }),
    encoding: "utf8",
    timeout: 12000,
    maxBuffer: 1024 * 1024,
    env: { PATH: "/usr/sbin:/usr/bin:/sbin:/bin" },
  });
  if (result.status !== 0)
    return {
      error: "CandidateProcessError",
      message: `${result.error?.message ?? result.status}: ${result.stderr}`,
    };
  try {
    return JSON.parse(result.stdout);
  } catch {
    return { error: "InvalidCandidateOutput", message: result.stdout };
  }
}
