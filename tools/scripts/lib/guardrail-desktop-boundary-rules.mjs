import { baselineGuideShapeFailures } from "./guardrail-baseline-guides-rules.mjs";
import { capabilitySecurityFailures } from "./guardrail-capability-security-rules.mjs";
import {
  commandSecurityManifestFailures,
  functionBodyContains,
} from "./guardrail-command-security-manifest-rules.mjs";
import { confirmDeadlineFailures } from "./guardrail-confirm-deadline-rules.mjs";
import { codeScanSecurityFailures } from "./guardrail-code-scan-security-rules.mjs";
import {
  desktopFrontendLogSafetyFailures,
  desktopNetworkProbeSafetyFailures,
  desktopProjectCommandSafetyFailures,
  desktopScanLogSafetyFailures,
} from "./guardrail-desktop-rules.mjs";
import { desktopLicensingSafetyFailures } from "./guardrail-desktop-licensing-rules.mjs";
import { fixGuideCspGuidanceFailures } from "./guardrail-fix-guide-csp-rules.mjs";
import { licenseCodeUnionFailures } from "./guardrail-license-code-union-rules.mjs";
import { licenseSurfaceFailures } from "./guardrail-license-surface-rules.mjs";
import { licenseLifecycleSafetyFailures } from "./guardrail-license-validation-rules.mjs";
import { manifestPublicationFailures } from "./guardrail-manifest-publication-rules.mjs";
import { privilegedBrokerTokenMarkerFailures } from "./guardrail-privileged-broker-rules.mjs";
import { releaseWorkflowSafetyFailures } from "./guardrail-release-workflow-rules.mjs";
import { tauriCspSafetyFailures } from "./guardrail-tauri-csp-rules.mjs";
import { tracingInstrumentAttributes } from "./guardrail-text-utils.mjs";
import { updaterTrustFailures } from "./guardrail-updater-trust-rules.mjs";

