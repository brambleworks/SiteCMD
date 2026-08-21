const LEGACY_SCAN_TABLES = [
  "scans",
  "scan_issues",
  "scan_sessions",
  "session_issues",
  "code_scans",
  "code_scan_issues",
];

const RETIRED_SCAN_COMMANDS = [
  "scan_url",
  "scan_multi",
  "run_code_scan",
  "get_scan_history",
  "get_code_scan_history",
  "get_scan_detail",
  "get_code_scan_detail",
  "get_session_history",
  "get_session_issues",
];

const MIGRATION_OR_TEST_FILE =
  /(?:\/migrations(?:\/|\.rs$)|schema_snapshot\.sql$|(?:^|\/)(?:tests\.rs|[^/]+_tests\.rs)|\.(?:test|spec)\.[cm]?[jt]sx?$)/;

// Flat alternatives avoid the nested quantifiers rejected by the regex audit.
const TEST_MOD_PLAIN = /^#\[cfg\(test\)\]\s*mod\s+tests\b/;
const TEST_MOD_WITH_PATH = /^#\[cfg\(test\)\]\s*#\[path\s*=\s*"[^"]*"\]\s*mod\s+tests\b/;

function stripInlineRustTests(source) {
  for (const match of source.matchAll(/#\[cfg\(test\)\]/g)) {
    const rest = source.slice(match.index);
    if (TEST_MOD_PLAIN.test(rest) || TEST_MOD_WITH_PATH.test(rest)) {
      return source.slice(0, match.index);
    }
  }
  return source;
}

function sourceFiles(listFiles) {
  const sourcePredicate = (file) => /\.(?:rs|sql|ts|tsx|mjs)$/.test(file);
  return [
    ...listFiles("apps/desktop/src-tauri/src", sourcePredicate),
    ...listFiles("apps/desktop/src", sourcePredicate),
    ...listFiles("apps/mcp-server/src", sourcePredicate),
  ];
}

function productionSource(read, file) {
  const source = read(file);
  return file.endsWith(".rs") ? stripInlineRustTests(source) : source;
}

export function unifiedScanArchitectureFailures(read, exists, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };
  const files = sourceFiles(listFiles);
  const productionFiles = files.filter((file) => !MIGRATION_OR_TEST_FILE.test(file));

  const legacySql = new RegExp(
    // Escapes keep the quote/backtick character class inside this template literal.
    // eslint-disable-next-line no-useless-escape
    `\\b(?:FROM|JOIN|INTO|UPDATE|DELETE\\s+FROM|CREATE\\s+TABLE|DROP\\s+TABLE|ALTER\\s+TABLE|REFERENCES)\\s+[\\\"\\\`]?(?:${LEGACY_SCAN_TABLES.join("|")})\\b`,
  );
  const legacySqlFiles = productionFiles.filter((file) =>
    legacySql.test(productionSource(read, file)),
  );
  check(
    legacySqlFiles.length === 0,
    `Production code must use scan_executions/scan_runs/scan_findings, never legacy scan tables: ${legacySqlFiles.join(", ")}`,
  );

  const pathBearingCodeId = /code_scan\.[A-Za-z0-9_.-]+:[A-Za-z0-9_./\\-]+/;
  const pathBearingFiles = productionFiles.filter((file) =>
    pathBearingCodeId.test(productionSource(read, file)),
  );
  check(
    pathBearingFiles.length === 0,
    `Canonical Code check IDs must stay rule-level; put files and lines in occurrence fields: ${pathBearingFiles.join(", ")}`,
  );

  const parserFiles = productionFiles.filter((file) => {
    const source = productionSource(read, file);
    return (
      source.includes("normalize_code_check_id") ||
      /(?:check_id|checkId)[^;\n]{0,100}\.split(?:_once)?\s*\(\s*['"]:/.test(source) ||
      /LIKE\s+['"]code_scan\.%:%/i.test(source)
    );
  });
  check(
    parserFiles.length === 0,
    `Production consumers must not parse or pattern-match canonical Code IDs for locations: ${parserFiles.join(", ")}`,
  );

  const commandFiles = [
    "apps/desktop/src-tauri/build.rs",
    "apps/desktop/src-tauri/src/lib.rs",
    "apps/desktop/src/lib/commands/scan.ts",
    "apps/desktop/src/lib/tauri-invoke.ts",
    ...listFiles("apps/desktop/src-tauri/permissions", (file) => /\.(?:toml|json)$/.test(file)),
  ].filter(exists);
  const retiredCommand = new RegExp(`["'](?:${RETIRED_SCAN_COMMANDS.join("|")})["']`);
  const retiredCommandFiles = commandFiles.filter((file) => retiredCommand.test(read(file)));
  check(
    retiredCommandFiles.length === 0,
    `Retired split scan IPC commands must not return; use run_scan_execution and execution history: ${retiredCommandFiles.join(", ")}`,
  );

  // Inspect each sibling module directly so a misplaced rule cannot pass on an
  // empty match.
  const executionFile = "apps/desktop/src-tauri/src/commands/scan/execution.rs";
  const verificationFile = "apps/desktop/src-tauri/src/commands/scan/verification.rs";
  const execution = exists(executionFile) ? read(executionFile) : "";
  const verification = exists(verificationFile) ? read(verificationFile) : "";
  const coverageStart = verification.indexOf("required_web_verification_ids(&check_ids)");
  const emptyCoverage = verification.indexOf("if coverage.is_empty()", coverageStart);
  const boundedAdmission = verification.indexOf(
    "admission_class: ScanAdmissionClass::BoundedVerification",
    emptyCoverage,
  );
  check(
    execution.includes(
      "admission_request(&plan, fingerprint, ScanAdmissionClass::GeneralScan, now)",
    ) &&
      coverageStart >= 0 &&
      emptyCoverage > coverageStart &&
      boundedAdmission > emptyCoverage &&
      !/match\s+(?:plan\.)?trigger[\s\S]{0,300}ScanAdmissionClass::(?:BoundedVerification|SystemExempt)/.test(
        execution + verification,
      ),
    `${verificationFile} must derive quota exemption from validated bounded coverage, never from the trigger label.`,
  );

  const issueStatesFile = "apps/desktop/src-tauri/src/db/issue_states.rs";
  const issueStates = exists(issueStatesFile) ? productionSource(read, issueStatesFile) : "";
  check(
    issueStates.includes("validate_canonical_check_id(check_id)") &&
      issueStates.includes("self.set_issue_state(") &&
      !/resolve_code_issue_state_ids|LIKE\s+['"]code_scan\.%:%|for\s+.*location/.test(issueStates),
    `${issueStatesFile} must validate and write one canonical lifecycle row without location fan-out.`,
  );

  const issuesPageFile = "apps/desktop/src/pages/IssuesPage.tsx";
  const issueGroupsFile = "apps/desktop/src/pages/issues/useInactiveIssueKeys.ts";
  const issueSummaryFile = "apps/desktop/src/lib/project-issue-summary.ts";
  const issuesPage = exists(issuesPageFile) ? read(issuesPageFile) : "";
  const issueGroups = exists(issueGroupsFile) ? read(issueGroupsFile) : "";
  const issueSummary = exists(issueSummaryFile) ? read(issueSummaryFile) : "";
  check(
    issuesPage.includes("rankIssueGroups(visibleIssueGroups)") &&
      issueGroups.includes("getWorkItems({ projectId, envUrl: normalizedUrl })") &&
      !issuesPage.includes("rankUnified(") &&
      !issuesPage.includes("latestResult?.issues") &&
      !issuesPage.includes("latestCodeResult?.issues") &&
      !issueSummary.includes("buildActiveProjectIssueSummary"),
    "Active Issues must rank canonical backend IssueGroup rows, never merge separate Web and Code arrays.",
  );

  return failures;
}
