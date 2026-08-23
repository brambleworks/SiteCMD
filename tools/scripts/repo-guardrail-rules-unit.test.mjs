import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { ROOT, read, realRead, rules } from "./guardrail-test-support.mjs";

const {
  asyncCommandDbBlockingFailures,
  brokerOnlyRegistrationFailures,
  codeOwnerSafetyFailures,
  deployWorkflowHardeningFailures,
  orderedBefore,
  parseSnapshotTables,
  telemetrySafetyFailures,
  ungrantedIpcCommandFailures,
  unifiedScanArchitectureFailures,
  workflowSafetyFailures,
} = rules;

describe("codeOwnerSafetyFailures", () => {
  const source = realRead(".github/CODEOWNERS");

  it("accepts the repository's sensitive-path ownership contract", () => {
    expect(codeOwnerSafetyFailures(() => source)).toEqual([]);
  });

  it("rejects removal of a privacy, release, or legal boundary", () => {
    const regressed = source
      .replace(/^\/apps\/desktop\/src\/components\/privacy\/.*\n/m, "")
      .replace(/^\/install\.sh.*\n/m, "")
      .replace(/^\/tools\/scripts\/.*\n/m, "");
    const failures = codeOwnerSafetyFailures(() => regressed).join("\n");
    expect(failures).toContain("/apps/desktop/src/components/privacy/");
    expect(failures).toContain("/install.sh");
    expect(failures).toContain("/tools/scripts/");
  });
});

describe("unifiedScanArchitectureFailures", () => {
  const base = {
    "apps/desktop/src-tauri/src/commands/scan/execution.rs":
      "admission_request(&plan, fingerprint, ScanAdmissionClass::GeneralScan, now)",
    "apps/desktop/src-tauri/src/commands/scan/verification.rs": [
      "required_web_verification_ids(&check_ids)",
      "if coverage.is_empty()",
      "admission_class: ScanAdmissionClass::BoundedVerification",
    ].join("\n"),
    "apps/desktop/src-tauri/src/db/issue_states.rs":
      "validate_canonical_check_id(check_id)\nself.set_issue_state(",
    "apps/desktop/src/pages/IssuesPage.tsx": "rankIssueGroups(visibleIssueGroups)",
    "apps/desktop/src/pages/issues/useInactiveIssueKeys.ts":
      "getWorkItems({ projectId, envUrl: normalizedUrl })",
    "apps/desktop/src/lib/project-issue-summary.ts": "buildIssueGroupSummary",
    "apps/desktop/src-tauri/build.rs": '"run_scan_execution"',
    "apps/desktop/src-tauri/src/lib.rs": "run_scan_execution",
    "apps/desktop/src/lib/commands/scan.ts": '"run_scan_execution"',
    "apps/desktop/src/lib/tauri-invoke.ts": "",
  };
  const run = (overrides = {}) => {
    const fixture = { ...base, ...overrides };
    const read = (file) => fixture[file] ?? "";
    const exists = (file) => Object.hasOwn(fixture, file);
    const listFiles = (dir, predicate) =>
      Object.keys(fixture).filter((file) => file.startsWith(`${dir}/`) && predicate(file));
    return unifiedScanArchitectureFailures(read, exists, listFiles);
  };

  it("accepts the canonical execution, identity, lifecycle, and Issues paths", () => {
    expect(run()).toEqual([]);
  });

  it("rejects legacy SQL, path-bearing IDs, and delimiter parsers", () => {
    expect(
      run({
        "apps/mcp-server/src/db.ts":
          "SELECT * FROM code_scans; const checkId = value; checkId.split(':'); const bad = 'code_scan.rule:src/a.ts';",
      }).join("\n"),
    ).toContain("legacy scan tables");
    expect(
      run({ "apps/desktop/src/lib/bad.ts": "const id = 'code_scan.rule:src/a.ts';" }).join("\n"),
    ).toContain("rule-level");
    expect(run({ "apps/desktop/src/lib/bad.ts": "checkId.split(':')" }).join("\n")).toContain(
      "must not parse",
    );
  });

  it("rejects trigger-derived exemption and split Active Issues inputs", () => {
    expect(
      run({
        "apps/desktop/src-tauri/src/commands/scan/execution.rs":
          "match plan.trigger { _ => ScanAdmissionClass::BoundedVerification }",
      }).join("\n"),
    ).toContain("never from the trigger label");
    expect(
      run({
        "apps/desktop/src-tauri/src/commands/scan/verification.rs":
          "match trigger { _ => ScanAdmissionClass::BoundedVerification }",
      }).join("\n"),
    ).toContain("never from the trigger label");
    expect(
      run({
        "apps/desktop/src/pages/IssuesPage.tsx":
          "rankUnified(latestResult?.issues, latestCodeResult?.issues)",
      }).join("\n"),
    ).toContain("canonical backend IssueGroup");
  });

  it("rejects retired public commands and lifecycle fan-out", () => {
    expect(run({ "apps/desktop/src-tauri/build.rs": '"scan_url"' }).join("\n")).toContain(
      "Retired split scan IPC commands",
    );
    expect(
      run({
        "apps/desktop/src-tauri/src/db/issue_states.rs":
          "validate_canonical_check_id(check_id)\nself.set_issue_state(\nLIKE 'code_scan.%:%'",
      }).join("\n"),
    ).toContain("without location fan-out");
  });
});

