#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { buildScanner, scanJson, scanReview, diffScans, categoryCounts } from "./lib/scanner.mjs";
import { runClaudeFix } from "./lib/claude.mjs";
import { buildPrompt } from "./lib/arms.mjs";
import { runConverge } from "./lib/converge.mjs";
import { aggregate, renderMarkdown } from "./lib/report.mjs";
import {
  createRunCopy,
  createRunRoot,
  ensurePinnedClone,
  removeRunWorkspace,
  validateBenchmarkConfig,
} from "./lib/workspaces.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "../..");
const WORK = path.join(HERE, ".work");
const REPOS = path.join(WORK, "repos");
const RUNS = path.join(WORK, "runs");
const RESULTS = path.join(HERE, "results");
const NULL_DEVICE = process.platform === "win32" ? "NUL" : "/dev/null";

function parseArgs(argv) {
  const opts = {
    dryRun: false,
    release: false,
    keepCopies: false,
    converge: false,
    targets: null,
    arms: null,
    repeats: null,
    config: null,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-run") opts.dryRun = true;
    else if (a === "--until-converged") opts.converge = true;
    else if (a === "--release") opts.release = true;
    else if (a === "--keep-copies") opts.keepCopies = true;
    else if (a === "--config") opts.config = argv[++i];
    else if (a === "--repeats") opts.repeats = Number(argv[++i]);
    else if (a === "--arms") opts.arms = argv[++i].split(",").map((s) => s.trim());
    else if (a === "--target") {
      opts.targets = (opts.targets || []).concat(argv[++i].split(",").map((s) => s.trim()));
    } else {
      throw new Error(`unknown option: ${a}`);
    }
  }
  return opts;
}

