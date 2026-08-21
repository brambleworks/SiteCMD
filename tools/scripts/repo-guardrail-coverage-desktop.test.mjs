import { describe, expect, it } from "vitest";
import {
  GUARDRAIL_TEST_TIMEOUT_MS,
  ROOT,
  expectGuardrailFailure,
  mustMutate,
  read,
  readFixtureFile,
  runRule,
  writeFixtureFile,
  rules,
} from "./guardrail-test-support.mjs";

const {
  ambientClockFailures,
  appShellNavFailures,
  codeScanInventoryFailures,
  desktopCategoryLabelFailures,
  desktopFrontendDisplayFailures,
  desktopFrontendStateFailures,
  desktopIssueStatusFailures,
  desktopProjectCommandSafetyFailures,
  desktopScanLabelFailures,
  desktopSeverityConsistencyFailures,
  desktopSharedTypeFailures,
  desktopStyleConsistencyFailures,
  desktopUpdateCommandFailures,
  desktopUrlIdentityFailures,
  emptyTestBodyFailures,
  engineVocabFailures,
  eventFabricFailures,
  issueStateSafetyFailures,
  overlayIo,
  performanceGateFailures,
  repoGuardrailFailures,
  reportScoreConsistencyFailures,
  rustEventSeverityFailures,
  rustSeverityConsistencyFailures,
  severityPolicyChokepointFailures,
  telemetryConsentFailures,
} = rules;

