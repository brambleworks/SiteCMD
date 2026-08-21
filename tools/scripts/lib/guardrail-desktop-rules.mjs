import { lineNumberFor } from "./guardrail-text-utils.mjs";

const DESKTOP_SCAN_LOG_FILES = [
  "apps/desktop/src-tauri/src/commands/scan/web_scan.rs",
  "apps/desktop/src-tauri/src/commands/scan/multi_scan.rs",
  "apps/desktop/src-tauri/src/commands/scan/tools.rs",
  "apps/desktop/src-tauri/src/background/scan_scheduler.rs",
  "apps/desktop/src-tauri/src/checks/polish/css_fetch.rs",
  "apps/desktop/src-tauri/src/core/sitemap.rs",
];

const TRACING_MACROS = ["debug", "error", "info", "trace", "warn"].map(
  (level) => `tracing::${level}!(`,
);

export function desktopScanLogSafetyFailures(read) {
  const failures = [];
  const sanitizerSource = read("apps/desktop/src-tauri/crates/engine/src/log_sanitizer.rs");
  if (
    !sanitizerSource.includes("pub fn log_safe_url_target") ||
    !sanitizerSource.includes("log_safe_url_target_removes_query_fragment_and_path_tokens")
  ) {
    failures.push("apps/desktop/src-tauri/crates/engine/src/log_sanitizer.rs");
  }

  for (const file of DESKTOP_SCAN_LOG_FILES) {
    const source = read(file);
    let logsUrl = false;
    for (const call of tracingMacroCalls(source)) {
      if (tracingCallLogsRawUrl(call.text)) {
        logsUrl = true;
        failures.push(`${file}:${lineNumberFor(source, call.index)}`);
      }
    }
    if (logsUrl && !source.includes("log_safe_url_target")) failures.push(file);
  }

  return failures;
}

export function desktopFrontendLogSafetyFailures(read) {
  const failures = [];
  const loggerSource = read("apps/desktop/src/lib/logger.ts");
  const loggerTests = read("apps/desktop/src/lib/logger.test.ts");
  const dataCommands = read("apps/desktop/src-tauri/src/commands/data/diagnostics.rs");

  if (!(
    loggerSource.includes("sanitizeFrontendLogText") &&
    loggerSource.includes("sanitizeFrontendLogText(message)") &&
    loggerSource.includes("sanitizeFrontendLogText(context)") &&
    loggerTests.includes("redacts urls, paths, emails, and secrets before sending logs to Rust") &&
    loggerTests.includes("truncates long frontend log messages before persistence")
  )) {
    failures.push("apps/desktop/src/lib/logger.ts");
  }

  if (!(
    dataCommands.includes("fn sanitize_frontend_log_text") &&
    dataCommands.includes("redact_diagnostic_text(value)") &&
    dataCommands.includes("let message = sanitize_frontend_log_text(&message);") &&
    dataCommands.includes("map(sanitize_frontend_log_text)") &&
    dataCommands.includes("sanitize_frontend_log_text_redacts_before_persistent_logging") &&
    dataCommands.includes("sanitize_frontend_log_text_truncates_before_persistent_logging")
  )) {
    failures.push("apps/desktop/src-tauri/src/commands/data/diagnostics.rs");
  }

  return failures;
}

