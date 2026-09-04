export function codeScanSecurityFailures(read) {
  const failures = [];
  const issueUtils = read("apps/desktop/src-tauri/src/core/code_scan/issue_utils.rs");
  const databaseTargets = read("apps/desktop/src-tauri/src/core/database_targets.rs");
  const sqliteInspection = read(
    "apps/desktop/src-tauri/src/core/code_scan/database_analysis/live_inspection/sqlite.rs",
  );
  // The security contract spans production code and its sibling test module.
  const filesystem =
    read("apps/desktop/src-tauri/src/core/code_scan/filesystem.rs") +
    read("apps/desktop/src-tauri/src/core/code_scan/filesystem_tests.rs");
  const networkPolicy =
    read("apps/desktop/src-tauri/src/network_policy.rs") +
    read("apps/desktop/src-tauri/src/network_policy_tests.rs");
  const dnsCache = read("apps/desktop/src-tauri/src/dns_cache.rs");
  const constants = read("apps/desktop/src-tauri/src/constants.rs");

  if (!(
    issueUtils.includes("redact_sensitive_excerpt_line") &&
    issueUtils.includes("redact_sensitive_excerpt_line(line.trim_end())") &&
    issueUtils.includes("security_regression_redacts_secret_like_values_from_source_excerpts")
  )) {
    failures.push(
      "Code Scan source excerpts must redact secret-like values before persistence, export, dossier rendering, or AI prompts.",
    );
  }
  if (!(
    databaseTargets.includes("bound_local_sqlite_path") &&
    databaseTargets.includes("canonicalize_local_sqlite_path") &&
    sqliteInspection.includes("canonicalize_local_sqlite_path(&path") &&
    databaseTargets.includes("security_regression_rejects_absolute_sqlite_paths_outside_project") &&
    databaseTargets.includes("security_regression_rejects_sqlite_symlinks_that_escape_project")
  )) {
    failures.push(
      "Code Scan local SQLite inspection must stay bounded to the linked project/env directory and reject symlink escapes.",
    );
  }
  if (!(
    filesystem.includes("DEFAULT_COLLECTION_LIMITS") &&
    filesystem.includes("max_files: 5_000") &&
    filesystem.includes("max_total_bytes: 64_000_000") &&
    filesystem.includes("collect_project_inventory") &&
    filesystem.includes("security_regression_source_file_collection_enforces_file_count_budget")
  )) {
    failures.push(
      "Code Scan filesystem collection must keep file-count, total-byte, and depth budgets with regression tests.",
    );
  }
  if (!(
    networkPolicy.includes("validate_resolved_domain_ip_target(domain, addr.ip(), policy)?") &&
    networkPolicy.includes(
      "security_regression_domain_resolution_rejects_loopback_under_scan_policy",
    )
  )) {
    failures.push(
      "Scan URL DNS validation must reject non-localhost domains that resolve to loopback addresses.",
    );
  }
  if (!(
    dnsCache.includes("validate_resolved_addrs(&host, &addrs, policy)?") &&
    dnsCache.includes("security_regression_cached_public_host_cannot_rebind_to_loopback") &&
    dnsCache.includes(".max_capacity(max_entries)") &&
    dnsCache.includes(".time_to_live(ttl)") &&
    dnsCache.includes("crate::constants::DNS_CACHE_TTL") &&
    constants.includes("pub const DNS_CACHE_TTL: Duration") &&
    dnsCache.includes("cache_evicts_by_capacity_and_ttl")
  )) {
    failures.push(
      "HTTP DNS resolution must re-check cached and fresh answers against the shared scan URL policy and keep a bounded TTL cache.",
    );
  }

  return failures;
}