describe.concurrent(
  "repo guardrail coverage: desktop architecture",
  { timeout: GUARDRAIL_TEST_TIMEOUT_MS },
  () => {
    it("rejects inline relative time formatters in desktop UI", () => {
      expectGuardrailFailure(
        desktopFrontendStateFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenRelativeTime.ts",
            "export function age(ts) { const diffSeconds = Math.floor((Date.now() - ts) / 1000); return `${diffSeconds}s ago`; }\n",
          );
        },
        "Desktop relative time labels must use lib/format.ts formatRelativeTime instead of inline second/minute/hour math",
      );

      expectGuardrailFailure(
        desktopFrontendStateFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenTimeAgoWrapper.ts",
            'import { formatRelativeTime } from "@/lib/format";\nexport function timeAgo(value) { return formatRelativeTime(value); }\n',
          );
        },
        "Desktop relative time labels must use lib/format.ts formatRelativeTime instead of inline second/minute/hour math",
      );
    });

    it("rejects duplicate dashboard and integration card shell class strings", () => {
      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenCardShell.tsx",
            'export function BrokenCardShell() { return <div className="rounded-xl bg-card p-4 ghost-border" />; }\n',
          );
        },
        "Desktop card, panel, tile, and list-row surfaces must use the shared component style API instead of legacy aliases or duplicate utility-style class strings",
      );

      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/integrations/BrokenIntegrationCard.tsx",
            'export function BrokenIntegrationCard() { return <div className="rounded-lg ghost-border bg-card p-5" />; }\n',
          );
        },
        "Desktop card, panel, tile, and list-row surfaces must use the shared component style API instead of legacy aliases or duplicate utility-style class strings",
      );

      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenLegacySurface.tsx",
            'export function BrokenLegacySurface() { return <div className="surface-card-loose" />; }\n',
          );
        },
        "Desktop card, panel, tile, and list-row surfaces must use the shared component style API instead of legacy aliases or duplicate utility-style class strings",
      );

      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenInlineStyle.tsx",
            'export function BrokenInlineStyle() { return <div style={{ backgroundColor: "red" }} />; }\n',
          );
        },
        "Desktop DOM styling must use shared classes/components instead of inline style props",
      );

      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenRawButton.tsx",
            'export function BrokenRawButton() { return <button type="button">Click</button>; }\n',
          );
        },
        "Desktop clickable actions must use the shared Button component instead of raw <button> elements",
      );

      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenLongClass.tsx",
            'export function BrokenLongClass() { return <div className="flex items-center justify-center gap-3 rounded-xl border border-border bg-card px-5 py-4 text-sm text-foreground transition-colors hover:bg-muted" />; }\n',
          );
        },
        "Desktop className strings over 100 characters must be moved into shared component classes",
      );

      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenUtilityCluster.tsx",
            'export function BrokenUtilityCluster() { return <div className="flex items-center gap-3 rounded-lg bg-card px-3" />; }\n',
          );
        },
        "Desktop className strings with six or more utility-shaped classes must use shared component classes",
      );
    });

    it("rejects arbitrary pixel typography and spacing", () => {
      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenTextSize.tsx",
            'export function BrokenTextSize() { return <div className="text-[17px]" />; }\n',
          );
        },
        "Desktop inline arbitrary px text sizes regressed: 1 occurrences (budget 0)",
      );

      expectGuardrailFailure(
        desktopStyleConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenSpacing.tsx",
            'export function BrokenSpacing() { return <div className="gap-[7px]" />; }\n',
          );
        },
        "Desktop inline arbitrary px spacing regressed: 1 occurrences (budget 0)",
      );
    });

    it("rejects duplicate project work-summary defaults and activity checks", () => {
      expectGuardrailFailure(
        desktopFrontendStateFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenWorkSummary.ts",
            "const EMPTY_WORK_SUMMARY = { unresolvedCount: 0 };\nexport { EMPTY_WORK_SUMMARY };\n",
          );
        },
        "Desktop project work-summary defaults/activity/issue totals must use lib/project-work-summary.ts and issue summary must not accept ignored alert/work-summary inputs",
      );

      expectGuardrailFailure(
        desktopFrontendStateFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/pages/issues/BrokenWorkSummaryActivity.ts",
            "export function hasActivity(summary) { return summary.unresolvedCount > 0 || summary.maintenanceCount > 0; }\n",
          );
        },
        "Desktop project work-summary defaults/activity/issue totals must use lib/project-work-summary.ts and issue summary must not accept ignored alert/work-summary inputs",
      );

      expectGuardrailFailure(
        desktopFrontendStateFailures,
        (fixtureRoot) => {
          const summaryPath = "apps/desktop/src/lib/project-issue-summary.ts";
          const source = readFixtureFile(fixtureRoot, summaryPath);
          writeFixtureFile(
            fixtureRoot,
            summaryPath,
            source.replace("totalCount: number;", "alertCount: number;\n  totalCount: number;"),
          );
        },
        "Desktop project work-summary defaults/activity/issue totals must use lib/project-work-summary.ts and issue summary must not accept ignored alert/work-summary inputs",
      );
    });

    it("rejects duplicate scan activity merge windows", () => {
      expectGuardrailFailure(
        desktopFrontendDisplayFailures,
        (fixtureRoot) => {
          const activityPath = "apps/desktop/src/lib/dashboard/activity.ts";
          const source = readFixtureFile(fixtureRoot, activityPath);
          writeFixtureFile(
            fixtureRoot,
            activityPath,
            source
              .replace("  FULL_SCAN_MERGE_WINDOW_MS,\n", "")
              .replace(
                "const RECENT_ACTIVITY_LIMIT = 5;",
                "const FULL_SCAN_MERGE_WINDOW_MS = 5 * 60 * 1000;\nconst RECENT_ACTIVITY_LIMIT = 5;",
              ),
          );
        },
        "Desktop scan activity merge windows must use lib/activity-feed.ts FULL_SCAN_MERGE_WINDOW_MS instead of duplicate literals.",
      );
    });

    it("rejects duplicate URL display formatting helpers in desktop UI", () => {
      expectGuardrailFailure(
        desktopFrontendDisplayFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenUrlLabel.ts",
            'export function label(url) { return url.replace(/^https?:\\/\\//, "").replace(/\\/$/, ""); }\n',
          );
        },
        "Desktop URL display labels must use lib/utils.ts URL display helpers instead of local regex, hostname, or pathname parsing",
      );

      expectGuardrailFailure(
        desktopFrontendDisplayFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/sites/BrokenDomain.ts",
            "function extractDomain(url) { return new URL(url).hostname; }\nexport { extractDomain };\n",
          );
        },
        "Desktop URL display labels must use lib/utils.ts URL display helpers instead of local regex, hostname, or pathname parsing",
      );

      expectGuardrailFailure(
        desktopFrontendDisplayFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/BrokenPathOrHost.ts",
            'export function label(parsed) { return parsed.pathname && parsed.pathname !== "/" ? parsed.pathname : parsed.hostname; }\n',
          );
        },
        "Desktop URL display labels must use lib/utils.ts URL display helpers instead of local regex, hostname, or pathname parsing",
      );

      expectGuardrailFailure(
        desktopFrontendDisplayFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/scan/BrokenPagePath.tsx",
            'export function label(page) { return new URL(page).pathname || "/"; }\n',
          );
        },
        "Desktop URL display labels must use lib/utils.ts URL display helpers instead of local regex, hostname, or pathname parsing",
      );
    });

    it("rejects duplicate URL identity normalizers in desktop UI", () => {
      expectGuardrailFailure(
        desktopUrlIdentityFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenUrlKey.ts",
            'export function key(url) { return `project:${url.replace(/\\/$/, "")}`; }\n',
          );
        },
        "Desktop URL identity/cache/work-item keys must use app-targets.ts normalizeAppUrlForKey instead of local trailing-slash regex copies",
      );
    });

    it("rejects a raw safeListen effect scaffold outside the event fabric", () => {
      expectGuardrailFailure(
        eventFabricFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/BrokenEventListener.tsx",
            'import { safeListen } from "@/lib/tauri-events";\n' +
              "export function useBroken() {\n" +
              '  return safeListen("site-score-changed", () => {});\n' +
              "}\n",
          );
        },
        "calls safeListen directly",
      );
    });

    it("rejects reintroducing navigation setter shims or the APPLY escape hatch", () => {
      expectGuardrailFailure(
        appShellNavFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/app/useNavigationState.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            `${source}\nexport type SetPage = Dispatch<SetStateAction<string>>;\n`,
          );
        },
        "reintroduces a Dispatch<SetStateAction<...>> navigation setter shim",
      );

      expectGuardrailFailure(
        appShellNavFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/app/useNavigationState.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            `${source}\nconst legacy = { type: "APPLY", apply: (current) => current };\nvoid legacy;\n`,
          );
        },
        "reintroduces the generic APPLY navigation escape hatch",
      );
    });

    it("rejects re-mirroring the active selection into a ref in the shell orchestrator", () => {
      expectGuardrailFailure(
        appShellNavFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/hooks/useAppShellOrchestration.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            `${source}\nconst selectionMirrorRef = { current: null };\nselectionMirrorRef.current = activeProject?.id ?? null;\n`,
          );
        },
        "mirrors the active selection into a ref",
      );
    });

    it("rejects assertionless (empty-body) Rust tests", () => {
      expectGuardrailFailure(
        emptyTestBodyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/empty_test_fixture.rs",
            "#[cfg(test)]\nmod tests {\n    #[test]\n    fn asserts_nothing() {\n        // confirmed by code inspection\n    }\n}\n",
          );
        },
        "has an empty body (only whitespace/comments)",
      );
    });

    it("rejects local package-update command maps in desktop components", () => {
      expectGuardrailFailure(
        desktopUpdateCommandFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/BrokenUpdateCommand.tsx",
            "export function command(pkg, ver) { return `npm install ${pkg}@${ver}`; }\n",
          );
        },
        "Desktop package-update command strings must use components/dashboard/update-commands.ts buildCommand instead of local ecosystem maps",
      );
    });

    it("rejects unbounded lockfile reads in dependency parsers", () => {
      expectGuardrailFailure(
        desktopUpdateCommandFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src-tauri/src/updates/npm_lockfiles.rs";
          const source = readFixtureFile(fixtureRoot, file);
          const mutated = source.replace(
            'super::read_dependency_file(&dir.join("package-lock.json"))?',
            'std::fs::read_to_string(dir.join("package-lock.json")).ok()?',
          );
          if (mutated === source) {
            throw new Error(
              `Expected to mutate a bounded lockfile read in ${file}, but found none`,
            );
          }
          writeFixtureFile(fixtureRoot, file, mutated);
        },
        "Dependency manifests and lockfiles must use updates::read_dependency_file",
      );
    });

    it("rejects telemetry hydration that can overwrite newer consent", () => {
      expectGuardrailFailure(
        telemetryConsentFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/lib/telemetry.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            source.replace("if (consentRevision !== revisionAtStart) return;", ""),
          );
        },
        "Desktop telemetry hydration must not overwrite a newer consent choice",
      );
    });

    it("rejects per-issue correlation database lookups", () => {
      expectGuardrailFailure(
        performanceGateFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src-tauri/src/core/correlation/resolver.rs";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            source.replace("cross_project::resolve_patterns", "cross_project::resolve_pattern"),
          );
        },
        "Correlation resolution must preload cross-environment, cross-project, and integration enrichment data",
      );
    });

    it("rejects any desktop file resurrecting the deleted project_work_items write path", () => {
      expectGuardrailFailure(
        issueStateSafetyFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/components/scan/CodeIssueDossier.tsx";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            `import { setProjectWorkItemStatus } from "@/lib/legacy";\n${source}`,
          );
        },
        "references setProjectWorkItemStatus: the project_work_items lifecycle store was deleted",
      );

      expectGuardrailFailure(
        issueStateSafetyFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/lib/issues.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            `${source}\nexport const legacy = () => invoke("set_project_work_item_status");\n`,
          );
        },
        "references set_project_work_item_status: the project_work_items lifecycle store was deleted",
      );
    });

    it("rejects a baseline migration that recreates the project_work_items table", () => {
      expectGuardrailFailure(
        issueStateSafetyFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src-tauri/src/db/migrations/001_baseline.sql";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            `${source}\nCREATE TABLE IF NOT EXISTS project_work_items (id INTEGER PRIMARY KEY);\n`,
          );
        },
        "must not recreate project_work_items - the dashboard queue is a projection, not a store",
      );
    });

    it("rejects a dashboard queue projection that stops deriving from the unified issue model", () => {
      expectGuardrailFailure(
        issueStateSafetyFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src-tauri/src/commands/project_work_items.rs";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            source.replaceAll("active_fix_attempt_check_ids", "list_all_fix_attempt_check_ids"),
          );
        },
        "must build dashboard entries from get_active_issue_groups (lifecycle) + active_fix_attempt_check_ids (verify-in-flight)",
      );
    });

    it("rejects an IssueActionBar that stops persisting lifecycle through @/lib/issues", () => {
      expectGuardrailFailure(
        issueStateSafetyFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/components/issues/IssueActionBar.tsx";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            source.replace(
              'import { blockIssue, getIssueState, ignoreIssue, reopenIssue } from "@/lib/issues";',
              "",
            ),
          );
        },
        "must persist + hydrate issue lifecycle through @/lib/issues",
      );
    });

    it("rejects duplicate severity count shapes and totals in desktop UI", () => {
      expectGuardrailFailure(
        desktopSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenSeverityCounts.ts",
            "export const counts = { critical: 0, high: 0, medium: 0, low: 0 };\n",
          );
        },
        "Desktop issue severity count records/totals must use lib/severity.ts helpers instead of local critical/high/medium/low object or sum copies",
      );

      expectGuardrailFailure(
        desktopSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/BrokenSeverityTotal.ts",
            "export function total(severityCounts) { return severityCounts.critical + severityCounts.high + severityCounts.medium + severityCounts.low; }\n",
          );
        },
        "Desktop issue severity count records/totals must use lib/severity.ts helpers instead of local critical/high/medium/low object or sum copies",
      );

      expectGuardrailFailure(
        desktopSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/lib/BrokenCodeIssueCounts.ts",
            "export interface CodeIssueCounts { critical: number; high: number; medium: number; low: number; }\n",
          );
        },
        "Desktop issue severity count records/totals must use lib/severity.ts helpers instead of local critical/high/medium/low object or sum copies",
      );
    });

    it("rejects duplicate generic issue severity labels in desktop UI", () => {
      expectGuardrailFailure(
        desktopSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/BrokenSeverityLabel.ts",
            'export function label(severity) { if (severity === "critical") return "Critical"; if (severity === "high") return "High"; if (severity === "medium") return "Medium"; if (severity === "low") return "Low"; return severity; }\n',
          );
        },
        "Desktop generic issue severity labels/colors must use lib/severity.ts helpers instead of local Critical/High/Medium/Low label maps",
      );

      expectGuardrailFailure(
        desktopSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/BrokenSeverityLabelWrapper.ts",
            "export function severityLabel(severity: string): string {\n  return severity.charAt(0).toUpperCase() + severity.slice(1);\n}\n",
          );
        },
        "Desktop generic issue severity labels/colors must use lib/severity.ts helpers instead of local Critical/High/Medium/Low label maps",
      );
    });

    it("rejects duplicated shared issue type unions", () => {
      expectGuardrailFailure(
        desktopSharedTypeFailures,
        (fixtureRoot) => {
          const types = readFixtureFile(fixtureRoot, "apps/desktop/src/lib/types.ts");
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/lib/types.ts",
            types
              .replace('import type { Severity } from "./severity";\n', "")
              .replace('export type { Severity } from "./severity";\n', "")
              .replace(
                'export type { IssueConfidence } from "./issue-confidence";\n',
                'export type { IssueConfidence } from "./issue-confidence";\nexport type Severity = "critical" | "high" | "medium" | "low";\n',
              ),
          );
        },
        "Desktop Severity type must be re-exported from lib/severity.ts, not duplicated in lib/types.ts.",
      );

      expectGuardrailFailure(
        desktopSharedTypeFailures,
        (fixtureRoot) => {
          const types = readFixtureFile(fixtureRoot, "apps/desktop/src/lib/types.ts");
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/lib/types.ts",
            types
              .replace('import type { IssueConfidence } from "./issue-confidence";\n', "")
              .replace('export type { IssueConfidence } from "./issue-confidence";\n', "")
              .replace(
                'export type { Severity } from "./severity";\n',
                'export type { Severity } from "./severity";\nexport type IssueConfidence = "confirmed" | "high" | "needs_review";\n',
              ),
          );
        },
        "Desktop IssueConfidence type must live with lib/issue-confidence.ts behavior and be re-exported from lib/types.ts.",
      );
    });

    it("rejects duplicated scan labels and stale scan type unions", () => {
      expectGuardrailFailure(
        desktopScanLabelFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/scan/BrokenScanLabels.ts",
            'export const label = "Web Scan · Security";\n',
          );
        },
        "Scan subtype labels must come from apps/desktop/src/lib/scan-labels.ts",
      );

      expectGuardrailFailure(
        desktopScanLabelFailures,
        (fixtureRoot) => {
          const generated = readFixtureFile(
            fixtureRoot,
            "apps/desktop/src/generated/ipc-bindings.ts",
          );
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/generated/ipc-bindings.ts",
            generated.replace(
              'export type ScanType = "health" | "security" | "accessibility" | "polish";',
              'export type ScanType = "health" | "security" | "accessibility";',
            ),
          );
        },
        'apps/desktop/src/generated/ipc-bindings.ts ScanType must include "polish"',
      );
    });

    it("rejects duplicate category label and domain style maps in desktop UI", () => {
      expectGuardrailFailure(
        desktopCategoryLabelFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/scan/BrokenCategoryLabels.ts",
            'const CATEGORY_LABELS = { security: "Security" };\nexport { CATEGORY_LABELS };\n',
          );
        },
        "Desktop category labels/domain styles must use lib/tokens.ts or scan/code-scan-result-model.ts instead of local maps",
      );

      expectGuardrailFailure(
        desktopCategoryLabelFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/scan/BrokenDomainStyles.ts",
            'const DOMAIN_STYLES = { security: { dot: "bg-red-400" } };\nexport { DOMAIN_STYLES };\n',
          );
        },
        "Desktop category labels/domain styles must use lib/tokens.ts or scan/code-scan-result-model.ts instead of local maps",
      );

      expectGuardrailFailure(
        desktopCategoryLabelFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/scan/BrokenCategoryOrder.ts",
            'const CATEGORY_ORDER = ["security", "performance", "seo", "accessibility", "compliance", "polish"];\nexport { CATEGORY_ORDER };\n',
          );
        },
        "Desktop category labels/domain styles must use lib/tokens.ts or scan/code-scan-result-model.ts instead of local maps",
      );

      expectGuardrailFailure(
        desktopCategoryLabelFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/BrokenTypedFilterOrder.ts",
            'import type { ScanCategory } from "@/lib/types";\nconst WEB_FILTER_ORDER: ScanCategory[] = ["security", "performance", "seo", "accessibility", "compliance", "config", "polish"];\nexport { WEB_FILTER_ORDER };\n',
          );
        },
        "Desktop category labels/domain styles must use lib/tokens.ts or scan/code-scan-result-model.ts instead of local maps",
      );

      expectGuardrailFailure(
        desktopCategoryLabelFailures,
        (fixtureRoot) => {
          const tokensPath = "apps/desktop/src/lib/tokens.ts";
          const source = readFixtureFile(fixtureRoot, tokensPath);
          writeFixtureFile(
            fixtureRoot,
            tokensPath,
            source
              .replace('import { CATEGORY_META } from "./category-meta";\n', "")
              .replace("compliance: CATEGORY_META.compliance.label", 'compliance: "Compliance"'),
          );
        },
        "Desktop category labels/domain styles must use lib/tokens.ts or scan/code-scan-result-model.ts instead of local maps",
      );

      expectGuardrailFailure(
        desktopCategoryLabelFailures,
        (fixtureRoot) => {
          const actionLanguagePath = "apps/desktop/src/lib/action-language.ts";
          const source = readFixtureFile(fixtureRoot, actionLanguagePath);
          writeFixtureFile(
            fixtureRoot,
            actionLanguagePath,
            source.replace(
              "`Open ${CATEGORY_LABELS.compliance} Results`",
              '"Open Compliance Results"',
            ),
          );
        },
        "Desktop category labels/domain styles must use lib/tokens.ts or scan/code-scan-result-model.ts instead of local maps",
      );
    });

    it("rejects duplicate web-check actionable status predicates", () => {
      expectGuardrailFailure(
        desktopIssueStatusFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/scan/BrokenIssueCount.ts",
            'export function count(issues) { return issues.filter((issue) => issue.status === "fail" || issue.status === "warn").length; }\n',
          );
        },
        "Desktop web-check actionable status logic must use lib/issues.ts helpers instead of local fail/warn or not-pass predicates",
      );

      expectGuardrailFailure(
        desktopIssueStatusFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenOpenChecks.ts",
            'export function open(issues) { return issues.filter((issue) => issue.status !== "pass"); }\n',
          );
        },
        "Desktop web-check actionable status logic must use lib/issues.ts helpers instead of local fail/warn or not-pass predicates",
      );

      expectGuardrailFailure(
        desktopIssueStatusFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenCheckStatusLabel.ts",
            `export function label(status) {
  switch (status) {
    case "pass": return "Pass";
    case "fail": return "Fail";
    case "warn": return "Warn";
    case "skipped": return "Skipped";
    default: return "";
  }
}
`,
          );
        },
        "Desktop web-check status labels must use lib/issues.ts formatCheckStatus instead of local Pass/Fail/Warn/Skipped switches",
      );
    });

    it("rejects report summaries that present Web Scan scores as SiteCMD Score", () => {
      expectGuardrailFailure(
        reportScoreConsistencyFailures,
        (fixtureRoot) => {
          const sectionsPath = "apps/desktop/src/components/reports/ReportsPageSections.tsx";
          const source = readFixtureFile(fixtureRoot, sectionsPath);
          writeFixtureFile(
            fixtureRoot,
            sectionsPath,
            source
              .replace("summary.site_score", "summary.web_scan_score")
              .replace("summary.has_code_scan", "summary.code_score != null"),
          );
        },
        "Saved report history summaries must persist/display unified SiteCMD Score, not Web Scan or Code Scan scores.",
      );
    });

    it("rejects PDF report models that stop using the generated backend contract", () => {
      expectGuardrailFailure(
        reportScoreConsistencyFailures,
        (fixtureRoot) => {
          const modelPath = "apps/desktop/src/components/reports/report-pdf-model.ts";
          const source = readFixtureFile(fixtureRoot, modelPath);
          writeFixtureFile(
            fixtureRoot,
            modelPath,
            mustMutate(
              source,
              "export type ReportData = GeneratedReportData;",
              "export interface ReportData {}",
            ),
          );
        },
        "Report payloads must carry the unified SiteCMD Score from current work_items",
      );
    });

    it("rejects sequential report integration fetches", () => {
      expectGuardrailFailure(
        reportScoreConsistencyFailures,
        (fixtureRoot) => {
          const reportPath = "apps/desktop/src-tauri/src/report.rs";
          const source = readFixtureFile(fixtureRoot, reportPath);
          writeFixtureFile(
            fixtureRoot,
            reportPath,
            source.replace(
              `let (analytics, uptime) = tokio::join!(
        fetch_analytics_summary(&configs, &sections, period_days),
        fetch_uptime_summary(&configs, &sections),
    );`,
              `let analytics = fetch_analytics_summary(&configs, &sections, period_days).await;
    let uptime = fetch_uptime_summary(&configs, &sections).await;`,
            ),
          );
        },
        "Independent analytics and uptime report summaries must be fetched concurrently.",
      );
    });

    it("rejects Code Scan analyzers that recursively rebuild project inventories", () => {
      expectGuardrailFailure(
        codeScanInventoryFailures,
        (fixtureRoot) => {
          const operationsPath = "apps/desktop/src-tauri/src/core/code_scan/operations.rs";
          const source = readFixtureFile(fixtureRoot, operationsPath);
          writeFixtureFile(
            fixtureRoot,
            operationsPath,
            source.replace(
              "let project_paths = collect_project_paths(project_files);",
              "let project_paths = collect_project_paths(root);",
            ),
          );
        },
        "Code Scan analyzers must reuse one bounded project inventory instead of recursively walking the project again.",
      );
    });

    it("requires Code Scan inventory readers to reject symlink swaps", () => {
      expectGuardrailFailure(
        codeScanInventoryFailures,
        (fixtureRoot) => {
          const inventoryPath = "apps/desktop/src-tauri/src/core/code_scan/project_inventory.rs";
          const source = readFixtureFile(fixtureRoot, inventoryPath);
          writeFixtureFile(
            fixtureRoot,
            inventoryPath,
            source.replace(
              "read_project_file(file, 250_000)",
              "fs::read(&file.absolute_path).ok()",
            ),
          );
        },
        "Code Scan inventory readers must use the bounded no-follow helper and reject files replaced by symlinks.",
      );
    });

    it("requires project metadata detection to use bounded no-follow reads", () => {
      expectGuardrailFailure(
        codeScanInventoryFailures,
        (fixtureRoot) => {
          const environmentsPath = "apps/desktop/src-tauri/src/core/project/environments.rs";
          const source = readFixtureFile(fixtureRoot, environmentsPath);
          writeFixtureFile(
            fixtureRoot,
            environmentsPath,
            mustMutate(
              source,
              'read_project_text(dir, ".lando.yml")',
              'std::fs::read_to_string(dir.join(".lando.yml")).ok()',
            ),
          );
        },
        "Project detection and AI framework detection must use the shared bounded no-follow reader for repository-controlled metadata.",
      );
    });

    it("checks that elevated desktop commands stay behind the broker", () => {
      const guardrails = [
        read("tools/scripts/check-repo-guardrails.mjs"),
        read("tools/scripts/lib/guardrail-capability-security-rules.mjs"),
        read("tools/scripts/lib/guardrail-command-security-manifest-rules.mjs"),
        read("tools/scripts/lib/guardrail-desktop-licensing-rules.mjs"),
        read("tools/scripts/lib/guardrail-desktop-rules.mjs"),
        read("tools/scripts/lib/guardrail-privileged-token-rules.mjs"),
        read("tools/scripts/lib/guardrail-code-scan-security-rules.mjs"),
        read("tools/scripts/lib/guardrail-desktop-boundary-rules.mjs"),
        read("tools/scripts/lib/guardrail-frontend-maintainability-rules.mjs"),
        read("tools/scripts/lib/guardrail-cross-surface-contract-rules.mjs"),
        read("tools/scripts/lib/guardrail-frontend-display-rules.mjs"),
        read("tools/scripts/lib/guardrail-frontend-rules.mjs"),
        read("tools/scripts/lib/guardrail-rust-rules.mjs"),
        read("tools/scripts/lib/guardrail-rust-event-severity-rules.mjs"),
        read("tools/scripts/lib/guardrail-tauri-csp-rules.mjs"),
      ].join("\n");

      expect(guardrails).toContain(
        "Tauri CSP must keep self-only default/script sources and must not allow style-src-attr 'unsafe-inline'",
      );
      expect(guardrails).toContain("No Tauri capability may grant the elevated permission set");
      expect(guardrails).toContain("must NOT include the elevated permission set");
      expect(guardrails).toContain("brokeredPermissionCommandFiles");
      expect(guardrails).toContain("brokeredDirectCommands");
      expect(guardrails).toContain("privileged bridge windows");
      expect(guardrails).toContain("allow-run-external-connector-command");
      expect(guardrails).toContain("allow-run-filesystem-access-command");
      expect(guardrails).toContain("Feature-scoped privileged broker command lists");
      expect(guardrails).toContain("forbidden main-renderer plugin permissions");
      expect(guardrails).toContain("keyring secret access or install-capable updater permissions");
      expect(guardrails).toContain("Desktop webhook delivery logs must redact");
      expect(guardrails).toContain("Desktop scan logs must use log_safe_url_target");
      expect(guardrails).toContain("Desktop frontend logs must redact and truncate");
      expect(guardrails).toContain(
        "Desktop issue dossier overlays must render through document.body",
      );
      expect(guardrails).toContain('window.open(..., "_blank") must include noopener,noreferrer');
      expect(guardrails).toContain(
        "Desktop licensing and checkout secret handling must use hashed non-PII fingerprints",
      );
      expect(guardrails).toContain(
        "match arms must exactly cover every brokered elevated permission",
      );
      expect(guardrails).toContain(
        "per-family scoped issuers must stay mounted with in-handler native confirmations",
      );
      expect(guardrails).toContain(
        "Sensitive privileged token broker commands must require native confirmation",
      );
      expect(guardrails).toContain("sensitive commands must use native user intent");
      expect(guardrails).toContain(
        "desktop project commands must block installer lifecycle scripts",
      );
      expect(guardrails).toContain("cap inherited stdout/stderr pipe draining");
      expect(guardrails).toContain(
        "desktop webview-analysis and PageSpeed commands must validate URLs",
      );
      expect(guardrails).toContain("Desktop scan/code-scan event severity must use");
      expect(guardrails).toContain("Code Scan source excerpts must redact secret-like values");
      expect(guardrails).toContain("Code Scan filesystem collection must keep file-count");
      expect(guardrails).toContain("Scan URL DNS validation must reject non-localhost domains");
      expect(guardrails).toContain("HTTP DNS resolution must re-check cached and fresh answers");
    });

    it("fails when issue dossiers stop rendering through a document body portal", () => {
      expectGuardrailFailure(
        desktopFrontendStateFailures,
        (fixtureRoot) => {
          const panel = readFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/IssueDossierPanel.tsx",
          );
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/issues/IssueDossierPanel.tsx",
            panel
              .replace('import { createPortal } from "react-dom";\n', "")
              .replace("return createPortal(panel, document.body);", "return panel;"),
          );
        },
        "Desktop issue dossier overlays must render through document.body",
      );
    });

    it("fails when desktop browser fallbacks omit noreferrer", () => {
      expectGuardrailFailure(
        repoGuardrailFailures,
        (fixtureRoot) => {
          const helper = readFixtureFile(fixtureRoot, "apps/desktop/src/lib/open-url.ts");
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/lib/open-url.ts",
            helper.replace('"noopener,noreferrer"', '"noopener"'),
          );
        },
        'window.open(..., "_blank") must include noopener,noreferrer',
      );
    });

    it("fails when desktop project commands stop blocking package-manager script aliases", () => {
      expectGuardrailFailure(
        desktopProjectCommandSafetyFailures,
        (fixtureRoot) => {
          const policy = readFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/desktop_project_commands.rs",
          );
          const tests = readFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/desktop_tests.rs",
          );
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/desktop_project_commands.rs",
            policy.replace('| "rebuild"', ""),
          );
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/desktop_tests.rs",
            tests.replace(
              "validate_project_command_policy_rejects_package_manager_lifecycle_and_script_aliases",
              "validate_project_command_policy_rejects_package_manager_aliases_removed",
            ),
          );
        },
        "desktop project commands must block installer lifecycle scripts",
      );
    });

    it("fails when scan event severity thresholds are inlined again", () => {
      expectGuardrailFailure(
        rustEventSeverityFailures,
        (fixtureRoot) => {
          const execution = readFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/scan/execution.rs",
          );
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/scan/execution.rs",
            `${execution}\nfn broken_event_severity(overall_score: u32) -> EventSeverity { if overall_score < 50 { EventSeverity::Critical } else if overall_score < 80 { EventSeverity::Warning } else { EventSeverity::Info } }\n`,
          );
        },
        "Desktop scan/code-scan event severity must use EventSeverity::from_scan_score/from_issue_counts",
      );
    });

    it("fails when a check reads an ambient clock instead of evaluation_time", () => {
      expectGuardrailFailure(
        ambientClockFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/checks/security/broken_clock.rs",
            `pub fn days_left(expiry: chrono::DateTime<chrono::Utc>) -> i64 {
    (expiry - chrono::Utc::now()).num_days()
}
`,
          );
        },
        "must take their time basis from the injected evaluation_time",
      );
    });

    it("fails when the desktop stops re-exporting the engine sync check surface", () => {
      expectGuardrailFailure(
        engineVocabFailures,
        (fixtureRoot) => {
          const checksMod = readFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/checks/mod.rs",
          );
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/checks/mod.rs",
            checksMod.replace("pub use sitecmd_engine::{Check, PageContext};", ""),
          );
        },
        "must re-export the engine sync check surface",
      );
    });

    it("fails when Rust issue severity string or rank helpers are copied again", () => {
      expectGuardrailFailure(
        rustSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/BrokenSeverityCopy.rs",
            `use crate::checks::Severity;
fn severity_str(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
    }
}
`,
          );
        },
        "Rust issue severity string/rank helpers must use the engine vocab's Severity methods instead of local match copies",
      );
    });

    it("fails when a Rust string severity rank table comes back", () => {
      expectGuardrailFailure(
        rustSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/core/broken_rank_table.rs",
            `fn severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}
`,
          );
        },
        "Rust issue severity string/rank helpers must use the engine vocab's Severity methods instead of local match copies",
      );
    });

    it("fails when Rust code compares a severity field against a string literal", () => {
      expectGuardrailFailure(
        rustSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/core/broken_severity_compare.rs",
            `fn is_urgent(issue: &crate::core::regression_blame::CurrentIssue) -> bool {
    issue.severity == "critical"
}
`,
          );
        },
        "Rust issue severity is the typed checks::Severity enum; compare against Severity::* variants, not string literals",
      );
    });

    it("fails when the web severity policy is applied outside the finalize chokepoint", () => {
      expectGuardrailFailure(
        severityPolicyChokepointFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/core/rogue_assembly.rs",
            `pub fn assemble(results: &mut [crate::checks::CheckResult]) {
    crate::core::severity_policy::normalize_check_results(results);
}
`,
          );
        },
        "may only be called from apps/desktop/src-tauri/src/core/scanner/finalize.rs",
      );
    });

    it("fails when the finalize chokepoint stops applying the severity policy", () => {
      expectGuardrailFailure(
        severityPolicyChokepointFailures,
        (fixtureRoot) => {
          const finalizePath = "apps/desktop/src-tauri/src/core/scanner/finalize.rs";
          const source = readFixtureFile(fixtureRoot, finalizePath);
          writeFixtureFile(
            fixtureRoot,
            finalizePath,
            source.replace("crate::core::severity_policy::normalize_check_results(results);", ""),
          );
        },
        "must call severity_policy::normalize_check_results",
      );
    });

    it("fails when Rust scan category string helpers are copied again", () => {
      expectGuardrailFailure(
        rustSeverityConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/BrokenCategoryCopy.rs",
            `use crate::checks::ScanCategory;
fn category_str(category: &ScanCategory) -> &'static str {
    match category { ScanCategory::Security => "security", ScanCategory::Performance => "performance", ScanCategory::Seo => "seo", ScanCategory::Accessibility => "accessibility", ScanCategory::Compliance => "compliance", ScanCategory::Config => "config", ScanCategory::Polish => "polish" }
}
`,
          );
        },
        "Rust scan category string/display helpers must use the engine vocab's ScanCategory methods instead of local match copies",
      );
    });

    it("checks documentation drift rules directly in the repo guardrails", () => {
      const guardrails = [
        read("tools/scripts/check-repo-guardrails.mjs"),
        read("tools/scripts/lib/guardrail-doc-rules.mjs"),
      ].join("\n");

      expect(guardrails).toContain("stale Code Scan or Tauri capability architecture");
      expect(guardrails).toContain("Full Scan -> Dashboard guided flow");
      expect(guardrails).toContain("tested Node 22.22.1+ requirement");
      expect(guardrails).toContain("README tool table must list every registered MCP tool");
      expect(guardrails).toContain("machine-specific absolute Markdown links");
      expect(guardrails).toContain("legacy aliases");
      expect(guardrails).toContain("guidance-only until it can actually queue desktop scans");
      expect(guardrails).toContain("request_scan tool description must stay guidance-only");
      expect(guardrails).toContain("credentials fall back to SQLite");
      expect(guardrails).toContain("recovery runbook");
      expect(guardrails).toContain(
        "scan comparison must compare Web Scan and Code Scan issues against their own scan-history windows",
      );
      expect(guardrails).toContain(
        "workspace fallback issues must be structurally typed, not double-cast to DB issues",
      );
      expect(guardrails).toContain(
        "causal graph JSON must be parsed as unknown generated data before use",
      );
    });

    it("passes against the working tree before mutation tests run", () => {
      expect(runRule(repoGuardrailFailures, overlayIo(ROOT))).toEqual([]);
    });
  },
);
