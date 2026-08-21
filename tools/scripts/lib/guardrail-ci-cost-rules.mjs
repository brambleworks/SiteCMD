import { orderedBefore, stripJsComments } from "./guardrail-text-utils.mjs";

// Exported for focused negative controls over synthetic workflows.
export function deployWorkflowHardeningFailures(read, workflowFiles) {
  const failures = [];
  for (const file of workflowFiles) {
    const source = read(file);
    if (!/\bwrangler deploy\b/.test(source)) continue;
    if (!(source.includes("concurrency:") && source.includes("cancel-in-progress: true"))) {
      failures.push(
        `${file} must declare a concurrency group with cancel-in-progress so two quick pushes cannot deploy out of order and ship the older commit last.`,
      );
    }
    const jobCount = (source.match(/^\s+runs-on:/gm) || []).length;
    const timeoutCount = (source.match(/^\s+timeout-minutes:/gm) || []).length;
    if (!(jobCount > 0 && timeoutCount >= jobCount)) {
      failures.push(
        `${file} must declare timeout-minutes on every job (found ${timeoutCount} timeout-minutes for ${jobCount} runs-on entries); a hung wrangler bills the 360-minute GitHub default.`,
      );
    }
    if (!source.includes("workflow_dispatch:")) {
      failures.push(
        `${file} must keep a workflow_dispatch trigger so a deploy-credential failure can be re-run without a dummy commit.`,
      );
    }
    const selfPath = `"${file}"`;
    if (!source.includes(selfPath)) {
      failures.push(
        `${file} must watch its own workflow file in the push paths filter (${selfPath}) so edits to the deploy pipeline deploy themselves.`,
      );
    }
  }
  return failures;
}