describe("orderedBefore", () => {
  it("holds only when both markers exist and the first precedes the second", () => {
    expect(orderedBefore("rate limit; then auth", "rate", "auth")).toBe(true);
    expect(orderedBefore("auth; then rate limit", "rate", "auth")).toBe(false);
  });

  it("returns false when either marker is absent (no -1 vacuous pass)", () => {
    expect(orderedBefore("only auth here", "rate", "auth")).toBe(false);
    expect(orderedBefore("only rate here", "rate", "auth")).toBe(false);
    expect(orderedBefore("neither present", "rate", "auth")).toBe(false);
  });
});

describe("parseSnapshotTables", () => {
  it("accepts SQLite schema text whose final ALTER-added column shares the closing line", () => {
    const tables = parseSnapshotTables(
      "CREATE TABLE scans (\n  id INTEGER PRIMARY KEY\n, issue_snapshot_version INTEGER NOT NULL CHECK (issue_snapshot_version IN (0, 1)), confidence_reason TEXT);\n",
    );

    expect([...tables.keys()]).toEqual(["scans"]);
    expect([...tables.get("scans")]).toEqual(["id", "issue_snapshot_version", "confidence_reason"]);
  });
});

describe("deployWorkflowHardeningFailures", () => {
  const WF = ".github/workflows/deploy-example.yml";
  const hardened = [
    "on:",
    "  push:",
    "    paths:",
    `      - "apps/example/**"`,
    `      - "${WF}"`,
    "  workflow_dispatch:",
    "concurrency:",
    "  group: deploy-example-${{ github.ref }}",
    "  cancel-in-progress: true",
    "jobs:",
    "  deploy:",
    "    runs-on: ubuntu-latest",
    "    timeout-minutes: 15",
    "    steps:",
    "      - run: pnpm exec wrangler deploy",
    "  notify-failure:",
    "    runs-on: ubuntu-latest",
    "    timeout-minutes: 5",
  ].join("\n");
  const run = (source) => deployWorkflowHardeningFailures(() => source, [WF]);

  it("passes the fully hardened workflow and ignores non-deploy workflows", () => {
    expect(run(hardened)).toEqual([]);
    expect(run("jobs:\n  test:\n    runs-on: ubuntu-latest\n")).toEqual([]);
  });

  it("fires when the concurrency group or cancel-in-progress is dropped", () => {
    const regressed = hardened.replace("  cancel-in-progress: true\n", "");
    expect(run(regressed).join("\n")).toContain("concurrency group");
  });

  it("fires when any job loses its timeout-minutes", () => {
    const regressed = hardened.replace("    timeout-minutes: 15\n", "");
    expect(run(regressed).join("\n")).toContain("timeout-minutes on every job");
  });

  it("fires when the workflow_dispatch recovery trigger is dropped", () => {
    const regressed = hardened.replace("  workflow_dispatch:\n", "");
    expect(run(regressed).join("\n")).toContain("workflow_dispatch");
  });

  it("fires when the workflow stops watching its own file in paths", () => {
    const regressed = hardened.replace(`      - "${WF}"\n`, "");
    expect(run(regressed).join("\n")).toContain("watch its own workflow file");
  });
});

