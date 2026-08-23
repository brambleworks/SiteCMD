import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect } from "vitest";
import { repoGuardrailFailures } from "./check-repo-guardrails.mjs";
import { OVERLAY_DELETED, overlayIo } from "./lib/repo-overlay-io.mjs";
import { orderedBefore } from "./lib/guardrail-text-utils.mjs";
import { agentGuidanceFailures } from "./lib/guardrail-agent-guidance-rules.mjs";
import { appShellNavFailures } from "./lib/guardrail-app-shell-nav-rules.mjs";
import { desktopCategoryLabelFailures } from "./lib/guardrail-category-rules.mjs";
import { cliSurfaceFailures } from "./lib/guardrail-cli-surface-rules.mjs";
import { codeScanInventoryFailures } from "./lib/guardrail-code-scan-inventory-rules.mjs";
import { codeScanSecurityFailures } from "./lib/guardrail-code-scan-security-rules.mjs";
import { codeOwnerSafetyFailures } from "./lib/guardrail-codeowners-rules.mjs";
import { commandWrapperFailures } from "./lib/guardrail-command-wrapper-rules.mjs";
import { desktopLicensingSafetyFailures } from "./lib/guardrail-desktop-licensing-rules.mjs";
import { desktopOAuthSafetyFailures } from "./lib/guardrail-desktop-oauth-rules.mjs";
import { desktopProjectCommandSafetyFailures } from "./lib/guardrail-desktop-rules.mjs";
import { handRolledDialogFailures } from "./lib/guardrail-dialog-rules.mjs";
import { documentationSafetyFailures } from "./lib/guardrail-doc-rules.mjs";
import { emDashFailures } from "./lib/guardrail-em-dash-rules.mjs";
import { emptyTestBodyFailures } from "./lib/guardrail-empty-test-body-rules.mjs";
import { ambientClockFailures, engineVocabFailures } from "./lib/guardrail-engine-vocab-rules.mjs";
import { eventFabricFailures } from "./lib/guardrail-event-fabric-rules.mjs";
import { fixGuideCspGuidanceFailures } from "./lib/guardrail-fix-guide-csp-rules.mjs";
import { desktopFrontendDisplayFailures } from "./lib/guardrail-frontend-display-rules.mjs";
import { desktopFrontendJsonSafetyFailures } from "./lib/guardrail-frontend-json-rules.mjs";
import { frontendMaintainabilityFailures } from "./lib/guardrail-frontend-maintainability-rules.mjs";
import { desktopFrontendStateFailures } from "./lib/guardrail-frontend-rules.mjs";
import { integrationUrlSecretFailures } from "./lib/guardrail-integration-url-secrets.mjs";
import { desktopIssueStatusFailures } from "./lib/guardrail-issue-rules.mjs";
import { issueStateSafetyFailures } from "./lib/guardrail-issue-state-rules.mjs";
import { licenseLifecycleSafetyFailures } from "./lib/guardrail-license-validation-rules.mjs";
import { mcpSchemaParityFailures } from "./lib/guardrail-mcp-schema-rules.mjs";
import { onboardingCopyFailures } from "./lib/guardrail-onboarding-copy-rules.mjs";
import { performanceGateFailures } from "./lib/guardrail-performance-rules.mjs";
import { publicationRecordFailures } from "./lib/guardrail-publication-record-rules.mjs";
import { pricingConsistencyFailures } from "./lib/guardrail-pricing-rules.mjs";
import { privateStorageSafetyFailures } from "./lib/guardrail-private-storage-rules.mjs";
import { privilegedTokenIssuerFailures } from "./lib/guardrail-privileged-token-rules.mjs";
import { queryLayerFailures } from "./lib/guardrail-query-layer-rules.mjs";
import { releaseArtifactSafetyFailures } from "./lib/guardrail-release-rules.mjs";
import { reportScoreConsistencyFailures } from "./lib/guardrail-report-score-rules.mjs";
import { displayImplLogReentrancyFailures } from "./lib/guardrail-rust-display-log-rules.mjs";
import { rustEventSeverityFailures } from "./lib/guardrail-rust-event-severity-rules.mjs";
import { rustLineBudgetFailures } from "./lib/guardrail-rust-loc-rules.mjs";
import { rustRatchetFailures } from "./lib/guardrail-rust-ratchets.mjs";
import { rustUnwrapBudgetFailures } from "./lib/guardrail-rust-rules.mjs";
import { rustSeverityConsistencyFailures } from "./lib/guardrail-rust-severity-rules.mjs";
import { rustlsCryptoProviderFailures } from "./lib/guardrail-rustls-provider-rules.mjs";
import { desktopScanLabelFailures } from "./lib/guardrail-scan-label-rules.mjs";
import { scanPersistOffThreadFailures } from "./lib/guardrail-scan-persist-offthread.mjs";
import { scanSchedulerPersistPathFailures } from "./lib/guardrail-scan-scheduler-rules.mjs";
import { desktopScannerBodySafetyFailures } from "./lib/guardrail-scanner-body-rules.mjs";
import { scoreArtifactLabelingFailures } from "./lib/guardrail-score-labeling-rules.mjs";
import { desktopScoreConsistencyFailures } from "./lib/guardrail-score-rules.mjs";
import { severityPolicyChokepointFailures } from "./lib/guardrail-severity-policy-rules.mjs";
import { desktopSeverityConsistencyFailures } from "./lib/guardrail-severity-rules.mjs";
import { desktopStyleConsistencyFailures } from "./lib/guardrail-style-rules.mjs";
import { supplyChainSafetyFailures } from "./lib/guardrail-supply-chain-rules.mjs";
import { supportEmailLiteralFailures } from "./lib/guardrail-support-email-rules.mjs";
import { tauriCspSafetyFailures } from "./lib/guardrail-tauri-csp-rules.mjs";
import { telemetryConsentFailures } from "./lib/guardrail-telemetry-consent-rules.mjs";
import { telemetryDisclosureFailures } from "./lib/guardrail-telemetry-disclosure-rules.mjs";
import { desktopSharedTypeFailures } from "./lib/guardrail-type-rules.mjs";
import { desktopUpdateCommandFailures } from "./lib/guardrail-update-rules.mjs";
import { desktopUrlIdentityFailures } from "./lib/guardrail-url-rules.mjs";
import { versionSyncFailures } from "./lib/guardrail-version-sync-rules.mjs";
import { updaterTrustFailures } from "./lib/guardrail-updater-trust-rules.mjs";
import { confirmDeadlineFailures } from "./lib/guardrail-confirm-deadline-rules.mjs";
import { asyncCommandDbBlockingFailures } from "./lib/guardrail-async-command-db-rules.mjs";
import {
  ciCostSafetyFailures,
  deployWorkflowHardeningFailures,
} from "./lib/guardrail-ci-cost-rules.mjs";
import {
  brokerOnlyRegistrationFailures,
  ungrantedIpcCommandFailures,
} from "./lib/guardrail-invoke-acl-rules.mjs";
import { parseSnapshotTables } from "./lib/guardrail-mcp-schema-rules.mjs";
import { telemetrySafetyFailures } from "./lib/guardrail-telemetry-rules.mjs";
import { unifiedScanArchitectureFailures } from "./lib/guardrail-unified-scan-rules.mjs";
import { workflowSafetyFailures } from "./lib/guardrail-workflow-rules.mjs";
import { releaseWorkflowSafetyFailures } from "./lib/guardrail-release-workflow-rules.mjs";
import { tailwindRemovalFailures } from "./lib/guardrail-tailwind-removal-rules.mjs";
import { commentQualityFailures } from "./lib/guardrail-comment-quality-rules.mjs";
import { licenseCodeUnionFailures } from "./lib/guardrail-license-code-union-rules.mjs";
import { licenseActivationErrorFailures } from "./lib/guardrail-license-activation-rules.mjs";
import { licenseSurfaceFailures } from "./lib/guardrail-license-surface-rules.mjs";
import { licenseValidationBranchFailures } from "./lib/guardrail-license-validation-branch-rules.mjs";
import { guardrailScriptLineBudgets } from "./lib/guardrail-script-budgets.mjs";
import { stripComments, stripNonCode } from "./lib/guardrail-source-text.mjs";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export const realRead = (relativePath) => fs.readFileSync(path.join(ROOT, relativePath), "utf8");
export const GUARDRAIL_TEST_TIMEOUT_MS = 300_000;

