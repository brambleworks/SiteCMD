export function reportScoreConsistencyFailures(read) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };
  const rustTypes = read("apps/desktop/src-tauri/src/report/types.rs");
  const rustReport = read("apps/desktop/src-tauri/src/report.rs");
  const rustHtml = read("apps/desktop/src-tauri/src/report/html.rs");
  const pdfSections = read("apps/desktop/src/components/reports/ReportPDFSections.tsx");
  const pdfModel = read("apps/desktop/src/components/reports/report-pdf-model.ts");
  const pageModel = read("apps/desktop/src/components/reports/reports-page-model.ts");
  const pageSections = read("apps/desktop/src/components/reports/ReportsPageSections.tsx");
  // Rust fields stay snake_case while wire fields use camelCase; the
  // frontend's own persisted history summary keeps snake keys.
  check(
    rustTypes.includes("pub site_score: SiteScoreSummary") &&
      rustReport.includes("compute_current_score") &&
      rustReport.includes("db.get_work_items_grouped(project_id, Some(site_url), now_ms)") &&
      /export type ReportData = \{[^\n]*siteScore: SiteScoreSummary/.test(
        read("apps/desktop/src/generated/ipc-bindings.ts"),
      ) &&
      pdfModel.includes("ReportData as GeneratedReportData") &&
      pdfModel.includes("export type ReportData = GeneratedReportData"),
    "Report payloads must carry the unified SiteCMD Score from current work_items, not infer it from Web Scan or Code Scan artifacts.",
  );
  check(
    pdfSections.includes("const { branding, siteScore } = data") &&
      pdfSections.includes("const { categories, codeScan, siteScore } = data") &&
      !pdfSections.includes('label="SiteCMD Score"\n          value={health.currentScore}') &&
      !pdfSections.includes('label="SiteCMD Score"\n          value={codeScan.currentScore}') &&
      !pdfSections.includes("Score Breakdown by Source") &&
      !pdfSections.includes('label="Diagnostic Score"') &&
      !pdfModel.includes("webScore") &&
      !pdfModel.includes("codeScore") &&
      rustHtml.includes("score = data.site_score.current_score") &&
      !rustHtml.includes("score = data.health.current_score"),
    "Report renderers must show the single SiteCMD Score from site_score, never a competing Web Scan / Code Scan score split.",
  );
  check(
    pageModel.includes("site_score: snapshot.siteScore.currentScore") &&
      pageModel.includes("has_code_scan: Boolean(snapshot.codeScan)") &&
      !pageModel.includes("web_scan_score: webScanSummary.currentScore") &&
      !pageModel.includes("code_score: snapshot.codeScan?.currentScore") &&
      pageSections.includes("summary.site_score") &&
      pageSections.includes("summary.has_code_scan") &&
      !pageSections.includes("summary.web_scan_score") &&
      !pageSections.includes("summary.code_score"),
    "Saved report history summaries must persist/display unified SiteCMD Score, not Web Scan or Code Scan scores.",
  );
  check(
    rustReport.includes("let (analytics, uptime) = tokio::join!(") &&
      !rustReport.includes("fetch_analytics_summary(&configs, &sections, period_days).await"),
    "Independent analytics and uptime report summaries must be fetched concurrently.",
  );
  // The standalone report embeds the canonical light palette and score bands.
  const lightTokens = read("apps/desktop/src/styles/tokens.css").split(".dark {")[0];
  const scoreTs = read("apps/desktop/src/lib/score.ts");
  const reportUtils = read("apps/desktop/src-tauri/src/report/html/utils.rs");
  const paletteTokens = [
    ...["excellent", "good", "attention", "poor", "critical"].map((b) => `score-${b}`),
    ...["critical", "high", "medium", "low"].map((s) => `severity-${s}`),
  ];
  for (const token of paletteTokens) {
    const value = lightTokens.match(new RegExp(`--${token}:\\s*([^;]+);`))?.[1]?.trim();
    check(
      Boolean(value),
      `Light-theme --${token} is missing from the :root block of apps/desktop/src/styles/tokens.css; the report palette pin extracts it from there.`,
    );
    check(
      !value || reportUtils.includes(`"${value}"`),
      `report/html/utils.rs must use the light-theme --${token} value "${value}" from tokens.css in score_color/severity_color.`,
    );
  }
  for (const band of ["excellent", "good", "attention", "poor"]) {
    const threshold = scoreTs.match(new RegExp(`${band}: (\\d+),`))?.[1];
    check(
      Boolean(threshold),
      `THRESHOLDS.${band} is missing from apps/desktop/src/lib/score.ts; the report score-band pin extracts it from there.`,
    );
    check(
      !threshold || reportUtils.includes(`score >= ${threshold}`),
      `report/html/utils.rs score_color must keep the ${band} band at "score >= ${threshold}" to match lib/score.ts.`,
    );
  }
  return failures;
}