describe("workflowSafetyFailures: compiling workflows provide the mcp resource path", () => {
  const RUST_WF = ".github/workflows/rust-example.yml";
  const QUALITY_WF = ".github/workflows/frontend-quality.yml";
  const MSRV_WF = ".github/workflows/rust-msrv.yml";
  const DEPENDENCY_WF = ".github/workflows/dependency-audit.yml";
  const REPO_GUARD_WF = ".github/workflows/repository-guardrails.yml";
  const CODEQL_WF = ".github/workflows/codeql.yml";
  const qualitySource =
    'paths: ["apps/desktop/src/**", "apps/mcp-server/src/**"]\n  merge_group:\n    types: [checks_requested]';
  const msrvSource = [
    "  pull_request:",
    "  merge_group:",
    "    types: [checks_requested]",
    "toolchain: 1.89.0",
    "run: mkdir -p apps/mcp-server/dist-bundle",
    "run: cargo check --locked --workspace --all-targets",
    "run: cargo check --locked --manifest-path crates/cli/Cargo.toml --all-targets",
  ].join("\n");
  const repoGuardSource = [
    "  pull_request:",
    "  merge_group:",
    "    types: [checks_requested]",
    "run: pnpm guardrails:repo",
    "run: pnpm workflows:check",
    "run: pnpm installer:check",
    "run: go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12",
  ].join("\n");
  const dependencySource = [
    "  merge_group:",
    "    types: [checks_requested]",
    '      - "THIRD_PARTY_DEPENDENCIES.json"',
    '      - "tools/scripts/check-javascript-licenses.mjs"',
    "run: pnpm run audit:licenses:js",
  ].join("\n");
  const codeqlSource = [
    "  pull_request:",
    "  merge_group:",
    "    types: [checks_requested]",
    "  push:",
    "    branches:",
    "      - main",
    "  schedule:",
    '    - cron: "23 8 * * 2"',
    "permissions:",
    "  contents: read",
    "      security-events: write",
    "          - javascript-typescript",
    "          - rust",
    "uses: github/codeql-action/init@ff2f1c621b7f889edc0d3c761ac2e6a3f8cdb0dd",
    "uses: github/codeql-action/analyze@ff2f1c621b7f889edc0d3c761ac2e6a3f8cdb0dd",
  ].join("\n");
  const run = (
    rustSource,
    frontendSource = qualitySource,
    rustMsrvSource = msrvSource,
    repositoryGuardrailSource = repoGuardSource,
    currentCodeqlSource = codeqlSource,
    currentDependencySource = dependencySource,
  ) =>
    workflowSafetyFailures(
      (file) => {
        if (file === QUALITY_WF) return frontendSource;
        if (file === MSRV_WF) return rustMsrvSource;
        if (file === REPO_GUARD_WF) return repositoryGuardrailSource;
        if (file === CODEQL_WF) return currentCodeqlSource;
        if (file === DEPENDENCY_WF) return currentDependencySource;
        return rustSource;
      },
      () => [RUST_WF, QUALITY_WF, MSRV_WF, REPO_GUARD_WF, CODEQL_WF, DEPENDENCY_WF],
    );

  it("passes a compiling workflow that provides the resource path", () => {
    const source = [
      "steps:",
      "  - run: mkdir -p apps/mcp-server/dist-bundle",
      "  - run: cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml",
    ].join("\n");
    expect(run(source)).toEqual([]);
  });

  it("fires when a workflow compiles the crate on a bare checkout", () => {
    const source =
      "steps:\n  - run: cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml";
    expect(run(source).join("\n")).toContain("dist-bundle");
  });

  it("ignores workflows that do not compile the desktop crate", () => {
    expect(run("steps:\n  - run: pnpm test")).toEqual([]);
  });

  it("rejects an MSRV workflow that stops compiling on Rust 1.89", () => {
    const floating = msrvSource.replace("toolchain: 1.89.0", "toolchain: stable");
    expect(run("steps:\n  - run: pnpm test", qualitySource, floating).join("\n")).toContain(
      "Rust MSRV",
    );
  });

  it("rejects a dependency workflow that stops enforcing JavaScript licenses", () => {
    const withoutLicensePolicy = dependencySource.replace(
      "pnpm run audit:licenses:js",
      "pnpm test",
    );
    expect(
      run(
        "steps:\n  - run: pnpm test",
        qualitySource,
        msrvSource,
        repoGuardSource,
        codeqlSource,
        withoutLicensePolicy,
      ).join("\n"),
    ).toContain("JavaScript licenses");
  });

  it("rejects a dependency workflow that skips merge-queue commits", () => {
    const withoutMergeGroup = dependencySource.replace(
      "  merge_group:\n    types: [checks_requested]\n",
      "",
    );
    expect(
      run(
        "steps:\n  - run: pnpm test",
        qualitySource,
        msrvSource,
        repoGuardSource,
        codeqlSource,
        withoutMergeGroup,
      ).join("\n"),
    ).toContain("queued dependency changes");
  });

  it("rejects repository guardrails that do not run on every pull request", () => {
    const filtered = repoGuardSource.replace(
      "  pull_request:",
      '  pull_request:\n    paths: ["tools/**"]',
    );
    expect(
      run("steps:\n  - run: pnpm test", qualitySource, msrvSource, filtered).join("\n"),
    ).toContain("every pull request");
  });

  it("rejects repository guardrails that stop validating workflow syntax", () => {
    const withoutWorkflowValidation = repoGuardSource.replace(
      "run: pnpm workflows:check",
      "run: pnpm test",
    );
    expect(
      run("steps:\n  - run: pnpm test", qualitySource, msrvSource, withoutWorkflowValidation).join(
        "\n",
      ),
    ).toContain("validate workflows and the public installer");
  });

  it("fires when a required check does not run for merge-queue commits", () => {
    const withoutMergeGroup = qualitySource.replace(
      "\n  merge_group:\n    types: [checks_requested]",
      "",
    );
    expect(run("steps:\n  - run: pnpm test", withoutMergeGroup).join("\n")).toContain(
      "must run on merge_group checks_requested",
    );
  });

  it("rejects CodeQL when a language or immutable action pin is removed", () => {
    expect(
      run(
        "steps:\n  - run: pnpm test",
        qualitySource,
        msrvSource,
        repoGuardSource,
        codeqlSource.replace("          - rust\n", "").replace(/@[0-9a-f]{40}/, "@v4"),
      ).join("\n"),
    ).toContain("CodeQL");
  });
});