export function desktopProjectCommandSafetyFailures(read) {
  const source = read("apps/desktop/src-tauri/src/commands/desktop_project_commands.rs");
  const tests = read("apps/desktop/src-tauri/src/commands/desktop_tests.rs");
  const constants = read("apps/desktop/src-tauri/src/constants.rs");
  const blockedAliases = [
    '"build"',
    '"explore"',
    '"rebuild"',
    '"remove"',
    '"start"',
    '"test"',
    '"approve-builds"',
    '"pm" && subcommand == "trust"',
  ];

  if (
    source.includes("installer_requires_script_opt_out") &&
    source.includes("installer_must_run_manually") &&
    source.includes("package_manager_script_alias_must_run_manually") &&
    blockedAliases.every((alias) => source.includes(alias)) &&
    source.includes("!SAFE_COMMANDS.contains(&command)") &&
    source.includes("Project commands must put the command name before flags") &&
    source.includes("--ignore-scripts") &&
    source.includes("--no-scripts") &&
    tests.includes("rejects_package_manager_lifecycle_and_script_aliases") &&
    tests.includes('npm", vec!["rebuild"') &&
    tests.includes('npm", vec!["remove"') &&
    tests.includes('pnpm", vec!["approve-builds"') &&
    tests.includes('pnpm", vec!["remove"') &&
    tests.includes('yarn", vec!["build"') &&
    tests.includes('yarn", vec!["remove"') &&
    tests.includes('bun", vec!["remove"') &&
    tests.includes('bun", vec!["pm", "trust"') &&
    tests.includes("rejects_installs_that_can_run_lifecycle_scripts") &&
    tests.includes("allows_installs_with_script_opt_out") &&
    tests.includes("rejects_false_script_opt_out_values") &&
    tests.includes("rejects_leading_flags_that_hide_the_command") &&
    tests.includes("blocks_installers_without_safe_script_opt_outs") &&
    constants.includes("PROJECT_COMMAND_OUTPUT_DRAIN_TIMEOUT") &&
    source.includes("collect_project_command_output") &&
    source.includes("task.abort()") &&
    source.includes("security_regression_project_command_output_reader_has_timeout")
  ) {
    return [];
  }

  return [
    "desktop project commands must block installer lifecycle scripts and cap inherited stdout/stderr pipe draining after timeouts.",
  ];
}

export function desktopNetworkProbeSafetyFailures(read) {
  const scanTools = read("apps/desktop/src-tauri/src/commands/scan/tools.rs");
  const pagespeed = read("apps/desktop/src-tauri/src/integrations/pagespeed.rs");
  if (
    scanTools.includes("async fn validate_webview_analysis_url") &&
    scanTools.includes("validate_webview_analysis_url(&url).await?") &&
    scanTools.includes("security_regression_webview_analysis_rejects_private_network_targets") &&
    pagespeed.includes(
      "crate::network_policy::validate_url(url, crate::network_policy::UrlPolicy::Scan).await?",
    ) &&
    pagespeed.includes("security_regression_pagespeed_rejects_private_network_targets")
  ) {
    return [];
  }

  return [
    "desktop webview-analysis and PageSpeed commands must validate URLs with the shared scan SSRF policy before touching network targets.",
  ];
}

function tracingCallLogsRawUrl(callText) {
  const normalizedCall = callText.replace(
    /(?:crate::log_sanitizer::)?log_safe_url_target\([^)]*\)/g,
    "SAFE_URL",
  );
  return (
    /(?:^|[,\s])(?:child_url|env_url|environment_url|monitor_url|page_url|sitemap_url|url)\s*=\s*%(?:child_url|env_url|environment_url|monitor_url|page_url|sitemap_url|url)\b/.test(
      normalizedCall,
    ) ||
    /(?:^|[,(]\s*)&?\b(?:child_url|env_url|environment_url|monitor_url|page_url|sitemap_url|url)\b\s*(?:,|\))/m.test(
      normalizedCall,
    ) ||
    /\b(?:child_url|env_url|environment_url|monitor_url|page_url|sitemap_url|url)\.(?:clone|to_string|as_str)\(\)\s*(?:,|\))/.test(
      normalizedCall,
    )
  );
}

function tracingMacroCalls(source) {
  const calls = [];
  let cursor = 0;

  while (cursor < source.length) {
    const next = nearestTracingMacro(source, cursor);
    if (!next) break;
    const body = readMacroCall(source, next.index + next.macro.length - 1);
    if (!body) {
      cursor = next.index + next.macro.length;
      continue;
    }
    calls.push({ index: next.index, text: body.text });
    cursor = body.end;
  }

  return calls;
}

function nearestTracingMacro(source, start) {
  return TRACING_MACROS.map((macro) => ({ macro, index: source.indexOf(macro, start) }))
    .filter((match) => match.index !== -1)
    .sort((left, right) => left.index - right.index)[0];
}

function readMacroCall(source, openParenIndex) {
  let quote = null;
  let depth = 0;

  for (let cursor = openParenIndex; cursor < source.length; cursor += 1) {
    const ch = source[cursor];
    if (quote) {
      if (ch === quote && source[cursor - 1] !== "\\") quote = null;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      continue;
    }
    if (ch === "(") {
      depth += 1;
    } else if (ch === ")") {
      depth -= 1;
      if (depth === 0) {
        return { end: cursor + 1, text: source.slice(openParenIndex, cursor + 1) };
      }
    }
  }

  return null;
}
