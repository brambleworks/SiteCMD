export function desktopFrontendStateFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const staleDisableFiles = sourceFiles.filter((file) =>
    read(file).includes("eslint-disable react-hooks/set-state-in-effect"),
  );
  check(
    staleDisableFiles.length === 0,
    `Remove stale react-hooks/set-state-in-effect disables: ${staleDisableFiles.join(", ")}`,
  );

  // React Compiler rules remain mandatory after their suppressions are removed.
  const eslintConfig = read("eslint.config.js");
  const unenforcedHookRules = ["refs", "preserve-manual-memoization", "immutability"].filter(
    (rule) => !eslintConfig.includes(`"react-hooks/${rule}": "error"`),
  );
  check(
    unenforcedHookRules.length === 0,
    `React Compiler hook rules must stay enforced, not suppressed behind a backlog: ${unenforcedHookRules.join(", ")}`,
  );

  const issueDossierPanelSource = read("apps/desktop/src/components/issues/IssueDossierPanel.tsx");
  const issueDossierPanelTests = read(
    "apps/desktop/src/components/issues/IssueDossierPanel.test.tsx",
  );
  const scanPrefsSource = read("apps/desktop/src/hooks/useScanPrefs.tsx");
  const scanPrefsTests = read("apps/desktop/src/hooks/useScanPrefs.test.ts");
  const projectWorkSummarySource = read("apps/desktop/src/lib/project-work-summary.ts");
  const projectWorkSummaryTests = read("apps/desktop/src/lib/project-work-summary.test.ts");
  const dashboardDataStateSource = read(
    "apps/desktop/src/components/dashboard/dashboard-data-state.ts",
  );
  const issuesPageSnapshotSource = read("apps/desktop/src/pages/issues/useIssuesPageSnapshot.ts");
  const projectIssueSummarySource = read("apps/desktop/src/lib/project-issue-summary.ts");
  const inlineRelativeTimeFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !/\.(ts|tsx)$/.test(file)) return false;
    if (file === "apps/desktop/src/lib/format.ts") return false;
    const source = read(file);
    return (
      source.includes("diffSeconds") ||
      source.includes("const diff = Math.floor((Date.now()") ||
      source.includes("const diff = now -") ||
      source.includes("const mins = Math.floor(diff /") ||
      source.includes("export function timeAgo(") ||
      source.includes("export function formatPendingAge(")
    );
  });
  const rustCheckTypes = read("apps/desktop/src-tauri/crates/engine/src/vocab.rs");
  const rustCodeScanTypes = read("apps/desktop/src-tauri/src/core/code_scan/types.rs");
  check(
    inlineRelativeTimeFiles.length === 0,
    `Desktop relative time labels must use lib/format.ts formatRelativeTime instead of inline second/minute/hour math: ${inlineRelativeTimeFiles.join(", ")}`,
  );
  check(
    rustCheckTypes.includes("pub enum IssueConfidence") &&
      rustCheckTypes.includes("#[serde(default)]\n    pub confidence: IssueConfidence") &&
      rustCheckTypes.includes("pub confidence: IssueConfidence") &&
      rustCheckTypes.includes("pub confidence_reason: Option<String>") &&
      rustCodeScanTypes.includes("#[serde(default)]\n    pub confidence: IssueConfidence") &&
      rustCodeScanTypes.includes("pub confidence: IssueConfidence") &&
      rustCodeScanTypes.includes("pub confidence_reason: Option<String>"),
    "Rust web/code issue payloads must carry confidence fields so new issues intentionally inherit or set confidence.",
  );
  check(
    issueDossierPanelSource.includes('import { createPortal } from "react-dom"') &&
      issueDossierPanelSource.includes("return createPortal(panel, document.body)") &&
      issueDossierPanelTests.includes("renders the fixed overlay through document.body") &&
      !issueDossierPanelSource.includes("return (\n    <>"),
    "Desktop issue dossier overlays must render through document.body so fixed backdrops cannot be clipped by page containers.",
  );
  check(
    scanPrefsSource.includes("TIMEOUT_MIN = 10") &&
      scanPrefsSource.includes("TIMEOUT_MAX = 60") &&
      scanPrefsSource.includes("RETENTION_MIN = 5") &&
      scanPrefsSource.includes("RETENTION_MAX = 100") &&
      scanPrefsSource.includes("boundedInteger(value.timeout") &&
      scanPrefsSource.includes("setPrefsState(normalized)") &&
      scanPrefsTests.includes("clamps malformed persisted timeout and retention values"),
    "Desktop scan preferences must clamp persisted timeout and retention values before use.",
  );

  const localEmptyWorkSummaryFiles = sourceFiles.filter((file) => {
    if (file === "apps/desktop/src/lib/project-work-summary.ts") return false;
    return read(file).includes("EMPTY_WORK_SUMMARY");
  });
  const inlineWorkSummaryActivityFiles = sourceFiles.filter((file) => {
    if (file === "apps/desktop/src/lib/project-work-summary.ts") return false;
    const source = read(file);
    return source.includes("unresolvedCount > 0 ||") && source.includes("maintenanceCount > 0");
  });
  check(
    projectWorkSummarySource.includes("EMPTY_PROJECT_WORK_SUMMARY") &&
      projectWorkSummarySource.includes("hasProjectWorkSummaryActivity") &&
      projectWorkSummarySource.includes("getProjectWorkSummaryIssueTotal") &&
      dashboardDataStateSource.includes("EMPTY_PROJECT_WORK_SUMMARY") &&
      // Gate cached fallbacks on hydration, not on individual empty fields.
      issuesPageSnapshotSource.includes("snapshotHydrated") &&
      issuesPageSnapshotSource.includes("useCachedSnapshot") &&
      projectIssueSummarySource.includes("getProjectWorkSummaryIssueTotal") &&
      !projectIssueSummarySource.includes("alertCount") &&
      !projectIssueSummarySource.includes("alerts?:") &&
      !projectIssueSummarySource.includes("workSummary?:") &&
      projectWorkSummaryTests.includes("detects activity across every work-summary count") &&
      localEmptyWorkSummaryFiles.length === 0 &&
      inlineWorkSummaryActivityFiles.length === 0,
    `Desktop project work-summary defaults/activity/issue totals must use lib/project-work-summary.ts and issue summary must not accept ignored alert/work-summary inputs: ${[...localEmptyWorkSummaryFiles, ...inlineWorkSummaryActivityFiles].join(", ")}`,
  );
  const durableEntryStoreSource = read("apps/desktop/src/lib/durable-entry-store.ts");
  const updateMemorySource = read("apps/desktop/src/lib/update-memory.ts");
  const durableMemoryTests = read("apps/desktop/src/lib/durable-memory-hydration.test.ts");
  // Issue memory is backend-owned canonical IssueGroup history and must not
  // regain a second client-side durable store.
  check(
    durableEntryStoreSource.includes("migrateFromLocalStorage<Record<string, E>>") &&
      durableEntryStoreSource.includes("storeSet(storeKey") &&
      durableEntryStoreSource.includes("if (dirty && cached)") &&
      durableEntryStoreSource.includes("writeLocalStorage(cached)"),
    "Desktop durable-entry-store must hydrate reads from the durable Tauri Store and merge early writes back, not read localStorage only.",
  );
  check(
    updateMemorySource.includes("createDurableEntryStore<UpdateMemoryEntry>") &&
      updateMemorySource.includes("createDurableEntryStore<UpdateSnapshotEntry>") &&
      updateMemorySource.includes("__resetUpdateMemoryForTests"),
    "Desktop update memory must build on createDurableEntryStore, not a bespoke localStorage-only cache.",
  );
  check(
    durableMemoryTests.includes(
      "keeps the localStorage fallback in sync when update snapshots merge after early writes",
    ),
    "Desktop durable memory hydration must sync merged store-backed state back to the localStorage fallback after early writes.",
  );

  const retiredCodeIssueMemoryFiles = sourceFiles.filter((file) =>
    /(?:code-issue-memory|CodeIssueMemorySection|useDirectCodeScanDetail)/.test(file),
  );
  check(
    retiredCodeIssueMemoryFiles.length === 0,
    `Client-side Code issue memory/detail caches must stay retired; canonical backend IssueGroup history owns issue memory: ${retiredCodeIssueMemoryFiles.join(", ")}`,
  );

  // Legacy low-contrast gray classes bypass the theme token system.
  const LOW_CONTRAST_GRAY_CLASS_RE = /text-(?:zinc|gray|slate)-(?:[3-7])00\b/;
  const lowContrastGrayClassFiles = sourceFiles.filter((file) => {
    if (!file.startsWith("apps/desktop/src/") || !file.endsWith(".css")) return false;
    return LOW_CONTRAST_GRAY_CLASS_RE.test(read(file));
  });
  check(
    lowContrastGrayClassFiles.length === 0,
    `Hardcoded text-zinc-/gray-/slate-300-700 fail WCAG AAA for small text. Use text-foreground or text-muted-foreground instead: ${lowContrastGrayClassFiles.join(", ")}`,
  );

  return failures;
}