describe("brokerOnlyRegistrationFailures", () => {
  const cleanSources = {
    "apps/desktop/src-tauri/permissions/data_admin.toml":
      '[[permission]]\nidentifier = "sitecmd-data-admin"\ncommands.allow = [\n    "delete_project",\n    "clear_scan_history"\n]\n',
    "apps/desktop/src-tauri/permissions/external_connectors.toml":
      '[[permission]]\ncommands.allow = [\n    "save_webhook_config"\n]\n',
    "apps/desktop/src-tauri/permissions/filesystem_access.toml":
      '[[permission]]\ncommands.allow = [\n    "open_path_in_editor"\n]\n',
    "apps/desktop/src-tauri/permissions/filesystem_export.toml":
      '[[permission]]\ncommands.allow = [\n    "write_export_file"\n]\n',
    "apps/desktop/src-tauri/permissions/project_execution.toml":
      '[[permission]]\ncommands.allow = [\n    "run_project_command"\n]\n',
    "apps/desktop/src-tauri/build.rs":
      'const APP_COMMANDS: &[&str] = &[\n    "ping",\n    "run_data_admin_command",\n];\n',
    "apps/desktop/src-tauri/src/lib.rs":
      ".invoke_handler(tauri::generate_handler![\n            commands::ping,\n            commands::run_data_admin_command,\n        ])",
  };
  const run = (overrides = {}) =>
    brokerOnlyRegistrationFailures((relativePath) => {
      const source = { ...cleanSources, ...overrides }[relativePath];
      if (source === undefined) throw new Error(`unexpected read: ${relativePath}`);
      return source;
    });

  it("passes when broker-only commands stay off the IPC surface", () => {
    expect(run()).toEqual([]);
  });

  it("fires when a broker-only command reappears in APP_COMMANDS", () => {
    const failures = run({
      "apps/desktop/src-tauri/build.rs":
        'const APP_COMMANDS: &[&str] = &[\n    "ping",\n    "delete_project",\n];\n',
    });
    expect(failures.join("\n")).toContain("APP_COMMANDS");
    expect(failures.join("\n")).toContain("delete_project");
  });

  it("fires when a broker-only command reappears in generate_handler", () => {
    const failures = run({
      "apps/desktop/src-tauri/src/lib.rs":
        ".invoke_handler(tauri::generate_handler![\n            commands::ping,\n            commands::desktop::open_path_in_editor,\n        ])",
    });
    expect(failures.join("\n")).toContain("generate_handler");
    expect(failures.join("\n")).toContain("open_path_in_editor");
  });

  it("fails closed when the broker allowlists or handler block cannot be parsed", () => {
    const emptyToml = "[[permission]]\n";
    expect(
      run({
        "apps/desktop/src-tauri/permissions/data_admin.toml": emptyToml,
        "apps/desktop/src-tauri/permissions/external_connectors.toml": emptyToml,
        "apps/desktop/src-tauri/permissions/filesystem_access.toml": emptyToml,
        "apps/desktop/src-tauri/permissions/filesystem_export.toml": emptyToml,
        "apps/desktop/src-tauri/permissions/project_execution.toml": emptyToml,
      }).join("\n"),
    ).toContain("Could not parse broker-only commands");
    expect(run({ "apps/desktop/src-tauri/src/lib.rs": "fn run() {}" }).join("\n")).toContain(
      "Could not parse apps/desktop/src-tauri/src/lib.rs",
    );
  });
});

