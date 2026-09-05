import { execFileSync } from "node:child_process";
import { chownSync, lstatSync, mkdirSync, rmdirSync } from "node:fs";
import path from "node:path";

export function createChannel(channel, owner) {
  if (owner) {
    if (
      process.platform !== "linux" ||
      process.getuid() !== 0 ||
      !/^\/run\/sitecmd-benchmark\/[a-f0-9]{24}$/.test(channel)
    )
      throw new Error("Isolated bridge requires a root guest controller and an exact trial path");
    const parent = lstatSync(path.dirname(channel));
    if (!parent.isDirectory() || parent.uid !== 0 || (parent.mode & 0o022) !== 0)
      throw new Error("Bridge parent must be a non-writable root directory");
  }
  mkdirSync(channel, { mode: 0o750 });
  let mounted = false;
  const close = () => {
    if (mounted) {
      execFileSync("umount", [channel]);
      mounted = false;
      rmdirSync(channel);
    }
  };
  try {
    if (owner) {
      execFileSync("mount", [
        "-t",
        "tmpfs",
        "-o",
        `size=32m,nr_inodes=1024,nosuid,nodev,noexec,mode=0750,uid=0,gid=${owner.gid}`,
        "tmpfs",
        channel,
      ]);
      mounted = true;
    }
    mkdirSync(`${channel}/requests`, { mode: 0o700 });
    mkdirSync(`${channel}/responses`, { mode: 0o750 });
    if (owner) {
      chownSync(`${channel}/requests`, owner.uid, owner.gid);
      chownSync(`${channel}/responses`, 0, owner.gid);
    }
    return { close };
  } catch (error) {
    close();
    throw error;
  }
}
