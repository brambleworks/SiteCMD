export function workflowSafetyFailures(read, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const workflowFiles = listFiles(".github/workflows", (file) => /\.ya?ml$/.test(file));
  const npxWorkflowFiles = workflowFiles.filter((file) => /\bnpx\b/.test(read(file)));
  check(
    npxWorkflowFiles.length === 0,
    `GitHub workflows must use the configured project package manager, not npx: ${npxWorkflowFiles.join(", ")}`,
  );

  // Workspace-only changes must still trigger root lint and tests.
  const frontendQualityWorkflow = read(".github/workflows/frontend-quality.yml");
  for (const watchedWorkspace of ["apps/desktop/src/**", "apps/mcp-server/src/**"]) {
    check(
      frontendQualityWorkflow.includes(watchedWorkspace),
      `frontend-quality.yml must run root lint/test gates when ${watchedWorkspace} changes.`,
    );
  }

  const rustMsrvPath = ".github/workflows/rust-msrv.yml";
  const rustMsrvWorkflow = workflowFiles.includes(rustMsrvPath) ? read(rustMsrvPath) : "";
  check(
    rustMsrvWorkflow.includes("toolchain: 1.89.0") &&
      rustMsrvWorkflow.includes("cargo check --locked --workspace --all-targets") &&
      rustMsrvWorkflow.includes(
        "cargo check --locked --manifest-path crates/cli/Cargo.toml --all-targets",
      ),
    "Rust MSRV CI must compile the desktop workspace and headless CLI on Rust 1.89.0.",
  );

  const dependencyAuditPath = ".github/workflows/dependency-audit.yml";
  if (workflowFiles.includes(dependencyAuditPath)) {
    const dependencyAudit = read(dependencyAuditPath);
    check(
      dependencyAudit.includes("pnpm run audit:licenses:js") &&
        dependencyAudit.includes("tools/scripts/check-javascript-licenses.mjs") &&
        dependencyAudit.includes("THIRD_PARTY_DEPENDENCIES.json"),
      "Dependency audit CI must enforce JavaScript licenses and watch its policy inputs.",
    );
    check(
      /^ {2}merge_group:\s*\n {4}types: \[checks_requested\]/m.test(dependencyAudit),
      "Dependency audit CI must run on merge_group checks_requested so queued dependency changes are audited.",
    );
  }

  const repositoryGuardrailPath = ".github/workflows/repository-guardrails.yml";
  const repositoryGuardrailWorkflow = workflowFiles.includes(repositoryGuardrailPath)
    ? read(repositoryGuardrailPath)
    : "";
  check(
    /^ {2}pull_request:\s*$/m.test(repositoryGuardrailWorkflow) &&
      !/^ {4}paths(?:-ignore)?:/m.test(repositoryGuardrailWorkflow) &&
      repositoryGuardrailWorkflow.includes("pnpm guardrails:repo") &&
      repositoryGuardrailWorkflow.includes("pnpm workflows:check") &&
      repositoryGuardrailWorkflow.includes("pnpm installer:check") &&
      repositoryGuardrailWorkflow.includes(
        "go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12",
      ),
    "Repository guardrails must run on every pull request and validate workflows and the public installer.",
  );

  const codeqlPath = ".github/workflows/codeql.yml";
  const codeqlWorkflow = workflowFiles.includes(codeqlPath) ? read(codeqlPath) : "";
  check(workflowFiles.includes(codeqlPath), "The repository must include a CodeQL workflow.");
  check(
    /^ {2}pull_request:\s*$/m.test(codeqlWorkflow) &&
      /^ {2}merge_group:\s*\n {4}types: \[checks_requested\]/m.test(codeqlWorkflow) &&
      /^ {2}push:\s*\n {4}branches:\s*\n {6}- main/m.test(codeqlWorkflow) &&
      /^ {2}schedule:\s*$/m.test(codeqlWorkflow),
    "CodeQL must scan pull requests, merge groups, main, and a scheduled full branch snapshot.",
  );
  check(
    codeqlWorkflow.includes("- javascript-typescript") && codeqlWorkflow.includes("- rust"),
    "CodeQL must analyze both JavaScript/TypeScript and Rust.",
  );
  check(
    codeqlWorkflow.includes("security-events: write") &&
      codeqlWorkflow.includes("contents: read") &&
      !codeqlWorkflow.includes("contents: write"),
    "CodeQL must use least-privilege read access plus security-events: write.",
  );
  check(
    /github\/codeql-action\/init@[0-9a-f]{40}/.test(codeqlWorkflow) &&
      /github\/codeql-action\/analyze@[0-9a-f]{40}/.test(codeqlWorkflow),
    "CodeQL actions must be pinned to immutable commit SHAs.",
  );

  // Required checks must subscribe to synthetic merge-group commits or the
  // merge queue cannot receive their status.
  for (const requiredWorkflow of [
    ".github/workflows/cargo-clippy.yml",
    ".github/workflows/codeql.yml",
    ".github/workflows/frontend-quality.yml",
    ".github/workflows/gitleaks.yml",
    ".github/workflows/knip.yml",
    ".github/workflows/playwright.yml",
    ".github/workflows/repository-guardrails.yml",
    ".github/workflows/rust-msrv.yml",
    ".github/workflows/rust-tests.yml",
    ".github/workflows/size-limit.yml",
  ]) {
    if (!workflowFiles.includes(requiredWorkflow)) continue;
    const source = read(requiredWorkflow);
    check(
      /^ {2}merge_group:\s*\n {4}types: \[checks_requested\]/m.test(source),
      `${requiredWorkflow} must run on merge_group checks_requested so its required check reports to GitHub's merge queue.`,
    );
  }

  // Fresh CI checkouts must create the MCP resource path before Tauri builds.
  const compilesDesktopCrate = (source) =>
    /\bcargo (check|clippy|nextest|test|build|llvm-cov)\b/.test(source) &&
    source.includes("src-tauri");
  const missingResourcePath = workflowFiles.filter(
    (file) => compilesDesktopCrate(read(file)) && !read(file).includes("dist-bundle"),
  );
  check(
    missingResourcePath.length === 0,
    `Workflows compiling the desktop crate must provide apps/mcp-server/dist-bundle (mkdir or a real bundle build) before any cargo step, or tauri-build panics on a fresh checkout: ${missingResourcePath.join(", ")}`,
  );

  return failures;
}