describe("ungrantedIpcCommandFailures", () => {
  const cleanSources = {
    "apps/desktop/src-tauri/src/lib.rs":
      ".invoke_handler(tauri::generate_handler![\n            commands::ping,\n            commands::license::get_license_status,\n            commands::run_data_admin_command,\n        ])",
    "apps/desktop/src-tauri/capabilities/default.json": JSON.stringify({
      windows: ["main"],
      permissions: ["default", "core:default", "dialog:default"],
    }),
    "apps/desktop/src-tauri/capabilities/data-admin.json": JSON.stringify({
      windows: ["data-admin"],
      permissions: ["core:event:default", "allow-run-data-admin-command"],
    }),
    "apps/desktop/src-tauri/permissions/default.toml":
      '[default]\npermissions = [\n    "allow-ping",\n    "allow-get-license-status"\n]\n',
    "apps/desktop/src-tauri/build.rs":
      'const APP_COMMANDS: &[&str] = &["ping", "get_license_status", "run_data_admin_command"];',
    "apps/desktop/src/lib/tauri-invoke.ts":
      'const PRIVILEGED_BROKER_COMMANDS = new Map([["clear_scan_history", "run_data_admin_command"]] as const);',
  };
  const run = (overrides = {}, { capabilitiesReadable = true } = {}) => {
    const sources = { ...cleanSources, ...overrides };
    return ungrantedIpcCommandFailures(
      (relativePath) => {
        const source = sources[relativePath];
        if (source === undefined) throw new Error(`unexpected read: ${relativePath}`);
        return source;
      },
      (dir) => {
        if (!capabilitiesReadable) throw new Error("capabilities dir missing");
        return Object.keys(sources).filter((file) => file.startsWith(`${dir}/`));
      },
    );
  };

  it("passes when every IPC command is granted by a set or a direct allow", () => {
    expect(run()).toEqual([]);
  });

  it("fires on a registered command that no capability grants", () => {
    const failures = run({
      "apps/desktop/src-tauri/src/lib.rs":
        ".invoke_handler(tauri::generate_handler![\n            commands::ping,\n            commands::license::get_license_status,\n            commands::run_data_admin_command,\n            commands::desktop::confirm_link_license_activation,\n        ])",
    });
    expect(failures.join("\n")).toContain("confirm_link_license_activation");
    expect(failures.join("\n")).toContain("no capability grants");
  });

  it("fires when a permission set stops granting a command it used to", () => {
    const failures = run({
      "apps/desktop/src-tauri/permissions/default.toml":
        '[default]\npermissions = [\n    "allow-ping"\n]\n',
    });
    expect(failures.join("\n")).toContain("get_license_status");
  });

  it("does not mistake a plugin permission for an app command grant", () => {
    expect(run().join("\n")).not.toContain("dialog");
  });

  it("fails closed when the handler block or the capabilities cannot be read", () => {
    expect(run({ "apps/desktop/src-tauri/src/lib.rs": "fn run() {}" }).join("\n")).toContain(
      "Could not parse apps/desktop/src-tauri/src/lib.rs",
    );
    expect(run({}, { capabilitiesReadable: false }).join("\n")).toContain(
      "Could not read the desktop capabilities",
    );
  });

  it("reads the last handler entry when it has no trailing comma", () => {
    const failures = run({
      "apps/desktop/src-tauri/src/lib.rs":
        ".invoke_handler(tauri::generate_handler![\n            commands::ping,\n            commands::license::get_license_status,\n            commands::run_data_admin_command,\n            commands::desktop::confirm_link_license_activation\n        ])",
    });
    expect(failures.join("\n")).toContain("confirm_link_license_activation");
  });

  it("does not register a command that is only mentioned in a comment", () => {
    expect(
      run({
        "apps/desktop/src-tauri/src/lib.rs":
          ".invoke_handler(tauri::generate_handler![\n            commands::ping,\n            commands::license::get_license_status,\n            commands::run_data_admin_command,\n            // commands::desktop::removed_command,\n        ])",
      }),
    ).toEqual([]);
  });

  it("does not accept a grant that exists only inside a TOML comment", () => {
    const failures = run({
      "apps/desktop/src-tauri/permissions/default.toml":
        '[default]\npermissions = [\n    "allow-ping",\n    # "allow-get-license-status"\n]\n',
    });
    expect(failures.join("\n")).toContain("get_license_status");
  });

  it("keeps a grant whose line merely ends in a comment", () => {
    expect(
      run({
        "apps/desktop/src-tauri/permissions/default.toml":
          '[default]\npermissions = [\n    "allow-ping",\n    "allow-get-license-status" # deep-link path\n]\n',
      }),
    ).toEqual([]);
  });

  it("fires when a frontend command is granted only to a non-main window", () => {
    const failures = run({
      "apps/desktop/src-tauri/permissions/default.toml":
        '[default]\npermissions = [\n    "allow-ping"\n]\n',
      "apps/desktop/src-tauri/capabilities/data-admin.json": JSON.stringify({
        windows: ["data-admin"],
        permissions: ["allow-run-data-admin-command", "allow-get-license-status"],
      }),
    });
    expect(failures.join("\n")).toContain("get_license_status");
    expect(failures.join("\n")).toContain("main window");
  });

  it("exempts a broker entrypoint the routing table names", () => {
    expect(run().join("\n")).not.toContain("run_data_admin_command");
  });
});

