export function desktopFrontendDisplayFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const activityFeedSource = read("apps/desktop/src/lib/activity-feed.ts");
  const dashboardActivitySource = read("apps/desktop/src/lib/dashboard/activity.ts");
  check(
    activityFeedSource.includes("export const FULL_SCAN_MERGE_WINDOW_MS") &&
      dashboardActivitySource.includes("FULL_SCAN_MERGE_WINDOW_MS") &&
      dashboardActivitySource.includes('from "@/lib/activity-feed"') &&
      !dashboardActivitySource.includes("const FULL_SCAN_MERGE_WINDOW_MS"),
    "Desktop scan activity merge windows must use lib/activity-feed.ts FULL_SCAN_MERGE_WINDOW_MS instead of duplicate literals.",
  );

  const utilsSource = read("apps/desktop/src/lib/utils.ts");
  const utilsTests = read("apps/desktop/src/lib/utils.test.ts");
  const inlineUrlDisplayFiles = sourceFiles.filter((file) => {
    if (file === "apps/desktop/src/lib/utils.ts") return false;
    const source = read(file);
    return (
      source.includes("replace(/^https?:") ||
      source.includes("function extractDomain") ||
      source.includes("new URL(normalizedUrl).hostname") ||
      source.includes("new URL(page).pathname") ||
      /hostname\s*\+\s*\([^)]*pathname/.test(source) ||
      source.includes("split(/[?#]/, 1)[0]") ||
      (source.includes('parsed.pathname && parsed.pathname !== "/"') &&
        source.includes(": parsed.hostname"))
    );
  });
  check(
    utilsSource.includes("formatUrlDisplay") &&
      utilsSource.includes("formatUrlHost") &&
      utilsSource.includes("formatUrlPathOrHost") &&
      utilsSource.includes("formatUrlHostPath") &&
      utilsSource.includes("getUrlPathname") &&
      utilsTests.includes("formats compact URL labels") &&
      utilsTests.includes("formats affected URL labels as path when useful and host otherwise") &&
      utilsTests.includes("formats event URL labels as host plus path without query secrets") &&
      inlineUrlDisplayFiles.length === 0,
    `Desktop URL display labels must use lib/utils.ts URL display helpers instead of local regex, hostname, or pathname parsing: ${inlineUrlDisplayFiles.join(", ")}`,
  );

  // The backdrop must clear the top bar and the scan column must remain
  // reachable when it exceeds the viewport height.
  const layoutCss = read("apps/desktop/src/styles/layout.css");
  const scanCss = read("apps/desktop/src/styles/pages/scan.css");
  const scanOverlaySource = read("apps/desktop/src/components/scan/ScanOverlay.tsx");
  const scanBackdropRule = layoutCss.split(".overlay-backdrop--scan")[1]?.split("}")[0] ?? "";
  const scanContentRule = scanCss.split(".scan-overlay-content")[1]?.split("}")[0] ?? "";
  check(
    /top:\s*3rem/.test(scanBackdropRule) &&
      /overflow-y:\s*auto/.test(scanBackdropRule) &&
      scanOverlaySource.includes('className="scan-overlay-content"') &&
      /margin:\s*auto/.test(scanContentRule),
    "Scan overlay must stay reachable in short windows: .overlay-backdrop--scan needs top: 3rem + overflow-y: auto (the z-110 top bar paints above it), and .scan-overlay-content needs margin: auto so overflow top-aligns and scrolls instead of clipping.",
  );

  // Persistent route banners can outlive transient scan errors.
  const appRoutesSource = read("apps/desktop/src/app/AppRoutes.tsx");
  const completionEffectsSource = read("apps/desktop/src/app/useScanCompletionEffects.ts");
  check(
    !appRoutesSource.includes("error:") &&
      !appRoutesSource.includes("{error") &&
      !appRoutesSource.includes("page-error-callout") &&
      completionEffectsSource.includes("formatScanError(parseScanError(error))") &&
      completionEffectsSource.includes("toast.error(formatted.title, formatted.body)") &&
      completionEffectsSource.includes("failJob("),
    "Scan failures must reach the user only through the typed scan-error toast and jobs-tray record in useScanCompletionEffects; AppRoutes must not render the raw useScan error string as a persistent banner (it outlives its cause, e.g. quota errors after upgrading).",
  );

  return failures;
}
