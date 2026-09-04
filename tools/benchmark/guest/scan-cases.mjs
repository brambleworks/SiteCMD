import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { createWorkspace, mountDesktopWorkspace, closeWorkspace } from "./trial-workspace.mjs";

const { cases, product } = JSON.parse(readFileSync(0, "utf8"));
const results = [];
for (const item of cases) {
  const reports = {};
  for (const variant of ["baseline", "reference"]) {
    const id = randomBytes(16).toString("hex");
    const workspace = createWorkspace(id, item[`${variant}Files`]);
    let mounted;
    try {
      mounted = mountDesktopWorkspace(id, workspace);
      const result = spawnSync(
        "sudo",
        ["-u", "sitecmd", product.cli, "audit", mounted.path, "--format", "json"],
        { encoding: "utf8", timeout: 120000, maxBuffer: 16 * 1024 * 1024 },
      );
      if (result.status !== 0) throw new Error(`Code Scan failed: ${result.stderr}`);
      const report = JSON.parse(result.stdout);
      reports[variant] = { report, raw: result.stdout };
    } finally {
      try {
        mounted?.close();
      } finally {
        closeWorkspace(workspace);
      }
    }
  }
  results.push({ id: item.id, ...reports });
}
console.log(JSON.stringify(results));