describe("telemetrySafetyFailures build attribution", () => {
  const WRAPPER = "apps/desktop/src/lib/telemetry.ts";
  const VITE_CONFIG = "apps/desktop/vite.config.ts";
  const APP_CONTENT = "apps/desktop/src/app/AppContent.tsx";
  const APP_SHELL_HELPERS = "apps/desktop/src/app/app-shell-helpers.ts";
  const cleanSources = {
    [WRAPPER]: [
      "getTelemetryConsent",
      "setBackendTelemetryConsent",
      "usageAnalytics: false",
      "crashReports: false",
      "diagnosticSender({ args: report })",
      "appVersion: APP_VERSION",
      "buildChannel: BUILD_CHANNEL",
      "MAX_QUEUED_EVENT_AGE_MS",
      "queuedEventIsWithinAcceptanceWindow",
      ".filter((event) => queuedEventIsWithinAcceptanceWindow(event.occurredAt))",
    ].join("\n"),
    "apps/desktop/src/lib/telemetry.test.ts":
      "drops queued usage events after the server acceptance window",
    [VITE_CONFIG]:
      'const appVersion = JSON.parse(fs.readFileSync(path.resolve(__dirname, "package.json"), "utf8")).version;\ndefine: { "import.meta.env.VITE_APP_VERSION": JSON.stringify(appVersion) }',
    [APP_CONTENT]: [
      "useHasCompletedFirstScan",
      "if (bootstrapState) {",
      "  return <StartupShell />;",
      "}",
      "const showTelemetryConsentPrompt = shouldShowTelemetryConsentPrompt({",
      "  hasCompletedFirstScan,",
      "});",
      "return showTelemetryConsentPrompt ? <TelemetryConsentPrompt /> : null;",
    ].join("\n"),
    [APP_SHELL_HELPERS]: [
      "export function shouldShowTelemetryConsentPrompt({ hasCompletedFirstScan, projectCount, showScanSummary, showFirstRunWalkthrough }) {",
      "  if (!hasCompletedFirstScan || projectCount === 0) return false;",
      "  return !showScanSummary && !showFirstRunWalkthrough;",
      "}",
    ].join("\n"),
  };
  const run = (overrides = {}) => {
    const sources = { ...cleanSources, ...overrides };
    return telemetrySafetyFailures(
      (relativePath) => sources[relativePath] ?? "",
      (relativePath) => relativePath in sources,
      () => [WRAPPER],
    );
  };
  const attribution = (failures) =>
    failures.filter((failure) => failure.includes("name the shipped build"));

  it("passes when the version define and both typed build fields are present", () => {
    expect(run()).toEqual([]);
  });

  it("detects a consent prompt inside the bootstrap branch without section comments", () => {
    const appContent = cleanSources[APP_CONTENT].replace(
      "return <StartupShell />;",
      "return <TelemetryConsentPrompt />;",
    );
    expect(run({ [APP_CONTENT]: appContent }).join("\n")).toContain(
      "must not render inside the StartupShell bootstrap branch",
    );
  });

  it("fires when the vite define that supplies the version is removed", () => {
    expect(attribution(run({ [VITE_CONFIG]: "export default defineConfig({})" }))).toHaveLength(1);
  });

  it("fires when the version is hardcoded instead of read from package.json", () => {
    expect(
      attribution(
        run({ [VITE_CONFIG]: 'define: { "import.meta.env.VITE_APP_VERSION": "\\"1.4.0\\"" }' }),
      ),
    ).toHaveLength(1);
  });

  it("fires when a typed payload drops the app version or build channel", () => {
    const withoutRelease = cleanSources[WRAPPER].replace("appVersion: APP_VERSION\n", "");
    const withoutEnvironment = cleanSources[WRAPPER].replace("buildChannel: BUILD_CHANNEL\n", "");
    expect(attribution(run({ [WRAPPER]: withoutRelease }))).toHaveLength(1);
    expect(attribution(run({ [WRAPPER]: withoutEnvironment }))).toHaveLength(1);
  });

  it("is not satisfied by a hardcoded version beside a mention of package.json", () => {
    expect(
      attribution(
        run({
          [VITE_CONFIG]:
            '// version comes from package.json\ndefine: { "import.meta.env.VITE_APP_VERSION": "\\"9.9.9\\"" }',
        }),
      ),
    ).toHaveLength(1);
  });

  it("is not satisfied by a define whose value never reaches the manifest", () => {
    expect(
      attribution(
        run({
          [VITE_CONFIG]:
            'const appVersion = "9.9.9";\ndefine: { "import.meta.env.VITE_APP_VERSION": JSON.stringify(appVersion) }',
        }),
      ),
    ).toHaveLength(1);
  });
});