function git(args, cwd) {
  const environment = Object.fromEntries(
    Object.entries(process.env).filter(([name]) => !name.startsWith("GIT_")),
  );
  Object.assign(environment, {
    GIT_CONFIG_GLOBAL: NULL_DEVICE,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_TERMINAL_PROMPT: "0",
  });
  const r = spawnSync("git", ["-c", `core.hooksPath=${NULL_DEVICE}`, ...args], {
    cwd,
    encoding: "utf8",
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (r.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${r.stderr || r.stdout}`);
  }
  return (r.stdout || "").trim();
}

function stamp() {
  return new Date().toISOString().replace(/[:.]/g, "-");
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const configPath = opts.config || path.join(HERE, "context-efficiency.config.json");
  const config = JSON.parse(readFileSync(configPath, "utf8"));
  if (opts.arms) config.arms = opts.arms;
  if (opts.repeats != null) config.repeats = opts.repeats;
  if (opts.release) config.scanRelease = true;
  config.mode = opts.converge ? "converge" : config.mode || "single";
  config.convergeStallRounds ??= 2;
  config.convergeMaxRounds ??= 6;
  config.convergePerRoundTurns ??= 30;
  validateBenchmarkConfig(config);

  let targets = config.targets;
  if (opts.targets) targets = targets.filter((t) => opts.targets.includes(t.name));
  if (targets.length === 0) throw new Error("no targets selected");

  console.log(`Building scanner (${config.scanRelease ? "release" : "debug"})...`);
  const scanner = buildScanner(REPO_ROOT, config.scanRelease);
  const runRoot = createRunRoot(RUNS);

  try {
    const targetResults = [];
    for (const target of targets) {
      console.log(`\n=== ${target.name} ===`);
      console.log(`Preparing ${target.repo} at ${target.ref}...`);
      const { dest: pristine, sha } = ensurePinnedClone({
        target,
        reposRoot: REPOS,
        runGit: git,
      });

      console.log("Baseline scan...");
      const tScan = Date.now();
      const baseline = scanJson(scanner, pristine);
      const scanMsBaseline = Date.now() - tScan;
      const reviewText = scanReview(scanner, pristine);
      console.log(`  baseline issues: ${baseline.issueCount} (scan ${scanMsBaseline}ms)`);
      if (baseline.issueCount === 0) {
        console.warn(
          "  WARNING: 0 baseline issues. This target may be clean or may not exercise the configured Code Scan rules. Skipping target.",
        );
        continue;
      }

      const runs = [];
      for (const arm of config.arms) {
        for (let rep = 0; rep < config.repeats; rep++) {
          const label = `${arm}-${rep}`;
          const copy = createRunCopy({
            pristine,
            runRoot,
            targetName: target.name,
            arm,
            repeat: rep,
          });

          let record;
          if (opts.dryRun) {
            const fix = {
              ok: true,
              dryRun: true,
              totalTokens: 0,
              costUsd: 0,
              numTurns: 0,
              durationMs: 0,
              wallMs: 0,
              inputTokens: 0,
              outputTokens: 0,
              cacheCreate: 0,
              cacheRead: 0,
            };
            const diff = diffScans(baseline, scanJson(scanner, copy));
            record = { arm, repeat: rep, fix, diff };
            console.log(`  [${label}] dry-run (no claude call)`);
          } else if (config.mode === "converge") {
            console.log(
              `  [${label}] converging (<=${config.convergeMaxRounds} rounds x ${config.convergePerRoundTurns} turns)...`,
            );
            const res = runConverge({
              arm,
              copy,
              baseline,
              reviewText,
              scanner,
              model: config.model,
              perRoundTurns: config.convergePerRoundTurns,
              stallRounds: config.convergeStallRounds,
              maxRounds: config.convergeMaxRounds,
              log: console.log,
            });
            record = { arm, repeat: rep, ...res };
            console.log(
              `  [${label}] ${res.converged} after ${res.totalRounds} round(s): resolved ${res.diff.resolvedCount}/${res.diff.baselineCount} | regressions ${res.diff.regressionCount} | ${fixSummary(res.fix)}`,
            );
          } else {
            console.log(`  [${label}] running claude...`);
            const prompt = buildPrompt(arm, { baseline, reviewText });
            const fix = runClaudeFix({
              prompt,
              cwd: copy,
              model: config.model,
              maxTurns: config.maxTurns,
            });
            if (!fix.ok) console.warn(`  [${label}] claude error: ${fix.error || fix.subtype}`);
            const diff = diffScans(baseline, scanJson(scanner, copy));
            console.log(
              `  [${label}] resolved ${diff.resolvedCount}/${diff.baselineCount} | regressions ${diff.regressionCount} | ${fixSummary(fix)}`,
            );
            record = { arm, repeat: rep, promptChars: prompt.length, fix, diff };
          }

          runs.push(record);
          if (!opts.keepCopies) removeRunWorkspace(runRoot, copy);
        }
      }

      targetResults.push({
        name: target.name,
        repo: target.repo,
        sha,
        baselineCount: baseline.issueCount,
        categoryCounts: categoryCounts(baseline),
        scanMsBaseline,
        reviewChars: reviewText.length,
        runs,
      });
    }

    if (targetResults.length === 0) {
      throw new Error("No targets produced results. Nothing to report.");
    }

    const perArm = aggregate(targetResults, config.arms);
    const when = stamp();
    const outDir = path.join(RESULTS, when);
    mkdirSync(outDir, { recursive: true });
    const raw = { stamp: when, config, dryRun: opts.dryRun, targets: targetResults, perArm };
    writeFileSync(path.join(outDir, "raw.json"), JSON.stringify(raw, null, 2));
    const md = renderMarkdown({
      perArm,
      arms: config.arms,
      targets: targetResults,
      config,
      stamp: when,
    });
    writeFileSync(path.join(outDir, "report.md"), md);

    console.log(`\nWrote ${path.relative(REPO_ROOT, outDir)}/report.md`);
    console.log("\n" + md);
    if (opts.keepCopies) console.log(`\nPreserved run copies under ${runRoot}`);
  } finally {
    if (!opts.keepCopies) removeRunWorkspace(RUNS, runRoot);
  }
}

function fixSummary(fix) {
  return `turns ${fix.numTurns} | ${fix.totalTokens.toLocaleString("en-US")} tok | $${fix.costUsd.toFixed(3)} | ${(fix.durationMs / 1000).toFixed(0)}s`;
}

main().catch((err) => {
  console.error(err.stack || String(err));
  process.exit(1);
});