export function ciCostSafetyFailures(read, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const workflowFiles = listFiles(".github/workflows", (file) => /\.ya?ml$/.test(file));
  const dailyCronWorkflows = [];
  for (const file of workflowFiles) {
    const cronLines = read(file).match(/cron:\s*["']([^"']+)["']/g) || [];
    for (const rawLine of cronLines) {
      const expr = rawLine
        .replace(/cron:\s*["']/, "")
        .replace(/["']$/, "")
        .trim();
      const fields = expr.split(/\s+/);
      // Wildcard day-of-month and day-of-week schedules run at least daily.
      if (fields.length === 5 && fields[2] === "*" && fields[4] === "*") {
        dailyCronWorkflows.push(`${file} (cron "${expr}")`);
      }
    }
  }
  check(
    dailyCronWorkflows.length === 0,
    `Scheduled workflows must run no more than weekly: pin a specific day-of-week or day-of-month instead of "* * *". These fire daily-or-more: ${dailyCronWorkflows.join(", ")}.`,
  );

  const dependencyAuditWorkflow = read(".github/workflows/dependency-audit.yml");
  check(
    /^ {2}pull_request:/m.test(dependencyAuditWorkflow) &&
      dependencyAuditWorkflow.includes("pnpm run audit:deps:signer"),
    "dependency-audit.yml must audit the standalone updater signer on dependency pull requests as well as push and schedule runs.",
  );

  const releaseWorkflow = read(".github/workflows/release.yml");
  const buildTauriScript = read(".github/scripts/release/build-tauri-app.sh");
  const releaseJobCount = (releaseWorkflow.match(/^\s+runs-on:/gm) || []).length;
  const releaseTimeoutCount = (releaseWorkflow.match(/^\s+timeout-minutes:/gm) || []).length;
  check(
    releaseJobCount > 0 && releaseTimeoutCount >= releaseJobCount,
    `release.yml must declare timeout-minutes on every job (found ${releaseTimeoutCount} timeout-minutes for ${releaseJobCount} runs-on entries). The 360-minute default bills up to 3600 minutes for one hung macOS job.`,
  );
  check(
    releaseWorkflow.includes("concurrency:") &&
      releaseWorkflow.includes("cancel-in-progress: true"),
    "release.yml must keep a concurrency group with cancel-in-progress so a superseded run of the same tag stops billing.",
  );
  // Build only installer formats that the corresponding release leg uploads.
  check(
    buildTauriScript.includes('--bundles "$BUNDLES"') &&
      releaseWorkflow.includes("bundles: app") &&
      releaseWorkflow.includes("bundles: appimage") &&
      releaseWorkflow.includes("bundles: nsis"),
    'release.yml must pass --bundles "$BUNDLES" to tauri build with per-leg matrix bundles (app / appimage / nsis) so unshipped installers are never built or signed.',
  );

  failures.push(...deployWorkflowHardeningFailures(read, workflowFiles));

  for (const file of [
    ".github/workflows/playwright.yml",
    ".github/workflows/knip.yml",
    ".github/workflows/size-limit.yml",
    ".github/workflows/frontend-quality.yml",
  ]) {
    check(
      !/^ {2}push:/m.test(read(file)),
      `${file} must not have a push trigger: verify-push.mjs already gates every push to main locally, so the hosted run is pure duplicate spend. Keep pull_request and workflow_dispatch only.`,
    );
  }

  // Hosted clippy covers platform-specific Rust branches unavailable locally.
  const clippyWorkflow = read(".github/workflows/cargo-clippy.yml");
  check(
    /^ {2}push:/m.test(clippyWorkflow) &&
      clippyWorkflow.includes("branches: [main]") &&
      clippyWorkflow.includes('- "apps/desktop/src-tauri/**"'),
    "cargo-clippy.yml must keep its push-to-main trigger scoped to apps/desktop/src-tauri/**: macOS clippy never type-checks Linux cfg arms, so without a hosted push run a Linux-only lint first surfaces in the release preflight, after the tag is already pushed.",
  );

  // Inspect executable content, not required strings mentioned in comments.
  const verifyPush = stripJsComments(read("tools/scripts/verify-push.mjs"));
  for (const requiredCheck of [
    "cargo nextest run",
    "cargo test --doc",
    "cargo clippy",
    "cargo build --manifest-path crates/cli/Cargo.toml",
    "forbid-adhoc-rust-patterns.sh",
    "check-agents-md.mjs",
  ]) {
    check(
      verifyPush.includes(requiredCheck),
      `verify-push.mjs must keep running \`${requiredCheck}\`: the local mirror is what catches a regression before the push lands (for the Rust-only checks it is the only gate at all).`,
    );
  }

  // Mirror every release dependency audit in the local push gate.
  const preflightAudits = new Set(
    [...releaseWorkflow.matchAll(/pnpm run (audit:deps:[a-z]+)/g)].map((match) => match[1]),
  );
  for (const auditScript of preflightAudits) {
    check(
      verifyPush.includes(auditScript),
      `verify-push.mjs must mirror release.yml's \`pnpm run ${auditScript}\`: an advisory that fails the release preflight otherwise surfaces only after the tag is pushed, and fixing it there costs a release cycle.`,
    );
  }

  // Mirror audit environment flags so local and release checks use the same data.
  for (const auditEnv of new Set(
    [...releaseWorkflow.matchAll(/^\s*(SITECMD_[A-Z_]*AUDIT[A-Z_]*):\s*"?1"?\s*$/gm)].map(
      (match) => match[1],
    ),
  )) {
    check(
      new RegExp(`${auditEnv}=1`).test(verifyPush),
      `verify-push.mjs must run its audit with ${auditEnv}=1, the way release.yml does. Mirroring the script name alone lets the local gate answer a weaker question than the release preflight, so a green push is followed by a failed tag.`,
    );
  }

  // Free runner disk before Rust builds consume the available space.
  const diskStepIndex = releaseWorkflow.search(/^\s*- name: Free runner disk\s*$/m);
  check(
    diskStepIndex !== -1,
    "release.yml preflight must keep its 'Free runner disk' step: the hosted image's free space is smaller than the Rust debug target dir, and without it the release dies with ENOSPC after the tag is pushed.",
  );
  check(
    diskStepIndex !== -1 && diskStepIndex < releaseWorkflow.indexOf("cargo nextest run"),
    "release.yml must free runner disk before the Rust test steps: the space is consumed by the debug target dir those steps build, so cleanup after them is cleanup after the failure.",
  );

  // Run fast external dependency checks before expensive local work.
  for (const slowStep of ["pnpm test", "cargo nextest run", "cargo clippy"]) {
    for (const auditScript of preflightAudits) {
      check(
        orderedBefore(releaseWorkflow, `pnpm run ${auditScript}`, slowStep),
        `release.yml preflight must run \`pnpm run ${auditScript}\` before \`${slowStep}\`: the audit takes seconds and fails on advisories published outside this repo, so running it later spends minutes on a preflight that cannot pass.`,
      );
    }
  }

  // Pin every push-gate step so removing one is an explicit guardrail change.
  for (const stepName of [
    "typecheck",
    "lint",
    "prettier",
    "installer",
    "workflows",
    "guardrails:repo",
    "knip:budget",
    "knip:files",
    "cargo-fmt",
    "rust-patterns",
    "frontend-constants",
    "gitleaks",
    "agents-md",
    "regex-audit",
    "audit:deps:rust",
    "tauri-commands",
    "cli-build",
    "desktop-vitest",
    "worker-vitest",
    "rust-nextest",
    "rust-doctest",
    "rust-clippy",
    "guardrails-tests",
    "perf-baseline",
    "rust-perf-gates",
    "desktop-build",
    "size-limit",
    "playwright",
    "naming-audit",
  ]) {
    check(
      verifyPush.includes(`name: "${stepName}"`),
      `verify-push.mjs must keep its "${stepName}" step: the local mirror is the primary push gate, and a dropped tier only fails after the push, in CI or production.`,
    );
  }
  for (const workerFilter of ["--filter sitecmd-mcp"]) {
    check(
      verifyPush.includes(workerFilter),
      `verify-push.mjs worker-vitest must keep \`${workerFilter}\`: that suite otherwise only runs after the push.`,
    );
  }

  return failures;
}