describe("asyncCommandDbBlockingFailures", () => {
  const FILE = "apps/desktop/src-tauri/src/commands/example.rs";
  const run = (source) =>
    asyncCommandDbBlockingFailures(
      () => source,
      (dir) => (dir.endsWith("/src/commands") ? [FILE] : []),
    );

  it("fires on an async command calling a Database method inline", () => {
    const offender = [
      "#[tauri::command]",
      "#[tracing::instrument(skip(db), fields(project_id))]",
      "pub async fn get_things(",
      "    db: State<'_, Arc<Database>>,",
      "    project_id: i64,",
      ") -> Result<Vec<Thing>, String> {",
      "    db.get_things(project_id).map_err(sanitize_error)",
      "}",
    ].join("\n");
    const failures = run(offender);
    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("get_things");
    expect(failures[0]).toContain("db.get_things(");
    expect(failures[0]).toContain("run_blocking");
  });

  it("fires on a rustfmt-wrapped multiline receiver chain", () => {
    const offender = [
      "#[tauri::command]",
      "pub async fn usage(db: State<'_, Arc<Database>>) -> Result<u32, String> {",
      "    let tier = db",
      "        .get_effective_tier();",
      "    Ok(scans_for(tier))",
      "}",
    ].join("\n");
    expect(run(offender).join("\n")).toContain("db.get_effective_tier(");
  });

  it("stays silent when the call is routed through run_blocking", () => {
    const clean = [
      "#[tauri::command]",
      "pub async fn get_things(db: State<'_, Arc<Database>>) -> Result<Vec<Thing>, String> {",
      "    let db = (*db).clone();",
      "    run_blocking(move || db.get_things()).await?.map_err(sanitize_error)",
      "}",
    ].join("\n");
    expect(run(clean)).toEqual([]);
  });

  it("stays silent for sync commands (Tauri thread-pool execution is fine)", () => {
    const sync = [
      "#[tauri::command]",
      "pub fn get_things_sync(db: State<'_, Arc<Database>>) -> Result<Vec<Thing>, String> {",
      "    db.get_things().map_err(sanitize_error)",
      "}",
    ].join("\n");
    expect(run(sync)).toEqual([]);
  });

  it("ignores non-blocking accessors and Arc plumbing on the receiver", () => {
    const accessors = [
      "#[tauri::command]",
      "pub async fn get_db_path(db: State<'_, Arc<Database>>) -> Result<String, String> {",
      "    let handle = db.inner().clone();",
      "    Ok(db.path().to_string())",
      "}",
    ].join("\n");
    expect(run(accessors)).toEqual([]);
  });

  it("passes the current tree (zero false positives)", () => {
    const listRustFiles = (dir, predicate, files = []) => {
      for (const entry of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
        const relativePath = `${dir}/${entry.name}`;
        if (entry.isDirectory()) listRustFiles(relativePath, predicate, files);
        else if (predicate(relativePath)) files.push(relativePath);
      }
      return files;
    };
    expect(asyncCommandDbBlockingFailures(read, listRustFiles)).toEqual([]);
  });
});
