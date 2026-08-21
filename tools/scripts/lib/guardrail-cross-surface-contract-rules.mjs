export function crossSurfaceContractFailures(read) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const mcpDbSource = [
    "apps/mcp-server/src/db.ts",
    "apps/mcp-server/src/db_connection.ts",
    "apps/mcp-server/src/db_correlation.ts",
    "apps/mcp-server/src/db_fix_attempts.ts",
    "apps/mcp-server/src/db_manifests.ts",
  ]
    .map(read)
    .join("\n");
  const mcpWorkspaceSource = read("apps/mcp-server/src/workspace.ts");
  const mcpCausalGraphSource = read("apps/mcp-server/src/causal_graph.ts");
  const desktopSeveritySource = read("apps/desktop/src/lib/severity.ts");
  const mcpServerSeveritySource = read("apps/mcp-server/src/severity.ts");
  const canonicalSeverityVocabulary = `["critical", "high", "medium", "low"]`;
  check(
    desktopSeveritySource.includes("critical: 0,") &&
      desktopSeveritySource.includes("high: 1,") &&
      desktopSeveritySource.includes("medium: 2,") &&
      desktopSeveritySource.includes("low: 3,") &&
      mcpServerSeveritySource.includes(canonicalSeverityVocabulary) &&
      mcpDbSource.includes('from "./severity.js"') &&
      mcpWorkspaceSource.includes('from "./severity.js"') &&
      mcpCausalGraphSource.includes('from "./severity.js"') &&
      !mcpDbSource.includes("const SEVERITY_ORDER") &&
      !mcpWorkspaceSource.includes("switch (severity)") &&
      !mcpCausalGraphSource.includes("SEVERITY_RANK"),
    "Severity ordering must match between apps/desktop/src/lib/severity.ts and apps/mcp-server/src/severity.ts, and MCP DB/workspace/causal ranking must import the shared MCP severity helpers.",
  );

  const rustBlameTestsPath = "apps/desktop/src-tauri/src/core/regression_blame_tests.rs";
  const alertModelTestsPath = "apps/desktop/src/components/alerts/alert-detail-model.test.ts";
  const detailFixture = read(rustBlameTestsPath).match(
    /const DETAIL_FIXTURE[^=]*=\s*r#"([\s\S]*?)"#/,
  )?.[1];
  const rustFixture = read(alertModelTestsPath).match(/const RUST_FIXTURE\s*=\s*`([\s\S]*?)`/)?.[1];
  check(
    typeof detailFixture === "string",
    `Could not extract the DETAIL_FIXTURE raw-string literal (r#"..."#) from ${rustBlameTestsPath}; the deploy-regression fixture parity check needs it.`,
  );
  check(
    typeof rustFixture === "string",
    `Could not extract the RUST_FIXTURE template literal from ${alertModelTestsPath}; the deploy-regression fixture parity check needs it.`,
  );
  check(
    detailFixture === undefined || rustFixture === undefined || detailFixture === rustFixture,
    `Deploy-regression detail fixtures must stay byte-identical: ${alertModelTestsPath} RUST_FIXTURE has drifted from ${rustBlameTestsPath} DETAIL_FIXTURE.`,
  );

  for (const { capability, identifier, broker } of [
    {
      capability: "apps/desktop/src-tauri/capabilities/data-admin.json",
      identifier: "sitecmd-data-admin",
      broker: "allow-run-data-admin-command",
    },
    {
      capability: "apps/desktop/src-tauri/capabilities/external-connectors.json",
      identifier: "sitecmd-external-connectors",
      broker: "allow-run-external-connector-command",
    },
    {
      capability: "apps/desktop/src-tauri/capabilities/filesystem-access.json",
      identifier: "sitecmd-filesystem-access",
      broker: "allow-run-filesystem-access-command",
    },
    {
      capability: "apps/desktop/src-tauri/capabilities/filesystem-export.json",
      identifier: "sitecmd-filesystem-export",
      broker: "allow-run-filesystem-export-command",
    },
    {
      capability: "apps/desktop/src-tauri/capabilities/project-execution.json",
      identifier: "sitecmd-project-execution",
      broker: "allow-run-project-execution-command",
    },
  ]) {
    const parsed = JSON.parse(read(capability));
    const grantedPermissions = Array.isArray(parsed.permissions) ? parsed.permissions : [];
    check(
      !grantedPermissions.includes(identifier) && grantedPermissions.includes(broker),
      `${capability} must NOT include the elevated permission set (${identifier}) on its bridge window. The bridge should only hold the broker dispatch (${broker}); elevated commands run as Rust function calls inside the broker, not as Tauri IPC, so the broker's token_state.consume() cannot be skipped.`,
    );
  }

  const appContentSource = read("apps/desktop/src/app/AppContent.tsx");
  check(
    appContentSource.includes("const navigationContextValue = useMemo<NavigationContextValue>"),
    "AppContent.tsx must wrap navigationContextValue in useMemo. An unmemoized context value forces every useNavigation() consumer (every routed page) to re-render on every shell-level state change.",
  );

  const googleOauthSource = read("apps/desktop/src-tauri/src/integrations/google_oauth.rs");
  check(
    googleOauthSource.includes("expected_state") &&
      googleOauthSource.includes("returned_state == &expected_state") &&
      googleOauthSource.includes("code_challenge") &&
      googleOauthSource.includes("code_challenge_method=S256") &&
      googleOauthSource.includes("code_verifier"),
    "google_oauth.rs must verify the OAuth state parameter and use PKCE (code_challenge_method=S256 + code_verifier on token exchange). Removing either re-opens CSRF / authorization-code-injection attacks against the desktop OAuth callback.",
  );
  const githubOauthSource = read("apps/desktop/src-tauri/src/integrations/github_oauth.rs");
  check(
    githubOauthSource.includes("device_code") &&
      githubOauthSource.includes("urn:ietf:params:oauth:grant-type:device_code"),
    "github_oauth.rs must use the device-code flow (no redirect URI, user manually enters the code). Switching to a redirect-based flow without adding state+PKCE would re-open CSRF.",
  );

  const sitemapSource = read("apps/desktop/src-tauri/src/core/sitemap.rs");
  check(
    sitemapSource.includes("async fn validate_sitemap_target(") &&
      /validate_sitemap_target\(&?sitemap_url, allow_local_dev\)/.test(sitemapSource) &&
      /validate_sitemap_target\(&?child_url, allow_local_dev\)/.test(sitemapSource),
    "core::sitemap must call validate_sitemap_target on every attacker-supplied URL (robots.txt Sitemap: directive AND <sitemapindex> child entry) before fetching. Removing either call re-opens an SSRF vector via crafted target sites.",
  );

  const projectPathsSource = read("apps/desktop/src-tauri/src/project_paths.rs");
  check(
    projectPathsSource.includes(
      "crate::core::code_scan::validate_project_path(Path::new(trimmed))",
    ),
    "project_paths::canonicalize_project_dir must route through validate_project_path so renderer-supplied project paths are bounded to the user's home directory.",
  );

  const projectCommandsSource = read("apps/desktop/src-tauri/src/commands/project.rs");
  check(
    projectCommandsSource.includes("crate::project_paths::canonicalize_project_dir(&path)") &&
      (projectCommandsSource.match(/canonicalize_project_dir/g) ?? []).length >= 3,
    "commands::project::detect_project_urls and update_project_path must route their renderer-supplied path argument through project_paths::canonicalize_project_dir so the home-dir bounds check (validate_project_path) is enforced before any filesystem walk.",
  );

  return failures;
}