export function read(relativePath) {
  return fs.readFileSync(path.join(ROOT, relativePath), "utf8");
}

export function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

let cachedRepoFiles;

function repoFiles() {
  if (cachedRepoFiles) return cachedRepoFiles;
  const output = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    {
      cwd: ROOT,
    },
  ).toString("utf8");

  cachedRepoFiles = Array.from(new Set(output.split("\0").filter(Boolean)));
  return cachedRepoFiles;
}

export function copyRepoFixture() {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "sitecmd-guardrails-"));

  for (const relativePath of repoFiles()) {
    const sourcePath = path.join(ROOT, relativePath);
    if (!fs.existsSync(sourcePath)) continue;

    const destPath = path.join(fixtureRoot, relativePath);
    fs.mkdirSync(path.dirname(destPath), { recursive: true });

    const stat = fs.lstatSync(sourcePath);
    if (stat.isSymbolicLink()) {
      fs.symlinkSync(fs.readlinkSync(sourcePath), destPath);
    } else if (stat.isFile()) {
      fs.copyFileSync(sourcePath, destPath, fs.constants.COPYFILE_FICLONE);
    }
  }

  return fixtureRoot;
}

export function writeFixtureFile(fixture, relativePath, source) {
  if (fixture instanceof Map) {
    fixture.set(relativePath, source);
    return;
  }
  const filePath = path.join(fixture, relativePath);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, source);
}

