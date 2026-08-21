export function desktopDefensiveEmptyStatesFailures(read) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const reportsHistorySource = read("apps/desktop/src/components/reports/useReportsHistory.ts");
  check(
    /return Array\.isArray\(entries\)\s*\?[^:]+:\s*\[\]/.test(reportsHistorySource),
    "useReportsHistory must coerce get_report_history results to an array so the Reports page survives a null backend response.",
  );

  const useEventsSource = read("apps/desktop/src/hooks/useEvents.ts");
  check(
    /Array\.isArray\(raw\)\s*\?\s*raw\s*:\s*\[\]/.test(useEventsSource),
    "useEvents must coerce get_events results to an array so the Activity page survives a null backend response.",
  );

  const deploysSource = read("apps/desktop/src/components/dashboard/DeploysPage.tsx");
  const deploysDataSource = read("apps/desktop/src/components/dashboard/useDeploysPageData.ts");
  check(
    /scanHistory:\s*Array\.isArray\(scans\)\s*\?\s*scans\s*:\s*\[\]/.test(deploysDataSource) &&
      /deployEvents:\s*Array\.isArray\(timeline\)\s*\?\s*timeline\s*:\s*\[\]/.test(
        deploysDataSource,
      ) &&
      /correlations:\s*Array\.isArray\(correlations\)\s*\?\s*correlations\s*:\s*\[\]/.test(
        deploysDataSource,
      ),
    "useDeploysPageData must coerce scan/deploy/correlation arrays to [] at the query boundary so the Deploys page survives a null backend response.",
  );
  check(
    /Array\.isArray\(gitStatus\?\.commits\)\s*\?\s*gitStatus\.commits\s*:\s*\[\]/.test(
      deploysSource,
    ) &&
      /Array\.isArray\(ghData\?\.workflow_runs\)\s*\?\s*ghData\.workflow_runs\s*:\s*\[\]/.test(
        deploysSource,
      ),
    "DeploysPage must guard gitStatus.commits and ghData.workflow_runs as arrays before iterating.",
  );

  const updatesSource = read("apps/desktop/src/components/dashboard/UpdatesPage.tsx");
  // Normalize both the collection and each package row at the command boundary.
  check(
    /normalizeUpdateReport\(rawResult\)/.test(updatesSource),
    "UpdatesPage must run the detect_updates result through normalizeUpdateReport, so every PackageUpdate matches its declared type before it reaches the UI.",
  );
  // Unmarked persisted snapshots cannot stand in for a live update scan.
  const rendersPersistedSnapshot =
    /buildHydratedUpdateReport/.test(updatesSource) ||
    /setReport\(\s*snapshot\.signals\.updates/.test(updatesSource) ||
    /normalizeUpdateReport\(\s*snapshot\.signals\.updates/.test(updatesSource);
  check(
    !rendersPersistedSnapshot,
    "UpdatesPage must not render a persisted snapshot (local update snapshot or project signal snapshot) as the report. Stale package data with nothing marking it provisional reads as a broken scan; show the loading skeleton until detect_updates returns.",
  );
  check(
    !/setError\(String\(e\)\)/.test(updatesSource),
    "UpdatesPage must not surface raw `String(e)` to the user. Use friendly copy and log the technical detail to console only.",
  );

  return failures;
}
