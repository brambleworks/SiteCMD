import { chmodSync, chownSync, mkdirSync, writeFileSync, readdirSync, lstatSync } from "node:fs";
import path from "node:path";
import { systemCommand } from "./desktop-session.mjs";

export function createWorkspace(id, files) {
  if (!/^[a-f0-9]{24,64}$/.test(id)) throw new Error("Invalid workspace identity");
  const parent = "/srv/sitecmd-benchmark/workspaces";
  chownSync(parent, 0, 0);
  chmodSync(parent, 0o711);
  const workspace = path.join(parent, id);
  mkdirSync(workspace, { mode: 0o755 });
  systemCommand("mount", [
    "-t",
    "tmpfs",
    "-o",
    "size=128M,nr_inodes=2048,nosuid,nodev,mode=755",
    "sitecmd-trial",
    workspace,
  ]);
  const uid = Number(systemCommand("id", ["-u", "runner"]));
  const gid = Number(systemCommand("id", ["-g", "runner"]));
  chownSync(workspace, uid, gid);
  for (const [name, contents] of Object.entries(files)) {
    if (
      name.startsWith("/") ||
      name.split("/").some((part) => !part || part === "." || part === "..")
    )
      throw new Error("Invalid workspace path");
    const parts = name.split("/");
    let directory = workspace;
    for (const part of parts.slice(0, -1)) {
      directory = path.join(directory, part);
      mkdirSync(directory, { recursive: true, mode: 0o755 });
      chownSync(directory, uid, gid);
    }
    const target = path.join(workspace, name);
    writeFileSync(target, contents, { flag: "wx", mode: 0o644 });
    chownSync(target, uid, gid);
  }
  return workspace;
}

export function closeWorkspace(workspace) {
  if (!/^\/srv\/sitecmd-benchmark\/workspaces\/[a-f0-9]{24,64}$/.test(workspace))
    throw new Error("Invalid workspace cleanup target");
  systemCommand("umount", [workspace]);
  chownSync(workspace, 0, 0);
  chmodSync(workspace, 0o700);
}

export function protectPreviousWorkspaces(active) {
  const parent = "/srv/sitecmd-benchmark/workspaces";
  for (const name of readdirSync(parent)) {
    const candidate = path.join(parent, name);
    if (candidate === active) continue;
    if (!/^[a-f0-9]{24,64}$/.test(name) || !lstatSync(candidate).isDirectory())
      throw new Error("Unexpected benchmark workspace; review it before running agents");
    chownSync(candidate, 0, 0);
    chmodSync(candidate, 0o700);
  }
}

export function mountDesktopWorkspace(id, workspace) {
  if (!/^[a-f0-9]{24,64}$/.test(id) || workspace !== `/srv/sitecmd-benchmark/workspaces/${id}`)
    throw new Error("Invalid desktop workspace mount");
  const desktopHome = systemCommand("getent", ["passwd", "sitecmd"]).split(":")[5];
  if (!desktopHome?.startsWith("/")) throw new Error("Desktop account home is unavailable");
  const target = path.join(desktopHome, "projects", id);
  mkdirSync(target, { recursive: true, mode: 0o755 });
  systemCommand("mount", ["--bind", workspace, target]);
  return { path: target, close: () => systemCommand("umount", [target]) };
}
