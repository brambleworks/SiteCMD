import { spawnSync } from "node:child_process";
import { digest } from "./workflow-plan.mjs";

export function sourceSnapshot(repository) {
  const git = (args) => {
    const result = spawnSync("git", args, {
      cwd: repository,
      timeout: 30000,
      maxBuffer: 256 * 1024 * 1024,
    });
    if (result.status !== 0) throw new Error(`Source export failed: ${result.stderr}`);
    return result.stdout;
  };
  const commit = git(["rev-parse", "HEAD"]).toString().trim();
  if (!/^[a-f0-9]{40}$/.test(commit)) throw new Error("A committed source revision is required");
  const archive = git(["archive", "--format=tar", commit]);
  return { commit, archive, sha256: digest(archive) };
}
