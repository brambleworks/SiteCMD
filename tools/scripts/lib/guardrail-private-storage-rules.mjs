export function privateStorageSafetyFailures(read) {
  const appIdentity = read("apps/desktop/src-tauri/src/app_identity.rs");
  const app = read("apps/desktop/src-tauri/src/lib.rs");
  const auditLog = read("apps/desktop/src-tauri/src/audit_log.rs");
  const keyringStore = read("apps/desktop/src-tauri/src/keyring/store.rs");
  const failures = [];

  if (
    !appIdentity.includes("ensure_private_directory") ||
    !appIdentity.includes("from_mode(0o700)") ||
    !appIdentity.includes("from_mode(0o600)") ||
    !appIdentity.includes("custom_flags(libc::O_NOFOLLOW)") ||
    !app.includes("ensure_private_directory(&app_data_dir)") ||
    !app.includes("restrict_private_file(&db_path)") ||
    !app.includes("ensure_private_directory(&log_dir)") ||
    !auditLog.includes("ensure_private_directory(parent)") ||
    !auditLog.includes("custom_flags(libc::O_NOFOLLOW)") ||
    !auditLog.includes("restrict_open_private_file(&file)") ||
    !keyringStore.includes("write_private_file(path, json.as_bytes())")
  ) {
    failures.push(
      "Desktop private state must keep app-data and log directories owner-only, keep database/audit/debug-secret files owner-only, and refuse symlink file writes.",
    );
  }

  return failures;
}
