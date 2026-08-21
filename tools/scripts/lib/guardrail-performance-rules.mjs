export function performanceGateFailures(read) {
  const frontendQualityWorkflow = read(".github/workflows/frontend-quality.yml");
  const verifyPush = read("tools/scripts/verify-push.mjs");
  const correlationResolver = read("apps/desktop/src-tauri/src/core/correlation/resolver.rs");
  const correlationTests = read("apps/desktop/src-tauri/src/core/correlation/resolver_tests.rs");
  const issueMemoryRail = read("apps/desktop/src/components/issues/IssueMemorySection.tsx");

  const failures = [];
  if (
    !frontendQualityWorkflow.includes("pnpm perf:baseline") ||
    !verifyPush.includes('name: "perf-baseline"') ||
    !verifyPush.includes("pnpm run perf:baseline")
  ) {
    failures.push("The performance baseline must run in frontend CI and local push verification.");
  }
  if (
    !correlationResolver.includes("cross_env::resolve_for_groups") ||
    !correlationResolver.includes("cross_project::resolve_patterns") ||
    !correlationResolver.includes("EnrichmentCache::load") ||
    !correlationTests.includes("resolver_database_work_is_constant_as_issue_count_grows") ||
    !correlationTests.includes("resolver_preloads_connected_integration_enrichments_once")
  ) {
    failures.push(
      "Correlation resolution must preload cross-environment, cross-project, and integration enrichment data, with DB-operation-count regression tests.",
    );
  }
  if (
    !issueMemoryRail.includes("getIssueCheckMemory(") ||
    issueMemoryRail.includes("getScanDetail(") ||
    issueMemoryRail.includes("get_scan_detail")
  ) {
    failures.push(
      "The issue History rail must read work_items lifecycle via the getIssueCheckMemory wrapper in one query, not replay getScanDetail per scan.",
    );
  }
  return failures;
}
