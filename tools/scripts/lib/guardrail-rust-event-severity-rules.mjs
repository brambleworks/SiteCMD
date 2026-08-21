export function rustEventSeverityFailures(read) {
  const dbTypes = read("apps/desktop/src-tauri/src/db/types.rs");
  const eventSeverityFiles = [
    "commands/scan/web_scan.rs",
    "commands/scan/multi_scan.rs",
    "commands/scan/code_scan.rs",
    "commands/scan/execution.rs",
    "background/scan_scheduler.rs",
    "db/events.rs",
  ].map((file) => `apps/desktop/src-tauri/src/${file}`);
  const inlineFiles = eventSeverityFiles.filter((file) =>
    /(?:overall_)?score < (?:50|80)|\*score < (?:50|80)|(?:report\.|\*)critical_count > 0|(?:report\.|\*)high_count > 0/.test(
      read(file),
    ),
  );
  const hasCentralHelper =
    dbTypes.includes("pub fn from_scan_score") &&
    dbTypes.includes("pub fn from_issue_counts") &&
    dbTypes.includes("maps_scan_scores_to_event_severity_bands") &&
    dbTypes.includes("maps_issue_counts_to_event_severity");
  return hasCentralHelper && inlineFiles.length === 0
    ? []
    : [
        `Desktop scan/code-scan event severity must use EventSeverity::from_scan_score/from_issue_counts instead of inline score/count thresholds: ${inlineFiles.join(", ")}`,
      ];
}
