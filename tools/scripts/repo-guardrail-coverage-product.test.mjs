import { describe, expect, it } from "vitest";
import {
  GUARDRAIL_TEST_TIMEOUT_MS,
  expectGuardrailFailure,
  guardrailFailuresFor,
  mustMutate,
  readFixtureFile,
  readJson,
  writeFixtureFile,
  rules,
} from "./guardrail-test-support.mjs";

const {
  ciCostSafetyFailures,
  commandWrapperFailures,
  desktopScoreConsistencyFailures,
  displayImplLogReentrancyFailures,
  emDashFailures,
  integrationUrlSecretFailures,
  licenseCodeUnionFailures,
  licenseLifecycleSafetyFailures,
  onboardingCopyFailures,
  performanceGateFailures,
  pricingConsistencyFailures,
  queryLayerFailures,
  releaseWorkflowSafetyFailures,
  repoGuardrailFailures,
  rustlsCryptoProviderFailures,
  scanPersistOffThreadFailures,
  scanSchedulerPersistPathFailures,
  scoreArtifactLabelingFailures,
  supplyChainSafetyFailures,
  supportEmailLiteralFailures,
  tauriCspSafetyFailures,
  telemetryDisclosureFailures,
  versionSyncFailures,
} = rules;

describe.concurrent(
  "repo guardrail coverage: product contracts",
  { timeout: GUARDRAIL_TEST_TIMEOUT_MS },
  () => {
    it("keeps the root test command from becoming desktop-only again", () => {
      const packageJson = readJson("package.json");

      expect(packageJson.scripts.test).toContain("pnpm run test:desktop");
      expect(packageJson.scripts.test).toContain("pnpm run test:mcp");
      expect(packageJson.scripts.test).toContain("pnpm run guardrails:repo:test");
      expect(packageJson.scripts["test:mcp"]).toContain("pnpm --filter sitecmd-mcp run test");
    });

    it("rejects disabling the pnpm minimum release age quarantine", () => {
      expectGuardrailFailure(
        supplyChainSafetyFailures,
        (fixtureRoot) => {
          const workspaceConfig = readFixtureFile(fixtureRoot, "pnpm-workspace.yaml");
          writeFixtureFile(
            fixtureRoot,
            "pnpm-workspace.yaml",
            workspaceConfig.replace("minimumReleaseAge: 1440", "minimumReleaseAge: 0"),
          );
        },
        "pnpm-workspace.yaml must keep minimumReleaseAge at 1440 minutes or higher",
      );
    });

    it("rejects a release runbook that overwrites the tag trust file", () => {
      expectGuardrailFailure(
        releaseWorkflowSafetyFailures,
        (fixtureRoot) => {
          const runbookPath = "docs/operations/releasing.md";
          const source = readFixtureFile(fixtureRoot, runbookPath);
          writeFixtureFile(
            fixtureRoot,
            runbookPath,
            source.replace(">> .github/allowed-signers", "> .github/allowed-signers"),
          );
        },
        "docs/operations/releasing.md must append to .github/allowed-signers",
      );
    });

    it("rejects an emptied tag trust file", () => {
      expectGuardrailFailure(
        releaseWorkflowSafetyFailures,
        (fixtureRoot) => {
          writeFixtureFile(fixtureRoot, ".github/allowed-signers", "# every key removed\n");
        },
        ".github/allowed-signers must list at least one signing key",
      );
    });

    it("rejects a release workflow without per-job timeout caps or run cancellation", () => {
      expectGuardrailFailure(
        ciCostSafetyFailures,
        (fixtureRoot) => {
          const workflowPath = ".github/workflows/release.yml";
          const source = readFixtureFile(fixtureRoot, workflowPath);
          writeFixtureFile(
            fixtureRoot,
            workflowPath,
            source.replace("    timeout-minutes: 75\n", ""),
          );
        },
        "release.yml must declare timeout-minutes on every job",
      );

      expectGuardrailFailure(
        ciCostSafetyFailures,
        (fixtureRoot) => {
          const workflowPath = ".github/workflows/release.yml";
          const source = readFixtureFile(fixtureRoot, workflowPath);
          writeFixtureFile(
            fixtureRoot,
            workflowPath,
            source.replace("  cancel-in-progress: true\n", ""),
          );
        },
        "release.yml must keep a concurrency group with cancel-in-progress",
      );
    });

    it("rejects re-adding a push-to-main trigger to a verify-push-mirrored quality workflow", () => {
      expectGuardrailFailure(
        ciCostSafetyFailures,
        (fixtureRoot) => {
          const workflowPath = ".github/workflows/playwright.yml";
          const source = readFixtureFile(fixtureRoot, workflowPath);
          writeFixtureFile(
            fixtureRoot,
            workflowPath,
            source.replace(
              "  workflow_dispatch:\n",
              "  push:\n    branches:\n      - main\n  workflow_dispatch:\n",
            ),
          );
        },
        "must not have a push trigger",
      );
    });

    it("rejects raw invoke() outside the command wrapper layer", () => {
      expectGuardrailFailure(
        commandWrapperFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/lib/current-score.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(fixtureRoot, file, `${source}\nvoid invoke("get_projects");\n`);
        },
        "calls invoke(...) directly",
      );

      expectGuardrailFailure(
        commandWrapperFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/hooks/useTier.tsx";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(
            fixtureRoot,
            file,
            `import { invoke } from "@/lib/tauri-invoke";\n${source}`,
          );
        },
        "imports `invoke` from the transport layer",
      );
    });

    it("rejects inline query keys and stray QueryClient construction outside the query layer", () => {
      expectGuardrailFailure(
        queryLayerFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/lib/current-score.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(fixtureRoot, file, `${source}\nconst opts = { queryKey: ["adhoc"] };\n`);
        },
        "uses an inline query-key array",
      );

      expectGuardrailFailure(
        queryLayerFailures,
        (fixtureRoot) => {
          const file = "apps/desktop/src/lib/current-score.ts";
          const source = readFixtureFile(fixtureRoot, file);
          writeFixtureFile(fixtureRoot, file, `${source}\nconst client = new QueryClient();\n`);
        },
        "There must be one client",
      );
    });

    it("rejects a SiteCMD version that is out of sync across release files", () => {
      expectGuardrailFailure(
        versionSyncFailures,
        (fixtureRoot) => {
          const lockPath = "apps/desktop/src-tauri/Cargo.lock";
          const source = readFixtureFile(fixtureRoot, lockPath);
          writeFixtureFile(
            fixtureRoot,
            lockPath,
            source.replace(/(name = "sitecmd"\nversion = ")[^"]+(")/, "$1999.0.0$2"),
          );
        },
        "SiteCMD version is out of sync across release files",
      );
    });

    it("rejects a Rust package that advertises an untested MSRV", () => {
      expectGuardrailFailure(
        versionSyncFailures,
        (fixtureRoot) => {
          const manifestPath = "apps/desktop/src-tauri/crates/cli/Cargo.toml";
          const source = readFixtureFile(fixtureRoot, manifestPath);
          writeFixtureFile(
            fixtureRoot,
            manifestPath,
            source.replace('rust-version = "1.89.0"', 'rust-version = "1.82.0"'),
          );
        },
        "Rust MSRV is out of sync",
      );
    });

    it("rejects a root Node engine that drifts from .nvmrc", () => {
      expectGuardrailFailure(
        versionSyncFailures,
        (fixtureRoot) => {
          const packagePath = "package.json";
          const source = readFixtureFile(fixtureRoot, packagePath);
          writeFixtureFile(
            fixtureRoot,
            packagePath,
            source.replace(/("node":\s*")[^"]+/, (_match, prefix) => `${prefix}0.0.0`),
          );
        },
        "The root Node engine must use .nvmrc as its minimum",
      );
    });

    it("rejects a workspace that only warns about the wrong Node runtime", () => {
      expectGuardrailFailure(
        versionSyncFailures,
        (fixtureRoot) => {
          const workspacePath = "pnpm-workspace.yaml";
          const source = readFixtureFile(fixtureRoot, workspacePath);
          writeFixtureFile(
            fixtureRoot,
            workspacePath,
            source.replace("engineStrict: true", "engineStrict: false"),
          );
        },
        "incompatible dependency engines fail installation",
      );
    });

    it("rejects a development runtime that only warns on mismatch", () => {
      expectGuardrailFailure(
        versionSyncFailures,
        (fixtureRoot) => {
          const manifestPath = "package.json";
          const source = readFixtureFile(fixtureRoot, manifestPath);
          writeFixtureFile(
            fixtureRoot,
            manifestPath,
            source.replace('"onFail": "error"', '"onFail": "warn"'),
          );
        },
        "devEngines.runtime must enforce the same Node range",
      );
    });

    it("rejects a workspace package that pins a divergent TypeScript spec", () => {
      expectGuardrailFailure(
        repoGuardrailFailures,
        (fixtureRoot) => {
          const pkgPath = "apps/mcp-server/package.json";
          const source = readFixtureFile(fixtureRoot, pkgPath);
          writeFixtureFile(
            fixtureRoot,
            pkgPath,
            source.replace(/"typescript": "~6\.0\.3"/, '"typescript": "^5.9.9"'),
          );
        },
        "Workspace packages must pin one TypeScript range spec",
      );
    });

    it("rejects a release build that stops baking telemetry and crash-report endpoints", () => {
      expectGuardrailFailure(
        telemetryDisclosureFailures,
        (fixtureRoot) => {
          const workflowPath = ".github/workflows/release.yml";
          const source = readFixtureFile(fixtureRoot, workflowPath);
          writeFixtureFile(
            fixtureRoot,
            workflowPath,
            source.replace(
              'VITE_SITECMD_TELEMETRY_ENDPOINT: "https://telemetry.sitecmd.com/v1/events"',
              'VITE_SITECMD_TELEMETRY_ENDPOINT: ""',
            ),
          );
        },
        "release.yml must bake VITE_SITECMD_TELEMETRY_ENDPOINT",
      );
    });

    it("rejects telemetry transports that bypass the Rust egress broker", () => {
      expectGuardrailFailure(
        telemetryDisclosureFailures,
        (fixtureRoot) => {
          const transportPath = "apps/desktop/src/lib/telemetry-transport.ts";
          const source = readFixtureFile(fixtureRoot, transportPath);
          writeFixtureFile(
            fixtureRoot,
            transportPath,
            mustMutate(source, "sendTelemetryRequest({ args })", "fetch(endpoint)"),
          );
        },
        "must use the typed Rust telemetry command and must not call renderer fetch",
      );
    });

    it("rejects telemetry commands that stop enforcing native consent", () => {
      expectGuardrailFailure(
        telemetryDisclosureFailures,
        (fixtureRoot) => {
          const commandPath = "apps/desktop/src-tauri/src/commands/telemetry.rs";
          const source = readFixtureFile(fixtureRoot, commandPath);
          writeFixtureFile(
            fixtureRoot,
            commandPath,
            mustMutate(source, "require_consent", "consent_check_removed"),
          );
        },
        "must own persisted consent",
      );
    });

    it("rejects telemetry schemas that become pass-through bodies", () => {
      expectGuardrailFailure(
        telemetryDisclosureFailures,
        (fixtureRoot) => {
          const schemaPath = "apps/desktop/src-tauri/src/commands/telemetry_schema.rs";
          const source = readFixtureFile(fixtureRoot, schemaPath);
          writeFixtureFile(
            fixtureRoot,
            schemaPath,
            mustMutate(source, "deny_unknown_fields", "default"),
          );
        },
        "strict body schemas",
      );
    });

    it("rejects drift between the disclosed Sentry host and the Rust allowlist", () => {
      expectGuardrailFailure(
        telemetryDisclosureFailures,
        (fixtureRoot) => {
          const commandPath = "apps/desktop/src-tauri/src/commands/telemetry.rs";
          const source = readFixtureFile(fixtureRoot, commandPath);
          writeFixtureFile(
            fixtureRoot,
            commandPath,
            mustMutate(
              source,
              'const SENTRY_INGEST_HOST: &str = "o4511662343127040.ingest.us.sentry.io";',
              'const SENTRY_INGEST_HOST: &str = "other.ingest.us.sentry.io";',
            ),
          );
        },
        "must keep exact telemetry host/path validation",
      );
    });

    it("rejects production renderer network access", () => {
      expectGuardrailFailure(
        tauriCspSafetyFailures,
        (fixtureRoot) => {
          const configPath = "apps/desktop/src-tauri/tauri.conf.json";
          const source = readFixtureFile(fixtureRoot, configPath);
          writeFixtureFile(
            fixtureRoot,
            configPath,
            mustMutate(
              source,
              "connect-src 'self' ipc: tauri:;",
              "connect-src 'self' ipc: tauri: https://telemetry.sitecmd.com;",
            ),
          );
        },
        "Production Tauri CSP must keep renderer connect-src limited to self/IPC",
      );
    });

    it("rejects re-introducing a tracing span in a Display/Debug fmt path", () => {
      expectGuardrailFailure(
        displayImplLogReentrancyFailures,
        (fixtureRoot) => {
          const configPath = "apps/desktop/src-tauri/src/licensing/config.rs";
          const source = readFixtureFile(fixtureRoot, configPath);
          writeFixtureFile(
            fixtureRoot,
            configPath,
            source.replace(
              "pub fn plan_name(&self) -> &'static str {",
              "#[tracing::instrument(skip(self))]\n    pub fn plan_name(&self) -> &'static str {",
            ),
          );
        },
        "Display/Debug fmt impls run inside the log writer lock",
      );
    });

    it("fails when release builds compile the Tauri devtools feature", () => {
      expectGuardrailFailure(
        tauriCspSafetyFailures,
        (fixtureRoot) => {
          const manifestPath = "apps/desktop/src-tauri/Cargo.toml";
          const source = readFixtureFile(fixtureRoot, manifestPath);
          writeFixtureFile(
            fixtureRoot,
            manifestPath,
            source.replace('features = ["tray-icon"]', 'features = ["tray-icon", "devtools"]'),
          );
        },
        "Production Tauri dependencies must not compile the devtools feature",
      );
    });

    it("fails when the Tauri main renderer regains a generic URL opener", () => {
      expectGuardrailFailure(
        tauriCspSafetyFailures,
        (fixtureRoot) => {
          const capabilityPath = "apps/desktop/src-tauri/capabilities/default.json";
          const capability = readFixtureFile(fixtureRoot, capabilityPath);
          writeFixtureFile(
            fixtureRoot,
            capabilityPath,
            capability.replace(
              '"dialog:default",',
              '"dialog:default",\n    "opener:allow-default-urls",',
            ),
          );
        },
        "Tauri main renderer must not have a generic URL opener",
      );
    });

    it("fails when performance baselines leave CI or push verification", () => {
      expectGuardrailFailure(
        performanceGateFailures,
        (fixtureRoot) => {
          const workflowPath = ".github/workflows/frontend-quality.yml";
          const source = readFixtureFile(fixtureRoot, workflowPath);
          writeFixtureFile(fixtureRoot, workflowPath, source.replace("pnpm perf:baseline", "true"));
        },
        "The performance baseline must run in frontend CI and local push verification",
      );
    });

    it("fails when license_lifecycle.rs reintroduces a silent paid-to-Free downgrade on validation failure", () => {
      expectGuardrailFailure(
        licenseLifecycleSafetyFailures,
        (fixtureRoot) => {
          const path =
            "apps/desktop/src-tauri/src/licensing/commands/license_lifecycle_validation.rs";
          const source = readFixtureFile(fixtureRoot, path);
          const broken = source.replace(
            "let row_answer = offline_validation_or_downgrade(&row)?;",
            "let row_answer = free_info();",
          );
          writeFixtureFile(fixtureRoot, path, broken);
        },
        "must answer the still-installed row through offline_validation_or_downgrade(&row)",
      );
    });

    it("fails when the activation error-code union drifts between Rust and TypeScript", () => {
      expectGuardrailFailure(
        licenseCodeUnionFailures,
        (fixtureRoot) => {
          const path = "apps/desktop/src/lib/license-activation-error.ts";
          const source = readFixtureFile(fixtureRoot, path);
          const broken = source.replace(/^ {2}"provider_refused",\n/m, "");
          writeFixtureFile(fixtureRoot, path, broken);
        },
        "is missing activation error codes the Rust side can emit",
      );
    });

    it("fails when validate_license stops comparing the re-read row against the validated instance", () => {
      expectGuardrailFailure(
        licenseLifecycleSafetyFailures,
        (fixtureRoot) => {
          const path =
            "apps/desktop/src-tauri/src/licensing/commands/license_lifecycle_validation.rs";
          const source = readFixtureFile(fixtureRoot, path);
          const broken = source.replace("if row.instance_id != state.instance_id {", "if false {");
          writeFixtureFile(fixtureRoot, path, broken);
        },
        "must note an instance change",
      );
    });

    it("fails when the deploy-regression detail fixtures drift between Rust and the frontend test", () => {
      expectGuardrailFailure(
        repoGuardrailFailures,
        (fixtureRoot) => {
          const testPath = "apps/desktop/src/components/alerts/alert-detail-model.test.ts";
          const source = readFixtureFile(fixtureRoot, testPath);
          const mutated = source.replace('"score_drop":8', '"score_drop":9');
          if (mutated === source) {
            throw new Error(
              'fixture mutation was a no-op: alert-detail-model.test.ts no longer contains "score_drop":8',
            );
          }
          writeFixtureFile(fixtureRoot, testPath, mutated);
        },
        "Deploy-regression detail fixtures must stay byte-identical",
      );

      expectGuardrailFailure(
        repoGuardrailFailures,
        (fixtureRoot) => {
          const rustPath = "apps/desktop/src-tauri/src/core/regression_blame_tests.rs";
          const source = readFixtureFile(fixtureRoot, rustPath);
          const mutated = source.replace("const DETAIL_FIXTURE", "const RENAMED_DETAIL_FIXTURE");
          if (mutated === source) {
            throw new Error(
              "fixture mutation was a no-op: regression_blame_tests.rs no longer declares DETAIL_FIXTURE",
            );
          }
          writeFixtureFile(fixtureRoot, rustPath, mutated);
        },
        "Could not extract the DETAIL_FIXTURE raw-string literal",
      );
    });

    it("fails when the scan scheduler bypasses the execution orchestrator", () => {
      const schedulerPath = "apps/desktop/src-tauri/src/background/scan_scheduler.rs";

      expectGuardrailFailure(
        scanSchedulerPersistPathFailures,
        (fixtureRoot) => {
          const source = readFixtureFile(fixtureRoot, schedulerPath);
          for (const call of [
            "save_scan(",
            "save_code_scan(",
            "insert_event(",
            "post_scan_persist(",
            "run_code_scan_internal(",
            "scan_url_for_execution(",
          ]) {
            if (source.includes(call)) {
              throw new Error(
                `fixture mutation would be a no-op: scan_scheduler.rs already contains ${call}`,
              );
            }
          }
          writeFixtureFile(
            fixtureRoot,
            schedulerPath,
            `${source}\n// synthetic scheduler-local persist path for the guardrail self-test\nfn scheduler_local_persist(db: &Database) {\n    let _ = db.save_scan(0, todo!());\n    let _ = db.save_code_scan(0, None, String::new(), todo!(), 0);\n    let _ = db.insert_event(todo!());\n}\n`,
          );
        },
        "not collectors or scheduler-local persistence: save_scan(, save_code_scan(, insert_event(",
      );

      expectGuardrailFailure(
        scanSchedulerPersistPathFailures,
        (fixtureRoot) => {
          const source = readFixtureFile(fixtureRoot, schedulerPath);
          const mutated = source.replace(
            "run_scan_execution_internal(",
            "renamed_execution_orchestrator(",
          );
          if (mutated === source || mutated.includes("run_scan_execution_internal(")) {
            throw new Error(
              "fixture mutation was a no-op: scan_scheduler.rs no longer calls the execution orchestrator",
            );
          }
          writeFixtureFile(fixtureRoot, schedulerPath, mutated);
        },
        "must route scheduled Web, Code, and Full actions through run_scan_execution_internal(",
      );
    });

    it("fails when a manual rustls path uses the provider-less ClientConfig::builder()", () => {
      const sslPath = "apps/desktop/src-tauri/src/checks/security/ssl.rs";

      expectGuardrailFailure(
        rustlsCryptoProviderFailures,
        (fixtureRoot) => {
          const source = readFixtureFile(fixtureRoot, sslPath);
          const mutated = source.replace(
            "crate::ssl_probe::webpki_roots_client_config()",
            "tokio_rustls::rustls::ClientConfig::builder()\n        .with_root_certificates(tokio_rustls::rustls::RootCertStore::empty())\n        .with_no_client_auth()",
          );
          if (mutated === source) {
            throw new Error(
              "fixture mutation was a no-op: ssl.rs no longer routes through webpki_roots_client_config()",
            );
          }
          writeFixtureFile(fixtureRoot, sslPath, mutated);
        },
        "must bind a crypto provider via builder_with_provider",
      );
    });

    it("fails when a maintained doc reintroduces an em-dash", () => {
      const emDash = String.fromCharCode(0x2014);
      expectGuardrailFailure(
        emDashFailures,
        (fixtureRoot) => {
          const docPath = "docs/product/get-value-in-5-minutes.md";
          const source = readFixtureFile(fixtureRoot, docPath);
          writeFixtureFile(fixtureRoot, docPath, `${source}\nReintroduced em${emDash}dash.\n`);
        },
        "uses an em-dash (U+2014)",
      );
    });

    it("fails when a tooling script reintroduces an em-dash", () => {
      const emDash = String.fromCharCode(0x2014);
      expectGuardrailFailure(
        emDashFailures,
        (fixtureRoot) => {
          const scriptPath = "tools/scripts/dev.sh";
          const source = readFixtureFile(fixtureRoot, scriptPath);
          writeFixtureFile(fixtureRoot, scriptPath, `${source}\n# reintroduced em${emDash}dash\n`);
        },
        "uses an em-dash (U+2014)",
      );
    });

    it("fails when a desktop top-level config reintroduces an em-dash", () => {
      const emDash = String.fromCharCode(0x2014);
      expectGuardrailFailure(
        emDashFailures,
        (fixtureRoot) => {
          const configPath = "apps/desktop/vite.config.ts";
          const source = readFixtureFile(fixtureRoot, configPath);
          writeFixtureFile(fixtureRoot, configPath, `${source}\n// reintroduced em${emDash}dash\n`);
        },
        "uses an em-dash (U+2014)",
      );
    });

    it("allows an em-dash on a line marked allow-em-dash (detection needles)", () => {
      const emDash = String.fromCharCode(0x2014);
      const scriptPath = "tools/scripts/dev.sh";
      const reported = guardrailFailuresFor(repoGuardrailFailures, (fixtureRoot) => {
        const source = readFixtureFile(fixtureRoot, scriptPath);
        writeFixtureFile(
          fixtureRoot,
          scriptPath,
          `${source}\n# needle "${emDash} Score:" allow-em-dash\n`,
        );
      });
      expect(reported).not.toContain(`${scriptPath}:`);
    });

    it("fails when a Rust backend source reintroduces an em-dash", () => {
      const emDash = String.fromCharCode(0x2014);
      expectGuardrailFailure(
        emDashFailures,
        (fixtureRoot) => {
          const rustPath = "apps/desktop/src-tauri/crates/engine/src/checks/security/forms.rs";
          const source = readFixtureFile(fixtureRoot, rustPath);
          writeFixtureFile(fixtureRoot, rustPath, `${source}\n// reintroduced em${emDash}dash\n`);
        },
        "uses an em-dash (U+2014)",
      );
    });

    it("fails when emitted issue copy uses a lazy (s) plural", () => {
      expectGuardrailFailure(
        emDashFailures,
        (fixtureRoot) => {
          const rustPath = "apps/desktop/src-tauri/crates/engine/src/checks/security/forms.rs";
          const source = readFixtureFile(fixtureRoot, rustPath);
          writeFixtureFile(
            fixtureRoot,
            rustPath,
            `${source}\nconst LAZY: &str = "found {} issue(s) on this page";\n`,
          );
        },
        'lazy plural "(s)" in emitted copy',
      );
    });

    it("ignores (s) outside string literals in scanned Rust sources", () => {
      const reported = guardrailFailuresFor(repoGuardrailFailures, (fixtureRoot) => {
        const rustPath = "apps/desktop/src-tauri/crates/engine/src/checks/security/forms.rs";
        const source = readFixtureFile(fixtureRoot, rustPath);
        writeFixtureFile(
          fixtureRoot,
          rustPath,
          `${source}\n// comment mentioning key(s) is fine outside a string\n`,
        );
      });
      expect(reported).not.toContain("lazy plural");
    });

    it("fails when onboarding copy names a retired navigation destination", () => {
      expectGuardrailFailure(
        onboardingCopyFailures,
        (fixtureRoot) => {
          const guidePath = "apps/desktop/src/components/layout/PageGuide.tsx";
          const source = readFixtureFile(fixtureRoot, guidePath);
          const mutated = source.replace(
            "Click into Issues, Updates, or Alerts when something needs work.",
            "Click into Issues, Launch, Security, or Updates when something needs work.",
          );
          if (mutated === source) {
            throw new Error(
              "fixture mutation was a no-op: PageGuide.tsx no longer contains the dashboard triage line",
            );
          }
          writeFixtureFile(fixtureRoot, guidePath, mutated);
        },
        "names the retired Launch page",
      );

      expectGuardrailFailure(
        onboardingCopyFailures,
        (fixtureRoot) => {
          const walkthroughPath = "apps/desktop/src/app/FirstRunWalkthrough.tsx";
          const source = readFixtureFile(fixtureRoot, walkthroughPath);
          const mutated = source.replace(
            "Look at the Issues and Updates cards first.",
            "Look at Action Items first.",
          );
          if (mutated === source) {
            throw new Error(
              "fixture mutation was a no-op: FirstRunWalkthrough.tsx no longer contains the dashboard cue",
            );
          }
          writeFixtureFile(fixtureRoot, walkthroughPath, mutated);
        },
        'points at an "Action Items" label the dashboard does not render',
      );
    });

    it("fails when a source file hardcodes the support email instead of importing SUPPORT_EMAIL", () => {
      expectGuardrailFailure(
        supportEmailLiteralFailures,
        (fixtureRoot) => {
          const filePath = "apps/desktop/src/components/layout/UpdateBanner.tsx";
          const source = readFixtureFile(fixtureRoot, filePath);
          writeFixtureFile(
            fixtureRoot,
            filePath,
            `${source}\n// reach us at support@sitecmd.com\n`,
          );
        },
        "inline support email literal",
      );
    });

    it("ignores support email literals inside test files", () => {
      const excludedPath = "apps/desktop/src/components/layout/UpdateBanner.test.tsx";
      const reported = guardrailFailuresFor(repoGuardrailFailures, (fixtureRoot) => {
        const source = readFixtureFile(fixtureRoot, excludedPath);
        writeFixtureFile(fixtureRoot, excludedPath, `${source}\n// hello@sitecmd.com\n`);
      });
      expect(reported).not.toContain(`${excludedPath}:`);
    });

    it("fails when the founder-beta commercial model publishes a speculative price", () => {
      expectGuardrailFailure(
        pricingConsistencyFailures,
        (fixtureRoot) => {
          const filePath = "apps/desktop/src/lib/commercial-model.json";
          const source = readFixtureFile(fixtureRoot, filePath);
          const mutated = source.replace('"publicPricing": "not_set"', '"publicPricing": "$29"');
          if (mutated === source) {
            throw new Error(
              "fixture mutation was a no-op: commercial-model.json no longer declares publicPricing as not_set",
            );
          }
          writeFixtureFile(fixtureRoot, filePath, mutated);
        },
        "must match the founder-beta commercial model",
      );
    });

    it("fails when the in-app founder-beta surface reintroduces checkout", () => {
      expectGuardrailFailure(
        pricingConsistencyFailures,
        (fixtureRoot) => {
          const filePath = "apps/desktop/src/components/settings/AccountSettings.tsx";
          const source = readFixtureFile(fixtureRoot, filePath);
          const mutated = `${source}\n// Get Plus for $29/mo through checkout.\n`;
          writeFixtureFile(fixtureRoot, filePath, mutated);
        },
        "must not expose a price or checkout",
      );
    });

    it("fails when an integration embeds an API key in a URL format string", () => {
      expectGuardrailFailure(
        integrationUrlSecretFailures,
        (fixtureRoot) => {
          const filePath = "apps/desktop/src-tauri/src/integrations/bing.rs";
          const source = readFixtureFile(fixtureRoot, filePath);
          writeFixtureFile(
            fixtureRoot,
            filePath,
            `${source}\nfn _leak(api_key: &str) -> String {\n    format!("https://api.example.com/stats?apikey={}", api_key)\n}\n`,
          );
        },
        "embeds a credential in a URL format string",
      );
    });

    it("fails when a scan-persist module drops the off-thread run_blocking wrapper", () => {
      expectGuardrailFailure(
        scanPersistOffThreadFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/commands/scan/web_scan.rs",
            "// regression: blocking DB persistence inlined on the async runtime\n",
          );
        },
        "scan persistence must run off the async runtime",
      );
    });

    it("fails when the Tauri attach/dev config disables CSP with null", () => {
      expectGuardrailFailure(
        tauriCspSafetyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/tauri.attach.conf.json",
            `${JSON.stringify({ app: { security: { csp: null } } }, null, 2)}\n`,
          );
        },
        'must not set "csp": null',
      );
    });

    it("keeps the retired SiteCMD-managed trial service from coming back", () => {
      expectGuardrailFailure(
        repoGuardrailFailures,
        (fixtureRoot) => {
          const invokePath = "apps/desktop/src/lib/tauri-invoke.ts";
          const source = readFixtureFile(fixtureRoot, invokePath);
          writeFixtureFile(
            fixtureRoot,
            invokePath,
            source.replace("validate_license", "start_trial"),
          );
        },
        "Retired SiteCMD-managed trial commands and UI must not be reintroduced",
      );
    });

    it("rejects user-facing split web/code score labels in desktop UI, exports, and alerts", () => {
      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenScoreLabel.tsx",
            "export function BrokenScoreLabel() { return <span>Web Score</span>; }\n",
          );
        },
        "Desktop user-facing scoring UI, exports, and alerts must use SiteCMD Score instead of split Web/Code score labels",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src-tauri/src/report/html.rs",
            'pub const LABEL: &str = "Code Score";\n',
          );
        },
        "Desktop user-facing scoring UI, exports, and alerts must use SiteCMD Score instead of split Web/Code score labels",
      );
    });

    it("rejects duplicate current-score sources in desktop UI", () => {
      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/components/dashboard/BrokenCurrentScore.tsx",
            'import { invoke } from "@/lib/tauri-invoke";\nexport async function loadScore() { return invoke("get_current_score", { projectId: 1 }); }\n',
          );
        },
        "Desktop current score must be loaded through lib/current-score.ts",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          const dashboardPath = "apps/desktop/src/components/dashboard/Dashboard.tsx";
          const source = readFixtureFile(fixtureRoot, dashboardPath);
          writeFixtureFile(
            fixtureRoot,
            dashboardPath,
            `${source}\nimport { computeSiteCmdScore } from "@/lib/sitecmd-score";\n`,
          );
        },
        "Dashboard and Issues must render the persisted current score snapshot instead of recomputing SiteCMD Score locally.",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          const dashboardPath = "apps/desktop/src/components/dashboard/Dashboard.tsx";
          const source = readFixtureFile(fixtureRoot, dashboardPath);
          writeFixtureFile(
            fixtureRoot,
            dashboardPath,
            source.replace(
              "issueCount: siteScoreIssueCount",
              "issueCount: currentScore.criticalCount + currentScore.highCount + currentScore.mediumCount + currentScore.lowCount",
            ),
          );
        },
        "Desktop visible SiteCMD Score surfaces must share the current-score loader and avoid cached/recomputed score snapshots.",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          const summaryPath = "apps/desktop/src/components/scan/scan-summary-model.ts";
          const source = readFixtureFile(fixtureRoot, summaryPath);
          writeFixtureFile(
            fixtureRoot,
            summaryPath,
            `import { computeSiteCmdScore } from "@/lib/sitecmd-score";\n${source.replace("return null;", "return computeSiteCmdScore({ webIssues: [], codeIssues: [] }).sitecmdScore;")}`,
          );
        },
        "Desktop visible SiteCMD Score surfaces must share the current-score loader and avoid cached/recomputed score snapshots.",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          const sitesPath = "apps/desktop/src/components/sites/SitesOverview.tsx";
          const source = readFixtureFile(fixtureRoot, sitesPath);
          writeFixtureFile(
            fixtureRoot,
            sitesPath,
            source.replace(
              "return project.siteScore;",
              "return Math.min(project.latestScore ?? 100, project.codeScore ?? 100);",
            ),
          );
        },
        "Sites overview must display the backend current SiteCMD Score, not min/latest Web Scan or Code Scan artifact scores.",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          const sitesPath = "apps/desktop/src/components/sites/SitesOverview.tsx";
          const source = readFixtureFile(fixtureRoot, sitesPath);
          writeFixtureFile(
            fixtureRoot,
            sitesPath,
            source.replace(
              "p.siteIssueCount",
              "getProjectIssueTotalFromWorkSummary(p.workSummary)",
            ),
          );
        },
        "Sites overview must display the backend current SiteCMD Score, not min/latest Web Scan or Code Scan artifact scores.",
      );

      expectGuardrailFailure(
        scoreArtifactLabelingFailures,
        (fixtureRoot) => {
          const modelPath = "apps/desktop/src/components/scan/code-scan-result-model.ts";
          const source = readFixtureFile(fixtureRoot, modelPath);
          writeFixtureFile(
            fixtureRoot,
            modelPath,
            source.replace('scoreLabel: "Diagnostic Score"', 'scoreLabel: "SiteCMD Score"'),
          );
        },
        "Raw scan artifact scores must stay out of primary UI chrome and be labelled as diagnostics in scan artifact surfaces.",
      );

      expectGuardrailFailure(
        scoreArtifactLabelingFailures,
        (fixtureRoot) => {
          const topBarPath = "apps/desktop/src/components/layout/TopBar.tsx";
          const source = readFixtureFile(fixtureRoot, topBarPath);
          writeFixtureFile(
            fixtureRoot,
            topBarPath,
            `${source}\nexport const brokenRawScore = (env) => env.latest_score;\n`,
          );
        },
        "Raw scan artifact scores must stay out of primary UI chrome and be labelled as diagnostics in scan artifact surfaces.",
      );

      expectGuardrailFailure(
        scoreArtifactLabelingFailures,
        (fixtureRoot) => {
          const trayPath = "apps/desktop/src/app/useTraySummary.ts";
          const source = readFixtureFile(fixtureRoot, trayPath);
          writeFixtureFile(
            fixtureRoot,
            trayPath,
            source.replace(
              "const hasAttention =",
              "const scoreAttention = (primaryEnv.latest_score ?? 100) < 80;\n      const hasAttention =",
            ),
          );
        },
        "Raw scan artifact scores must stay out of primary UI chrome and be labelled as diagnostics in scan artifact surfaces.",
      );

      expectGuardrailFailure(
        scoreArtifactLabelingFailures,
        (fixtureRoot) => {
          const emptyStatePath = "apps/desktop/src/components/dashboard/DashboardEmptyState.tsx";
          const source = readFixtureFile(fixtureRoot, emptyStatePath);
          writeFixtureFile(
            fixtureRoot,
            emptyStatePath,
            `${source}\nexport const brokenScoreChrome = (latestCodeScanSummary: { overallScore: number }) =>\n  \`\${latestCodeScanSummary.overallScore}%\`;\n`,
          );
        },
        "Raw scan artifact scores must stay out of primary UI chrome and be labelled as diagnostics in scan artifact surfaces.",
      );

      expectGuardrailFailure(
        scoreArtifactLabelingFailures,
        (fixtureRoot) => {
          const scanResultsPath = "apps/desktop/src/components/scan/code-scan-result-model.ts";
          const source = readFixtureFile(fixtureRoot, scanResultsPath);
          writeFixtureFile(
            fixtureRoot,
            scanResultsPath,
            source.replace("Diagnostic Score", "SiteCMD Score"),
          );
        },
        "Raw scan artifact scores must stay out of primary UI chrome and be labelled as diagnostics in scan artifact surfaces.",
      );

      expectGuardrailFailure(
        scoreArtifactLabelingFailures,
        (fixtureRoot) => {
          const activityPath = "apps/desktop/src/lib/dashboard/activity.ts";
          const source = readFixtureFile(fixtureRoot, activityPath);
          writeFixtureFile(
            fixtureRoot,
            activityPath,
            source.replace(
              '${pluralize(latestCodeScan.issueCount, "issue")} found',
              '${pluralize(latestCodeScan.issueCount, "issue")} · score ${latestCodeScan.overallScore}',
            ),
          );
        },
        "Raw scan artifact scores must stay out of primary UI chrome and be labelled as diagnostics in scan artifact surfaces.",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          const hookPath = "apps/desktop/src/hooks/useAppShellOrchestration.ts";
          const source = readFixtureFile(fixtureRoot, hookPath);
          writeFixtureFile(
            fixtureRoot,
            hookPath,
            source
              .replace(
                "getScoreMessage(scheduledCompletionScore.score)",
                "getScoreMessage(payload.score)",
              )
              .replace("score: scheduledCompletionScore.score", "score: payload.score")
              .replace(
                "issueCount: scheduledCompletionScore.issueCount",
                "issueCount: payload.issues",
              ),
          );
        },
        "Desktop visible SiteCMD Score surfaces must share the current-score loader and avoid cached/recomputed score snapshots.",
      );

      expectGuardrailFailure(
        scoreArtifactLabelingFailures,
        (fixtureRoot) => {
          const mcpIndexPath = "apps/mcp-server/src/server.ts";
          const source = readFixtureFile(fixtureRoot, mcpIndexPath);
          writeFixtureFile(
            fixtureRoot,
            mcpIndexPath,
            source.replace("Get the latest scan artifact score", "Get the latest SiteCMD Score"),
          );
        },
        "sitecmd-mcp must label historical scan row scores as scan artifact scores, not the current SiteCMD Score.",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          const issueRankingPath = "apps/desktop/src/lib/issue-ranking.ts";
          const source = readFixtureFile(fixtureRoot, issueRankingPath);
          writeFixtureFile(
            fixtureRoot,
            issueRankingPath,
            source.replace(
              "if (isSeverity(severity)) return severityRank(severity);",
              'return severity === "critical" ? 0 : severity === "high" ? 1 : severity === "medium" ? 2 : 3;',
            ),
          );
        },
        "Frontend severity ordering and score penalties must reuse severity.ts and sitecmd-score.ts instead of local literals",
      );

      expectGuardrailFailure(
        desktopScoreConsistencyFailures,
        (fixtureRoot) => {
          writeFixtureFile(
            fixtureRoot,
            "apps/desktop/src/lib/BrokenSeverityOrder.ts",
            'const SEVERITY_ORDER = ["critical", "high", "medium", "low"] as const;\nexport { SEVERITY_ORDER };\n',
          );
        },
        "Frontend severity ordering and score penalties must reuse severity.ts and sitecmd-score.ts instead of local literals",
      );
    });
  },
);
