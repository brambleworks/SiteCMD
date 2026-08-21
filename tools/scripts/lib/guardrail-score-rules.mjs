export function desktopScoreConsistencyFailures(read, sourceFiles) {
  const failures = [];
  const readDesktop = (file) => read(`apps/desktop/src/${file}`);
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const splitScoreLabelPattern =
    /\b(Web Score|Code Score)\b|Web score dropped|Code score dropped|Latest Web Scan scored|Latest Code Scan scored|Web Scan: .*\/100|Code scan: .*\/100|Code Scan: .*\/100|Web Scan \{summary\.web_scan_score\}\/100|Code \$\{summary\.code_score\}\/100/;
  const userFacingBackendScoreFiles = [
    "apps/desktop/src-tauri/src/commands/scan/code_scan.rs",
    "apps/desktop/src-tauri/src/commands/scan/policy.rs",
    "apps/desktop/src-tauri/src/core/native_alerts.rs",
    "apps/desktop/src-tauri/src/background/scan_scheduler.rs",
    "apps/desktop/src-tauri/src/db/events.rs",
    "apps/desktop/src-tauri/src/report/html.rs",
    "apps/desktop/src-tauri/src/report/html/sections.rs",
    "apps/desktop/src-tauri/src/report/html/supplemental_sections.rs",
  ];
  const splitScoreLabelFiles = [...sourceFiles, ...userFacingBackendScoreFiles].filter((file) => {
    if (!/\.(ts|tsx|rs)$/.test(file) || /\.test\.(ts|tsx)$/.test(file)) return false;
    return splitScoreLabelPattern.test(read(file));
  });
  check(
    splitScoreLabelFiles.length === 0,
    `Desktop user-facing scoring UI, exports, and alerts must use SiteCMD Score instead of split Web/Code score labels: ${splitScoreLabelFiles.join(", ")}`,
  );

  const siteCmdScoreSource = readDesktop("lib/sitecmd-score.ts");
  const siteCmdScoreTests = readDesktop("lib/sitecmd-score.test.ts");
  const currentScoreSource = readDesktop("lib/current-score.ts");
  const currentScoreHook = readDesktop("hooks/useCurrentScore.ts");
  const appContentSource = readDesktop("app/AppContent.tsx");
  const postScanSummarySource = readDesktop("app/usePostScanSummary.ts");
  const dashboardSource = readDesktop("components/dashboard/Dashboard.tsx");
  const issuesPageSource = readDesktop("pages/IssuesPage.tsx");
  const sitesOverviewSource = readDesktop("components/sites/SitesOverview.tsx");
  const scanSummaryModelSource = readDesktop("components/scan/scan-summary-model.ts");
  const projectSummaryTypesSource = readDesktop("lib/project-summary-types.ts");
  const projectDashboardSource = read("apps/desktop/src-tauri/src/commands/project_dashboard.rs");
  const scanCompletionEffects = read("apps/desktop/src/lib/scan-completion-effects.ts");
  const appShellOrchestration = read("apps/desktop/src/hooks/useAppShellOrchestration.ts");
  const tokensSource = readDesktop("lib/tokens.ts");
  const issueRankingSource = readDesktop("lib/issue-ranking.ts");
  const searchConsoleModelSource = readDesktop("components/dashboard/search-console-page-model.ts");
  const confidenceTypes = readDesktop("lib/issue-confidence.ts");
  const duplicateDesktopSeverityOrderFiles = sourceFiles.filter(
    (file) =>
      file.startsWith("apps/desktop/src/") &&
      /\.(ts|tsx)$/.test(file) &&
      file !== "apps/desktop/src/lib/severity.ts" &&
      !/\.test\.tsx?$/.test(file) &&
      read(file).includes("const SEVERITY_ORDER"),
  );
  // Score is Rust-authored (compute_current_score); sitecmd-score.ts is presentation only.
  check(
    siteCmdScoreSource.includes("SEVERITY_BASE_PENALTY") &&
      siteCmdScoreSource.includes("scoreIssueImpact") &&
      siteCmdScoreSource.includes("siteCmdScoreModelFromSnapshot") &&
      !/healthScoreFromSeverity|computeSiteCmdScore/.test(siteCmdScoreSource) &&
      siteCmdScoreTests.includes("siteCmdScoreModelFromSnapshot") &&
      confidenceTypes.includes("needs_review") &&
      confidenceTypes.includes("ISSUE_CONFIDENCE_MULTIPLIER"),
    "sitecmd-score.ts must stay presentation-only (scoreIssueImpact ranking + siteCmdScoreModelFromSnapshot); the SiteCMD health score is Rust-authored (compute_current_score). Do not reintroduce a JS score engine (healthScoreFromSeverity / computeSiteCmdScore).",
  );
  check(
    siteCmdScoreSource.includes('from "@/lib/severity"') &&
      !siteCmdScoreSource.includes("const SEVERITY_ORDER") &&
      issueRankingSource.includes("severityRank") &&
      issueRankingSource.includes('from "@/lib/severity"') &&
      !issueRankingSource.includes('severity === "critical" ? 0') &&
      duplicateDesktopSeverityOrderFiles.length === 0 &&
      !tokensSource.includes("SEVERITY_PENALTY") &&
      searchConsoleModelSource.includes("scoreIssueImpact") &&
      !searchConsoleModelSource.includes("SEVERITY_PENALTY"),
    `Frontend severity ordering and score penalties must reuse severity.ts and sitecmd-score.ts instead of local literals: ${duplicateDesktopSeverityOrderFiles.join(", ")}`,
  );

  // getCurrentScore in lib/commands/issues.ts wraps get_current_score; only lib/current-score.ts may call it, so every score read shares one loader.
  const directCurrentScoreInvokeFiles = sourceFiles.filter(
    (file) =>
      file.startsWith("apps/desktop/src/") &&
      /\.(ts|tsx)$/.test(file) &&
      !/\.test\.tsx?$/.test(file) &&
      file !== "apps/desktop/src/lib/current-score.ts" &&
      file !== "apps/desktop/src/lib/commands/issues.ts" &&
      (read(file).includes("get_current_score") || read(file).includes("getCurrentScore(")),
  );
  check(
    directCurrentScoreInvokeFiles.length === 0,
    `Desktop current score must be loaded through lib/current-score.ts (which calls the getCurrentScore wrapper), not direct get_current_score / getCurrentScore calls: ${directCurrentScoreInvokeFiles.join(", ")}`,
  );
  check(
    currentScoreSource.includes("getCurrentScore(") &&
      currentScoreSource.includes("currentScoreIssueCount") &&
      currentScoreHook.includes("loadCurrentScoreSnapshot") &&
      !currentScoreHook.includes("new Map") &&
      postScanSummarySource.includes("loadCurrentScoreSnapshot") &&
      appContentSource.includes("usePostScanSummary(") &&
      !scanSummaryModelSource.includes("computeSiteCmdScore") &&
      scanSummaryModelSource.includes("sitecmdScore: number | null") &&
      scanSummaryModelSource.includes("return null") &&
      scanCompletionEffects.includes("loadCompletionScore") &&
      scanCompletionEffects.includes("loadCurrentScoreSnapshot") &&
      appShellOrchestration.includes("loadScheduledCompletionScore") &&
      appShellOrchestration.includes("loadCurrentScoreSnapshot") &&
      !appShellOrchestration.includes("getScoreMessage(payload.score)") &&
      !appShellOrchestration.includes("score: payload.score") &&
      !appShellOrchestration.includes("issueCount: payload.issues") &&
      dashboardSource.includes("buildProjectIssueSummaryFromWorkSummary(workSummary)") &&
      !dashboardSource.includes("currentScore.criticalCount +"),
    "Desktop visible SiteCMD Score surfaces must share the current-score loader and avoid cached/recomputed score snapshots.",
  );
  check(
    !dashboardSource.includes("computeSiteCmdScore") &&
      !issuesPageSource.includes("computeSiteCmdScore") &&
      dashboardSource.includes("useCurrentScore") &&
      issuesPageSource.includes("useCurrentScore") &&
      issuesPageSource.includes("siteCmdScoreModelFromSnapshot"),
    "Dashboard and Issues must render the persisted current score snapshot instead of recomputing SiteCMD Score locally.",
  );
  check(
    /pub struct TodayProjectWorkSummary[\s\S]*pub site_score: Option<u32>/.test(
      projectDashboardSource,
    ) &&
      !/pub struct TodayProjectWorkSummary[\s\S]*pub code_score: Option<u32>/.test(
        projectDashboardSource,
      ) &&
      projectDashboardSource.includes("compute_current_score(&groups, now_ms)") &&
      projectSummaryTypesSource.includes("siteScore: number | null") &&
      projectSummaryTypesSource.includes("siteIssueCount: number") &&
      projectDashboardSource.includes("site_issue_count") &&
      projectDashboardSource.includes("site_critical_count") &&
      projectDashboardSource.includes("site_high_count") &&
      sitesOverviewSource.includes("return project.siteScore") &&
      sitesOverviewSource.includes("p.siteIssueCount") &&
      sitesOverviewSource.includes("p.siteCriticalCount") &&
      !sitesOverviewSource.includes("getProjectIssueTotalFromWorkSummary") &&
      !sitesOverviewSource.includes("project.codeScore") &&
      !sitesOverviewSource.includes("Math.min(project.latestScore"),
    "Sites overview must display the backend current SiteCMD Score, not min/latest Web Scan or Code Scan artifact scores.",
  );
  // The labeling/persistence family (diagnostic labels, MCP artifact labels,
  // site-score-changed emission) lives in guardrail-score-labeling-rules.mjs.
  return failures;
}