export function desktopBoundaryFailures(read, readJson, exists, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const tauriRustFiles = listFiles("apps/desktop/src-tauri/src", (file) => file.endsWith(".rs"));
  const unsafeTracingFields = [];
  const unsafeTracingFieldNames = [
    "raw",
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "credential",
    "authorization",
    "database_url",
    "db_url",
    "path",
    "src_path",
    "dest_path",
    "source_path",
    "project_path",
    "project_root",
    "root",
    "env_file_path",
    "working_dir",
    "dir",
    "url",
    "site_url",
    "page_url",
    "env_url",
    "environment_url",
    "monitor_url",
    "sitemap_url",
  ];
  for (const file of tauriRustFiles) {
    const source = read(file);
    for (const attribute of tracingInstrumentAttributes(source)) {
      const normalizedAttribute = attribute.text.toLowerCase().replace(/\s+/g, "");
      const hasUnsafeTracingField =
        normalizedAttribute.includes("fields(") &&
        unsafeTracingFieldNames.some(
          (field) =>
            normalizedAttribute.includes(`fields(${field}=`) ||
            normalizedAttribute.includes(`,${field}=`),
        );
      if (hasUnsafeTracingField) unsafeTracingFields.push(`${file}:${attribute.line}`);
    }
  }
  check(
    unsafeTracingFields.length === 0,
    `tracing::instrument fields must not record raw or secret-like values; use skip(...) and log parsed safe metadata instead: ${unsafeTracingFields.join(", ")}`,
  );

  const unsafeDesktopScanUrlLogs = desktopScanLogSafetyFailures(read);
  check(
    unsafeDesktopScanUrlLogs.length === 0,
    `Desktop scan logs must use log_safe_url_target before writing scan URLs to persistent logs: ${unsafeDesktopScanUrlLogs.join(", ")}`,
  );
  const unsafeDesktopFrontendLogs = desktopFrontendLogSafetyFailures(read);
  check(
    unsafeDesktopFrontendLogs.length === 0,
    `Desktop frontend logs must redact and truncate sensitive text before writing to persistent logs: ${unsafeDesktopFrontendLogs.join(", ")}`,
  );
  const unsafeDesktopLicensing = desktopLicensingSafetyFailures(read);
  check(
    unsafeDesktopLicensing.length === 0,
    `Desktop licensing and checkout secret handling must use hashed non-PII fingerprints, must not echo sensitive response bodies, and must honour Lemon Squeezy on_trial entitlements: ${unsafeDesktopLicensing.join(", ")}`,
  );
  failures.push(...updaterTrustFailures(read, exists));
  failures.push(...licenseLifecycleSafetyFailures(read));
  failures.push(...licenseCodeUnionFailures(read));
  failures.push(...licenseSurfaceFailures(read, listFiles));
  failures.push(...privilegedBrokerTokenMarkerFailures(read));
  failures.push(...confirmDeadlineFailures(read));
  failures.push(...desktopProjectCommandSafetyFailures(read));
  failures.push(...desktopNetworkProbeSafetyFailures(read));

  const releaseWorkflowFailures = releaseWorkflowSafetyFailures(read);
  check(
    releaseWorkflowFailures.length === 0,
    `Release workflow guardrails failed: ${releaseWorkflowFailures.join("; ")}`,
  );
  failures.push(...manifestPublicationFailures(read, listFiles));
  failures.push(...tauriCspSafetyFailures(read, exists));
  failures.push(...capabilitySecurityFailures(read, readJson, exists, listFiles));
  failures.push(...commandSecurityManifestFailures(read, readJson, listFiles));

  const dataExports = read("apps/desktop/src-tauri/src/commands/data/exports.rs");
  check(
    dataExports.includes("persist_noclobber(target)"),
    "Export file writes must use no-clobber persistence unless native overwrite approval was captured.",
  );
  check(
    functionBodyContains(
      dataExports,
      "confirm_export_write",
      "validate_export_write_path(path)?",
    ) &&
      functionBodyContains(dataExports, "confirm_export_write", "confirm_sensitive_action(") &&
      !functionBodyContains(dataExports, "confirm_export_write", "return Ok(false);") &&
      functionBodyContains(
        dataExports,
        "write_export_file",
        "confirm_export_write(app, &path).await?",
      ) &&
      functionBodyContains(
        dataExports,
        "write_export_bytes",
        "confirm_export_write(app, &path).await?",
      ) &&
      !dataExports.includes("confirm_export_overwrite_if_needed("),
    "Desktop export writes must require native confirmation before creating or replacing files.",
  );

  const codeScanCommands = read("apps/desktop/src-tauri/src/commands/code_scan.rs");
  check(
    !codeScanCommands.includes("_for_tier(") &&
      !read("apps/desktop/src-tauri/src/licensing/mod.rs").includes("pub mod redaction"),
    "Tier redaction is retired with the free complete workbench: Code Scan results serve every tier the same payload, and the redaction module must stay deleted.",
  );
  failures.push(...codeScanSecurityFailures(read));

  const desktopWebhooksSource = read("apps/desktop/src-tauri/src/webhooks.rs");
  check(
    desktopWebhooksSource.includes("webhook_log_target") &&
      desktopWebhooksSource.includes("redact_webhook_url_from_error") &&
      desktopWebhooksSource.includes("webhook_error_redaction_replaces_full_destination_urls") &&
      !desktopWebhooksSource.includes('Webhook delivered to {}", url') &&
      !desktopWebhooksSource.includes('Webhook to {} failed: {}", url'),
    "Desktop webhook delivery logs must redact full destination URLs before logging delivery results.",
  );

  const destructiveDbDeletes = [
    ["apps/desktop/src-tauri/src/db/projects.rs", "delete_environment"],
    ["apps/desktop/src-tauri/src/db/scans.rs", "clear_scan_history"],
    ["apps/desktop/src-tauri/src/db/scans.rs", "delete_scan"],
    ["apps/desktop/src-tauri/src/db/scans.rs", "delete_site_scans"],
    ["apps/desktop/src-tauri/src/db/scan_retention.rs", "prune_scan_executions_for_scope"],
  ];
  const nonTransactionalDeletes = destructiveDbDeletes
    .filter(
      ([file, functionName]) => !functionBodyContains(read(file), functionName, ".transaction("),
    )
    .map(([file, functionName]) => `${file}::${functionName}`);
  check(
    nonTransactionalDeletes.length === 0,
    `Multi-step destructive DB deletes must run inside a transaction: ${nonTransactionalDeletes.join(", ")}`,
  );

  failures.push(...fixGuideCspGuidanceFailures(read, listFiles));
  failures.push(...baselineGuideShapeFailures(read, listFiles));

  const packageJson = JSON.parse(read("package.json"));
  const guardrailTestSweep = packageJson.scripts?.["guardrails:repo:test"] ?? "";
  check(
    guardrailTestSweep.startsWith("vitest run ") &&
      guardrailTestSweep.split(/\s+/).includes("tools/scripts"),
    "Repo guardrails must run the full tools/scripts vitest sweep from root pnpm test.",
  );

  return failures;
}
