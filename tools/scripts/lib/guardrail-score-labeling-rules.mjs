export function scoreArtifactLabelingFailures(read) {
  const failures = [];
  const readDesktop = (file) => read(`apps/desktop/src/${file}`);
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const dashboardEmptyStateSource = readDesktop("components/dashboard/DashboardEmptyState.tsx");
  const dashboardActivitySource = readDesktop("lib/dashboard/activity.ts");
  const topBarSource = readDesktop("components/layout/TopBar.tsx");
  const traySummarySource = readDesktop("app/useTraySummary.ts");
  const codeScanPresentationSource = readDesktop("components/scan/code-scan-result-model.ts");
  const codeScanCommandSource = read("apps/desktop/src-tauri/src/commands/scan/code_scan.rs");
  const scanSchedulerSource = read("apps/desktop/src-tauri/src/background/scan_scheduler.rs");
  const executionSource = read("apps/desktop/src-tauri/src/commands/scan/execution.rs");
  const mcpIndexSource = read("apps/mcp-server/src/server.ts");
  const mcpReadmeSource = read("apps/mcp-server/README.md");

  check(
    codeScanPresentationSource.includes('scoreLabel: "Diagnostic Score"') &&
      !codeScanPresentationSource.includes('scoreLabel: "SiteCMD Score"') &&
      !topBarSource.includes("env.latest_score") &&
      !traySummarySource.includes("latest_score") &&
      !dashboardEmptyStateSource.includes("latestCodeScanSummary.overallScore") &&
      !dashboardActivitySource.includes("· score") &&
      !dashboardActivitySource.includes("overallScore}`") &&
      !dashboardActivitySource.includes("overall_score}`") &&
      !codeScanCommandSource.includes('title: format!("SiteCMD Score') &&
      !scanSchedulerSource.includes('title: format!("SiteCMD Score'),
    "Raw scan artifact scores must stay out of primary UI chrome and be labelled as diagnostics in scan artifact surfaces.",
  );
  check(
    mcpIndexSource.includes("formatScanArtifactScore") &&
      mcpIndexSource.includes("Get the latest scan artifact score") &&
      mcpIndexSource.includes("Get scan artifact score history") &&
      !mcpIndexSource.includes("**Score:**") &&
      !mcpIndexSource.includes("| Date | Score |") &&
      !mcpIndexSource.includes("— Score:") && // allow-em-dash: needle that bans an em-dash before "Score:" in MCP output
      mcpReadmeSource.includes("latest scan artifact score") &&
      mcpReadmeSource.includes("scan artifact score history") &&
      !mcpReadmeSource.includes("latest scan score") &&
      !mcpReadmeSource.includes("health score and category breakdown") &&
      !mcpReadmeSource.includes("Track score history"),
    "sitecmd-mcp must label historical scan row scores as scan artifact scores, not the current SiteCMD Score.",
  );
  const scanPersistenceSources = [
    read("apps/desktop/src-tauri/src/commands/scan/web_scan.rs"),
    read("apps/desktop/src-tauri/src/commands/scan/code_scan.rs"),
    read("apps/desktop/src-tauri/src/commands/scan/multi_scan.rs"),
  ];
  check(
    executionSource.includes("record_scan_execution_event(&db, &execution)") &&
      executionSource.includes("emit_site_score_changed(&app, project_id)") &&
      executionSource.includes("emit_scan_execution_completed(") &&
      scanPersistenceSources.every((source) => !source.includes("emit_site_score_changed")),
    "Only the execution finalizer may emit site-score-changed and scan-execution-completed; child Web/Code/multi persistence must stay silent until every planned child settles.",
  );
  return failures;
}
