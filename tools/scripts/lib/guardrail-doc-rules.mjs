export function documentationSafetyFailures(read, exists, listFiles) {
  const failures = [];

  const guidanceFiles = [
    "CLAUDE.md",
    "AGENTS.md",
    ...listFiles("apps", (file) => /(^|\/)(CLAUDE|AGENTS|README)\.md$/i.test(file)),
  ].filter(exists);
  const docFiles = [
    "README.md",
    ...listFiles("docs", (file) => file.endsWith(".md")),
    ...guidanceFiles,
  ];
  const localAbsoluteMarkdownLinks = docFiles.filter((file) =>
    /\]\(\/Users\/[^)]+\)/.test(read(file)),
  );
  if (localAbsoluteMarkdownLinks.length > 0) {
    failures.push(
      `Documentation must not use machine-specific absolute Markdown links: ${localAbsoluteMarkdownLinks.join(", ")}`,
    );
  }

  const staleAdminKeyGuidanceFiles = guidanceFiles.filter((file) =>
    /\?key=<ADMIN_KEY>|auth via `\?key=<ADMIN_KEY>`/.test(read(file)),
  );
  if (staleAdminKeyGuidanceFiles.length > 0) {
    failures.push(
      `Repo guidance must not document query-string admin keys: ${staleAdminKeyGuidanceFiles.join(", ")}`,
    );
  }

  const staleTypecheckGuidanceFiles = guidanceFiles.filter((file) =>
    /No [`"']?(lint|typecheck)[`"']? or [`"']?(lint|typecheck)[`"']? scripts exist|No [`"']?typecheck[`"']? scripts exist/.test(
      read(file),
    ),
  );
  if (staleTypecheckGuidanceFiles.length > 0) {
    failures.push(
      `Repo guidance must not claim missing typecheck scripts after quality gates exist: ${staleTypecheckGuidanceFiles.join(", ")}`,
    );
  }

  const staleArchitectureGuidanceFiles = guidanceFiles.filter((file) =>
    /core\/guardrails\.rs|AppGuardrailsReport|gate lives in the frontend display layer|New commands must[^.\n]+capabilities\/default\.json|Add to [`"']?capabilities\/default\.json|credentials fall back to SQLite/.test(
      read(file),
    ),
  );
  if (staleArchitectureGuidanceFiles.length > 0) {
    failures.push(
      `Repo guidance must not describe stale Code Scan or Tauri capability architecture: ${staleArchitectureGuidanceFiles.join(", ")}`,
    );
  }

  const firstRunFlowDocs = [
    "docs/product/get-value-in-5-minutes.md",
    "docs/qa/manual-testing-runbook.md",
    "docs/qa/acceptance-review-template.md",
  ].filter(exists);
  const staleFirstRunFlowDocs = firstRunFlowDocs.filter((file) =>
    /first tracked Web Scan|first Web Scan fix|before the first scan|first scan leads naturally into the Issues page|Issues is the clear action center|Today all tell a consistent story|Go straight to Issues|land in Issues|Click \*\*Scan\*\*/i.test(
      read(file),
    ),
  );
  if (staleFirstRunFlowDocs.length > 0) {
    failures.push(
      `First-run docs must describe the Full Scan -> Dashboard guided flow, not the old Web Scan -> Issues flow: ${staleFirstRunFlowDocs.join(", ")}`,
    );
  }

  const mcpReadmeSource = read("apps/mcp-server/README.md");
  const mcpClaudeSource = read("apps/mcp-server/CLAUDE.md");
  const mcpPackage = JSON.parse(read("apps/mcp-server/package.json"));
  const mcpMinimumNodeWorkflow = read(".github/workflows/frontend-quality.yml");
  const agentToolCards = read("apps/desktop/src/components/settings/AgentToolCards.tsx");
  if (
    mcpPackage.engines?.node !== ">=22.22.1" ||
    !/\*\*Node\.js\*\*\s+22\.22\.1\+ for manual setup/.test(mcpReadmeSource) ||
    !/node-version:\s*22\.22\.1/.test(mcpMinimumNodeWorkflow) ||
    !/Node 22\.22\.1 or newer on your PATH/.test(agentToolCards) ||
    /NODE_OPTIONS:\s*--experimental-sqlite/.test(mcpMinimumNodeWorkflow)
  ) {
    failures.push(
      "sitecmd-mcp package, README, desktop copy, and minimum-Node workflow must agree on the tested Node 22.22.1+ requirement.",
    );
  }

  const persistentMcpScriptPaths = [
    "Library/Application Support/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs",
    "$XDG_DATA_HOME/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs",
    ".local/share/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs",
    "%LOCALAPPDATA%\\com.sitecmd.app\\sitecmd-mcp\\sitecmd-mcp.mjs",
  ];
  if (
    persistentMcpScriptPaths.some((path) => !mcpReadmeSource.includes(path)) ||
    /Contents\/Resources\/sitecmd-mcp\/sitecmd-mcp\.mjs/.test(mcpReadmeSource)
  ) {
    failures.push(
      "sitecmd-mcp manual setup must use the persistent per-OS script paths, not app installation resources.",
    );
  }

  if (
    !mcpClaudeSource.includes("AGENTS.md") ||
    !mcpClaudeSource.includes("README.md") ||
    /\b(?:get_fix_brief|request_verification)\b|read-only boundary|tool set \(/.test(
      mcpClaudeSource,
    )
  ) {
    failures.push(
      "sitecmd-mcp CLAUDE.md must remain a pointer instead of copying mutable database or tool contracts.",
    );
  }

  const mcpToolSources = [
    "apps/mcp-server/src/server.ts",
    "apps/mcp-server/src/correlation_tools.ts",
  ];
  const mcpToolNames = mcpToolSources.flatMap((file) =>
    Array.from(read(file).matchAll(/registerTool\(\s*\n\s*"([^"]+)"/g), (match) => match[1]),
  );
  const legacyToolRegistrations = mcpToolSources.filter((file) =>
    /\bserver\.tool\(/.test(read(file)),
  );
  if (legacyToolRegistrations.length > 0) {
    failures.push(
      `sitecmd-mcp must register tools with registerTool and annotations, never the deprecated server.tool: ${legacyToolRegistrations.join(", ")}`,
    );
  }
  const undocumentedMcpTools = mcpToolNames.filter(
    (toolName) => !mcpReadmeSource.includes(`\`${toolName}\``),
  );
  if (undocumentedMcpTools.length > 0) {
    failures.push(
      `sitecmd-mcp README tool table must list every registered MCP tool: ${undocumentedMcpTools.join(", ")}`,
    );
  }
  if (
    exists("apps/mcp-server/recovery-runbook.md") &&
    !mcpReadmeSource.includes("recovery-runbook.md")
  ) {
    failures.push("sitecmd-mcp README must link the recovery runbook.");
  }
  if (/`request_scan`\s*\|\s*Ask SiteCMD to start or queue a scan/.test(mcpReadmeSource)) {
    failures.push(
      "sitecmd-mcp README must describe request_scan as guidance-only until it can actually queue desktop scans.",
    );
  }
  if (
    /Request a new scan in SiteCMD|start or queue desktop scans/.test(
      read("apps/mcp-server/src/server.ts"),
    )
  ) {
    failures.push(
      "sitecmd-mcp request_scan tool description must stay guidance-only until it can actually queue desktop scans.",
    );
  }

  const publicMcpDocFiles = ["apps/mcp-server/README.md"].filter(exists);
  const staleMcpToolDocFiles = publicMcpDocFiles.filter((file) =>
    /\b(list_projects|get_scan_results|list_issues|build_fix_prompt|get_score_trend)\b/.test(
      read(file),
    ),
  );
  if (staleMcpToolDocFiles.length > 0) {
    failures.push(
      `MCP docs must use current SiteCMD tool names instead of legacy aliases: ${staleMcpToolDocFiles.join(", ")}`,
    );
  }

  return failures;
}
