const SCAN_PERSIST_MODULES = [
  "apps/desktop/src-tauri/src/commands/scan/web_scan.rs",
  "apps/desktop/src-tauri/src/commands/scan/multi_scan.rs",
];

export function scanPersistOffThreadFailures(read, exists) {
  const failures = [];
  for (const file of SCAN_PERSIST_MODULES) {
    if (!exists(file)) continue;
    if (!read(file).includes("run_blocking(")) {
      failures.push(
        `${file} - scan persistence must run off the async runtime via run_blocking(...) because rusqlite blocks the calling thread; the off-thread wrapper is missing.`,
      );
    }
  }
  return failures;
}
