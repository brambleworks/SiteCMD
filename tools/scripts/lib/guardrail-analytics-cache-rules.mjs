export function desktopAnalyticsCacheFailures(read) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const cacheSource = read("apps/desktop/src/lib/analytics-snapshot-cache.ts");
  const visibilitySource = read("apps/desktop/src/lib/useVisibilityRefresh.ts");
  const hookSource = read("apps/desktop/src/components/dashboard/useAnalyticsQuery.ts");
  const searchConsoleSource = read("apps/desktop/src/components/dashboard/SearchConsolePage.tsx");
  const analyticsSource = read("apps/desktop/src/components/dashboard/AnalyticsPage.tsx");

  check(
    cacheSource.includes("readAnalyticsSnapshot") &&
      cacheSource.includes("writeAnalyticsSnapshot") &&
      cacheSource.includes("buildAnalyticsSnapshotKey") &&
      cacheSource.includes("sitecmd_analytics_snapshots_v1"),
    "analytics-snapshot-cache must keep its public read/write/key API and the localStorage key intact so hydration survives module rewrites.",
  );

  check(
    visibilitySource.includes("staleAfterMs") &&
      visibilitySource.includes("visibilitychange") &&
      visibilitySource.includes("hiddenSinceRef"),
    "useVisibilityRefresh must subscribe to visibilitychange and gate refresh by a stale-after threshold.",
  );

  check(
    hookSource.includes("readAnalyticsSnapshot") && hookSource.includes("writeAnalyticsSnapshot"),
    "useAnalyticsQuery must hydrate from (initialData) and write through to the analytics snapshot cache so the card body survives WKWebView remounts.",
  );
  check(
    hookSource.includes("useVisibilityRefresh"),
    "useAnalyticsQuery must wire useVisibilityRefresh so a long minimize triggers a refetch instead of leaving the card blank.",
  );

  for (const [label, source] of [
    ["SearchConsolePage", searchConsoleSource],
    ["AnalyticsPage", analyticsSource],
  ]) {
    check(
      source.includes("useAnalyticsQuery"),
      `${label} must fetch analytics through useAnalyticsQuery so it inherits snapshot hydration and the visibility refetch instead of re-rolling its own fetch.`,
    );
  }

  return failures;
}
