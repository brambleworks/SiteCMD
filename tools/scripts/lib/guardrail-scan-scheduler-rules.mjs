const SCHEDULER_FILE = "apps/desktop/src-tauri/src/background/scan_scheduler.rs";
const FORBIDDEN_BYPASSES = [
  "save_scan(",
  "save_code_scan(",
  "insert_event(",
  "post_scan_persist(",
  "run_code_scan_internal(",
  "scan_url_for_execution(",
];
const REQUIRED_ORCHESTRATOR_CALL = "run_scan_execution_internal(";

export function scanSchedulerPersistPathFailures(read) {
  const failures = [];
  const schedulerSource = read(SCHEDULER_FILE);

  const forbiddenCalls = FORBIDDEN_BYPASSES.filter((call) => schedulerSource.includes(call));
  if (forbiddenCalls.length > 0) {
    failures.push(
      `${SCHEDULER_FILE} must route scheduled scans through run_scan_execution_internal, not collectors or scheduler-local persistence: ${forbiddenCalls.join(", ")}`,
    );
  }

  if (!schedulerSource.includes(REQUIRED_ORCHESTRATOR_CALL)) {
    failures.push(
      `${SCHEDULER_FILE} must route scheduled Web, Code, and Full actions through ${REQUIRED_ORCHESTRATOR_CALL}`,
    );
  }

  // Scheduled scans must use the managed cancellation state.
  if (schedulerSource.includes("ScanControlState::default()")) {
    failures.push(
      `${SCHEDULER_FILE} must thread the managed ScanControlState into scheduled scans (managed_scan_control), not a throwaway ScanControlState::default() that cancel_scan can never reach.`,
    );
  }
  if (!schedulerSource.includes("scan_control: ScanControlState")) {
    failures.push(
      `${SCHEDULER_FILE} must accept the app's managed ScanControlState as a parameter (threaded from the spawn site) so scheduled scans join the cancellation system.`,
    );
  }

  return failures;
}