export function readFixtureFile(fixture, relativePath) {
  if (fixture instanceof Map) {
    const pending = fixture.get(relativePath);
    if (pending === OVERLAY_DELETED) throw new Error(`fixture deleted ${relativePath}`);
    return pending ?? read(relativePath);
  }
  return fs.readFileSync(path.join(fixture, relativePath), "utf8");
}

export function deleteFixtureFile(fixture, relativePath) {
  if (fixture instanceof Map) {
    fixture.set(relativePath, OVERLAY_DELETED);
    return;
  }
  fs.rmSync(path.join(fixture, relativePath), { force: true });
}

export function mustMutate(source, searchValue, replaceValue) {
  const mutated = source.replaceAll(searchValue, replaceValue);
  if (mutated === source) {
    throw new Error(
      `negative-control mutation is a no-op; update the search string to match the current source: ${searchValue}`,
    );
  }
  return mutated;
}

export function runGuardrails(fixtureRoot, { cwd = fixtureRoot } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [path.join(ROOT, "tools/scripts/check-repo-guardrails.mjs")],
      {
        cwd,
        env: {
          ...process.env,
          SITECMD_GUARDRAILS_ROOT: fixtureRoot,
        },
      },
    );
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.setEncoding("utf8").on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (status) => resolve({ status, stdout, stderr }));
  });
}

const desktopSourceFiles = (io) =>
  io.listFiles("apps/desktop/src", (file) => /\.(ts|tsx|css)$/.test(file));
