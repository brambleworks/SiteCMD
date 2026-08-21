import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";

const MANIFEST = "apps/desktop/src-tauri/crates/cli/Cargo.toml";

/** Build the release CLI package and return its binary path. */
export function buildScanner(repoRoot, release = false) {
  const profile = release ? "release" : "debug";
  const binary = path.join(repoRoot, "apps/desktop/src-tauri/target", profile, "sitecmd_cli");
  const args = ["build", "--manifest-path", MANIFEST];
  if (release) args.push("--release");
  const r = spawnSync("cargo", args, {
    cwd: repoRoot,
    stdio: "inherit",
    encoding: "utf8",
  });
  if (r.status !== 0) {
    throw new Error(`cargo build for sitecmd_cli failed (exit ${r.status})`);
  }
  if (!existsSync(binary)) {
    throw new Error(`scanner binary not found after build: ${binary}`);
  }
  return binary;
}

/** Run an audit and parse its `CodeScanReportView` JSON. */
export function scanJson(binary, projectPath) {
  const r = spawnSync(binary, ["audit", projectPath, "--format", "json"], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (r.status !== 0 && !r.stdout) {
    throw new Error(
      `scan failed for ${projectPath} (exit ${r.status}): ${r.stderr || "no output"}`,
    );
  }
  let report;
  try {
    report = JSON.parse(r.stdout);
  } catch (err) {
    throw new Error(
      `could not parse scan JSON for ${projectPath}: ${err.message}\n${r.stderr || ""}`,
      { cause: err },
    );
  }
  return report;
}

/** Run an audit and return its review-format brief. */
export function scanReview(binary, projectPath) {
  const r = spawnSync(binary, ["audit", projectPath, "--format", "review"], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (r.status !== 0 && !r.stdout) {
    throw new Error(
      `review scan failed for ${projectPath} (exit ${r.status}): ${r.stderr || "no output"}`,
    );
  }
  return r.stdout;
}

/** Return a finding identity that survives line shifts. */
export function issueKey(issue) {
  if (issue.checkId) return issue.checkId;
  return `${issue.id}::${issue.relativePath}`;
}

export function keySet(report) {
  return new Set((report.issues || []).map(issueKey));
}

/** Compare resolved, unresolved, and newly introduced findings. */
export function diffScans(baseline, post) {
  const before = keySet(baseline);
  const after = keySet(post);
  const resolved = [...before].filter((k) => !after.has(k));
  const unresolved = [...before].filter((k) => after.has(k));
  const regressions = [...after].filter((k) => !before.has(k));
  return {
    baselineCount: before.size,
    postCount: after.size,
    resolved,
    unresolved,
    regressions,
    resolvedCount: resolved.length,
    unresolvedCount: unresolved.length,
    regressionCount: regressions.length,
    resolutionRate: before.size === 0 ? 0 : resolved.length / before.size,
  };
}

export function categoryCounts(report) {
  const counts = {};
  for (const issue of report.issues || []) {
    counts[issue.category] = (counts[issue.category] || 0) + 1;
  }
  return counts;
}
