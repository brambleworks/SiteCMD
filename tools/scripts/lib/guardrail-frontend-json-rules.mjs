export function desktopFrontendJsonSafetyFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const jsonRecordSource = read("apps/desktop/src/lib/json-record.ts");
  const storeSource = read("apps/desktop/src/lib/store.ts");
  const shellStateSource = read("apps/desktop/src/lib/app-shell-state.ts");
  const appShellOrchestrationSource = read("apps/desktop/src/hooks/useAppShellOrchestration.ts");
  const projectSelectionSource = read("apps/desktop/src/lib/project-selection-state.ts");
  const reportsPageModelSource = read("apps/desktop/src/components/reports/reports-page-model.ts");
  const alertDossierSource = read("apps/desktop/src/components/alerts/AlertDossier.tsx");
  const alertDetailModelSource = read("apps/desktop/src/components/alerts/alert-detail-model.ts");

  check(
    jsonRecordSource.includes("parseJsonRecord") &&
      jsonRecordSource.includes("JSON.parse(raw)") &&
      jsonRecordSource.includes("parseNumberRecord") &&
      jsonRecordSource.includes("isJsonRecord(parsed)") &&
      jsonRecordSource.includes("!Array.isArray(value)") &&
      shellStateSource.includes("parseJsonRecord(raw)") &&
      appShellOrchestrationSource.includes("parseNumberRecord(parseJsonRecord(raw))") &&
      projectSelectionSource.includes("parseJsonRecord(raw)") &&
      !shellStateSource.includes("JSON.parse(raw) as") &&
      !appShellOrchestrationSource.includes('return parsed && typeof parsed === "object"') &&
      !projectSelectionSource.includes("JSON.parse(raw) as"),
    "Desktop persisted localStorage state must parse JSON as unknown records before reading fields.",
  );

  const uncheckedRawLocalStorageCasts = sourceFiles.flatMap((file) =>
    read(file)
      .split("\n")
      .map((line, index) => ({ file, line: index + 1, text: line.trim() }))
      .filter(
        ({ text }) =>
          text.includes("JSON.parse(raw) as") && !text.includes("JSON.parse(raw) as unknown"),
      )
      .map(({ file, line }) => `${file}:${line}`),
  );
  const migrateCallsWithoutParser = sourceFiles.filter((file) =>
    /migrateFromLocalStorage<[\s\S]*?\([^;]*,\s*(?:DEFAULTS|\{\}|\[\]|"system")\s*\)/.test(
      read(file),
    ),
  );
  check(
    storeSource.includes("parseStoredValue: (value: unknown) => T | null") &&
      storeSource.includes("store.get<unknown>(storeKey)") &&
      !storeSource.includes("JSON.parse(raw) as T") &&
      uncheckedRawLocalStorageCasts.length === 0 &&
      migrateCallsWithoutParser.length === 0,
    `Desktop localStorage migrations must validate unknown JSON before promotion to durable store. Raw casts: ${uncheckedRawLocalStorageCasts.join(", ")}. Missing parsers: ${migrateCallsWithoutParser.join(", ")}`,
  );

  check(
    reportsPageModelSource.includes("parseJsonRecord(saved)") &&
      reportsPageModelSource.includes("parseJsonRecord(sectionsJson)") &&
      !reportsPageModelSource.includes("...JSON.parse(saved)") &&
      !reportsPageModelSource.includes("...JSON.parse(sectionsJson)"),
    "Desktop report preferences must validate localStorage JSON before merging into branding or section state.",
  );
  check(
    alertDossierSource.includes("parseAlertDetailRecord(alert.detailJson)") &&
      alertDetailModelSource.includes('import { parseJsonRecord } from "@/lib/json-record";') &&
      alertDetailModelSource.includes("parseJsonRecord(json)") &&
      !alertDetailModelSource.includes("JSON.parse(json) as unknown") &&
      !alertDetailModelSource.includes("return parsed as Record<string, unknown>"),
    "Desktop alert dossiers must validate detail JSON through parseJsonRecord before rendering source metadata.",
  );
  check(
    jsonRecordSource.includes("coerceJsonRecord"),
    "Desktop scan raw_data readers must keep coerceJsonRecord available for legacy JSON-string raw_data.",
  );

  const projectSummaryCacheSource = read("apps/desktop/src/lib/project-summary-cache.ts");
  const projectSummarySignalsSource = read("apps/desktop/src/lib/project-summary-signals.ts");
  check(
    projectSummaryCacheSource.includes("parseSnapshotCacheEntry") &&
      projectSummaryCacheSource.includes("isJsonRecord(value)") &&
      projectSummaryCacheSource.includes(
        "parseSnapshotCacheEntry<T>(JSON.parse(raw) as unknown)",
      ) &&
      projectSummarySignalsSource.includes(
        "parseSnapshotCacheEntry<DashboardReferenceSignals>(JSON.parse(raw) as unknown)",
      ) &&
      !projectSummaryCacheSource.includes("JSON.parse(raw) as SnapshotCacheEntry") &&
      !projectSummarySignalsSource.includes(
        "JSON.parse(raw) as { snapshot: DashboardReferenceSignals",
      ),
    "Desktop dashboard/session snapshot caches must validate cached JSON entries before hydrating them.",
  );
  return failures;
}