const RULE_ARGUMENTS = new Map([
  [repoGuardrailFailures, (io) => [io]],
  [agentGuidanceFailures, (io) => [io.read, io.exists, io.listFiles]],
  [ambientClockFailures, (io) => [io.read, io.listFiles]],
  [appShellNavFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [ciCostSafetyFailures, (io) => [io.read, io.listFiles]],
  [cliSurfaceFailures, (io) => [io.read, io.exists, io.listFiles]],
  [codeOwnerSafetyFailures, (io) => [io.read]],
  [codeScanInventoryFailures, (io) => [io.read]],
  [codeScanSecurityFailures, (io) => [io.read]],
  [commandWrapperFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopCategoryLabelFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopFrontendDisplayFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopFrontendJsonSafetyFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopFrontendStateFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopIssueStatusFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopLicensingSafetyFailures, (io) => [io.read]],
  [desktopOAuthSafetyFailures, (io) => [io.read]],
  [desktopProjectCommandSafetyFailures, (io) => [io.read]],
  [desktopScanLabelFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopScannerBodySafetyFailures, (io) => [io.read, io.listFiles]],
  [desktopScoreConsistencyFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopSeverityConsistencyFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopSharedTypeFailures, (io) => [io.read]],
  [desktopStyleConsistencyFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopUpdateCommandFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [desktopUrlIdentityFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [displayImplLogReentrancyFailures, (io) => [io.read, io.listFiles]],
  [documentationSafetyFailures, (io) => [io.read, io.exists, io.listFiles]],
  [emDashFailures, (io) => [io.read, io.exists, io.listFiles]],
  [emptyTestBodyFailures, (io) => [io.read, io.listFiles]],
  [engineVocabFailures, (io) => [io.read]],
  [eventFabricFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [fixGuideCspGuidanceFailures, (io) => [io.read, io.listFiles]],
  [
    frontendMaintainabilityFailures,
    (io) => [io.read, io.listFiles, desktopSourceFiles(io), new Map()],
  ],
  [handRolledDialogFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [integrationUrlSecretFailures, (io) => [io.read, io.exists, io.listFiles]],
  [issueStateSafetyFailures, (io) => [io.read, io.exists, desktopSourceFiles(io)]],
  [licenseCodeUnionFailures, (io) => [io.read]],
  [licenseLifecycleSafetyFailures, (io) => [io.read]],
  [mcpSchemaParityFailures, (io) => [io.read, io.listFiles]],
  [onboardingCopyFailures, (io) => [io.read]],
  [performanceGateFailures, (io) => [io.read]],
  [pricingConsistencyFailures, (io) => [io.read, io.exists]],
  [publicationRecordFailures, (io) => [io.read, io.exists, io.listFiles]],
  [privateStorageSafetyFailures, (io) => [io.read]],
  [privilegedTokenIssuerFailures, (io) => [io.read]],
  [queryLayerFailures, (io) => [io.read, desktopSourceFiles(io)]],
  [releaseArtifactSafetyFailures, (io) => [io.read, io.exists, io.listFiles]],
  [releaseWorkflowSafetyFailures, (io) => [io.read]],
  [reportScoreConsistencyFailures, (io) => [io.read]],
  [rustEventSeverityFailures, (io) => [io.read]],
  [rustLineBudgetFailures, (io) => [io.read, io.listFiles]],
  [rustRatchetFailures, (io) => [io.read, io.exists, io.listFiles]],
  [rustSeverityConsistencyFailures, (io) => [io.read, io.listFiles]],
  [rustUnwrapBudgetFailures, (io) => [io.read, io.exists, io.listFiles]],
  [rustlsCryptoProviderFailures, (io) => [io.read, io.listFiles]],
  [scanPersistOffThreadFailures, (io) => [io.read, io.exists]],
  [scanSchedulerPersistPathFailures, (io) => [io.read]],
  [scoreArtifactLabelingFailures, (io) => [io.read]],
  [severityPolicyChokepointFailures, (io) => [io.read, io.listFiles]],
  [supplyChainSafetyFailures, (io) => [io.read, { root: io.root }]],
  [supportEmailLiteralFailures, (io) => [io.read, io.exists, io.listFiles]],
  [tauriCspSafetyFailures, (io) => [io.read, io.exists]],
  [telemetryConsentFailures, (io) => [io.read, io.exists]],
  [telemetryDisclosureFailures, (io) => [io.read, io.exists]],
  [telemetrySafetyFailures, (io) => [io.read, io.exists, io.listFiles]],
  [versionSyncFailures, (io) => [io.read]],
]);

export function runRule(rule, io) {
  const shape = RULE_ARGUMENTS.get(rule);
  if (!shape) throw new Error(`no argument shape registered for ${rule.name}`);
  return rule(...shape(io));
}

const cleanFailuresByRule = new Map();
function cleanFailures(rule) {
  if (!cleanFailuresByRule.has(rule)) {
    cleanFailuresByRule.set(rule, runRule(rule, overlayIo(ROOT)).join("\n"));
  }
  return cleanFailuresByRule.get(rule);
}

export function expectGuardrailFailure(rule, mutator, expectedMessage) {
  expect(cleanFailures(rule)).not.toContain(expectedMessage);

  const overlay = new Map();
  mutator(overlay);
  expect(overlay.size).toBeGreaterThan(0);
  expect(runRule(rule, overlayIo(ROOT, overlay)).join("\n")).toContain(expectedMessage);
}

export function guardrailFailuresFor(rule, mutator) {
  const overlay = new Map();
  mutator(overlay);
  return runRule(rule, overlayIo(ROOT, overlay)).join("\n");
}

export const rules = Object.freeze({
  OVERLAY_DELETED,
  agentGuidanceFailures,
  ambientClockFailures,
  appShellNavFailures,
  asyncCommandDbBlockingFailures,
  brokerOnlyRegistrationFailures,
  ciCostSafetyFailures,
  cliSurfaceFailures,
  codeOwnerSafetyFailures,
  codeScanInventoryFailures,
  codeScanSecurityFailures,
  commandWrapperFailures,
  commentQualityFailures,
  confirmDeadlineFailures,
  deployWorkflowHardeningFailures,
  desktopCategoryLabelFailures,
  desktopFrontendDisplayFailures,
  desktopFrontendJsonSafetyFailures,
  desktopFrontendStateFailures,
  desktopIssueStatusFailures,
  desktopLicensingSafetyFailures,
  desktopOAuthSafetyFailures,
  desktopProjectCommandSafetyFailures,
  desktopScanLabelFailures,
  desktopScannerBodySafetyFailures,
  desktopScoreConsistencyFailures,
  desktopSeverityConsistencyFailures,
  desktopSharedTypeFailures,
  desktopStyleConsistencyFailures,
  desktopUpdateCommandFailures,
  desktopUrlIdentityFailures,
  displayImplLogReentrancyFailures,
  documentationSafetyFailures,
  emDashFailures,
  emptyTestBodyFailures,
  engineVocabFailures,
  eventFabricFailures,
  fixGuideCspGuidanceFailures,
  frontendMaintainabilityFailures,
  guardrailScriptLineBudgets,
  handRolledDialogFailures,
  integrationUrlSecretFailures,
  issueStateSafetyFailures,
  licenseActivationErrorFailures,
  licenseCodeUnionFailures,
  licenseLifecycleSafetyFailures,
  licenseSurfaceFailures,
  licenseValidationBranchFailures,
  mcpSchemaParityFailures,
  onboardingCopyFailures,
  orderedBefore,
  overlayIo,
  parseSnapshotTables,
  performanceGateFailures,
  pricingConsistencyFailures,
  privateStorageSafetyFailures,
  privilegedTokenIssuerFailures,
  publicationRecordFailures,
  queryLayerFailures,
  releaseArtifactSafetyFailures,
  releaseWorkflowSafetyFailures,
  repoGuardrailFailures,
  reportScoreConsistencyFailures,
  rustEventSeverityFailures,
  rustLineBudgetFailures,
  rustRatchetFailures,
  rustSeverityConsistencyFailures,
  rustUnwrapBudgetFailures,
  rustlsCryptoProviderFailures,
  scanPersistOffThreadFailures,
  scanSchedulerPersistPathFailures,
  scoreArtifactLabelingFailures,
  severityPolicyChokepointFailures,
  stripComments,
  stripNonCode,
  supplyChainSafetyFailures,
  supportEmailLiteralFailures,
  tailwindRemovalFailures,
  tauriCspSafetyFailures,
  telemetryConsentFailures,
  telemetryDisclosureFailures,
  telemetrySafetyFailures,
  ungrantedIpcCommandFailures,
  unifiedScanArchitectureFailures,
  updaterTrustFailures,
  versionSyncFailures,
  workflowSafetyFailures,
});
