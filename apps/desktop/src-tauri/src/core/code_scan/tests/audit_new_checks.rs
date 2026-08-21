use super::*;

#[test]
fn detects_reflected_origin_with_credentials() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/data/route.ts",
        r#"
                export async function GET(request: Request) {
                  const headers = new Headers();
                  headers.set("Access-Control-Allow-Origin", request.headers.get("origin"));
                  headers.set("Access-Control-Allow-Credentials", "true");
                  return new Response("ok", { headers });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("cors-origin-reflection:"))
        .expect("expected cors-origin-reflection issue");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("SameSite"));
    assert!(issue.description.contains("does not prove"));
}

#[test]
fn detects_cors_middleware_origin_true_with_credentials() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "server.js",
        r#"
                const express = require("express");
                const cors = require("cors");
                const app = express();
                app.use(cors({ origin: true, credentials: true }));
                app.listen(3000);
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("cors-origin-reflection:")));
}

#[test]
fn skips_reflection_for_allowlisted_origin_variable() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/data/route.ts",
        r#"
                const ALLOWED_ORIGINS = ["https://app.example.com"];

                export async function GET(request: Request) {
                  const origin = request.headers.get("origin") ?? "";
                  const headers = new Headers();
                  if (ALLOWED_ORIGINS.includes(origin)) {
                    headers.set("Access-Control-Allow-Origin", origin);
                  }
                  headers.set("Access-Control-Allow-Credentials", "true");
                  return new Response("ok", { headers });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("cors-origin-reflection:")));
}

#[test]
fn detects_disabled_tls_verification_across_runtimes() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "lib/insecure-agent.ts",
        r#"
                import https from "https";
                export const agent = new https.Agent({ rejectUnauthorized: false });
            "#,
    );
    write_file(
        temp.path(),
        "scripts/sync.py",
        r#"
import requests

def fetch(url):
    return requests.get(url, verify=False)
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let flagged: Vec<&str> = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("tls-verification-disabled:"))
        .map(|issue| issue.relative_path.as_str())
        .collect();
    assert!(flagged.contains(&"lib/insecure-agent.ts"));
    assert!(flagged.contains(&"scripts/sync.py"));
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("tls-verification-disabled:"))
        .unwrap();
    assert_eq!(issue.severity, Severity::High);
    assert!(issue
        .description
        .contains("matching outbound client or call"));
    assert!(issue.description.contains("network-path attacker could"));
    assert!(!issue.description.contains("Every connection"));
}

#[test]
fn skips_tls_finding_for_unrelated_verify_kwarg() {
    // `verify=False` on a non-HTTP call (a JWT decode here) is a different
    // problem, not disabled TLS; the requests/httpx call scoping keeps it out.
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/auth.py",
        r#"
import jwt

def read_claims(token):
    return jwt.decode(token, verify=False)
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("tls-verification-disabled:")));
}

#[test]
fn detects_next_config_ignoring_build_errors_as_medium_review() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "next.config.js",
        r#"
                module.exports = {
                  typescript: { ignoreBuildErrors: true },
                  eslint: { ignoreDuringBuilds: true },
                };
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("nextconfig-errors-ignored:"))
        .expect("expected nextconfig-errors-ignored issue");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not prove"));
}

#[test]
fn next_config_ignoring_only_eslint_is_low_review() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "next.config.mjs",
        r#"
                export default {
                  eslint: { ignoreDuringBuilds: true },
                };
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("nextconfig-errors-ignored:"))
        .expect("expected nextconfig-errors-ignored issue");
    assert_eq!(issue.severity, Severity::Low);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("Next.js 16"));
}

#[test]
fn next_config_ignore_flags_in_comments_do_not_fire() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "next.config.js",
        r#"
                // Never use typescript: { ignoreBuildErrors: true }
                /* Legacy example: eslint: { ignoreDuringBuilds: true } */
                module.exports = { reactStrictMode: true };
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .all(|issue| !issue.id.starts_with("nextconfig-errors-ignored:")));
}

#[test]
fn detects_django_debug_and_wildcard_hosts_in_settings() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "mysite/settings.py",
        r#"
DEBUG = True
ALLOWED_HOSTS = ["*"]
DATABASES = {}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("framework-debug-enabled:"))
        .expect("expected framework-debug-enabled issue");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    let evidence = issue.evidence.as_deref().unwrap_or("");
    assert!(evidence.contains("DEBUG = True"));
    assert!(evidence.contains("ALLOWED_HOSTS"));
    assert!(issue
        .description
        .contains("when Django renders its technical 500 response"));
    assert!(!issue.description.contains("every error page"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("Parse and validate"));
}

#[test]
fn wildcard_hosts_without_debug_is_medium() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "config/settings/production.py",
        r#"
DEBUG = False
ALLOWED_HOSTS = ['*']
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("framework-debug-enabled:"))
        .expect("expected framework-debug-enabled issue");
    assert_eq!(issue.severity, Severity::Medium);
    assert!(issue.description.contains("does not by itself prove"));
}

#[test]
fn skips_debug_in_dev_settings_module_and_wp_and_env() {
    let temp = TempDir::new().unwrap();
    // A dev settings module legitimately runs with debug on.
    write_file(temp.path(), "config/settings/dev.py", "DEBUG = True\n");
    // A dev env file too.
    write_file(
        temp.path(),
        ".env.development",
        "APP_DEBUG=true\nAPP_NAME=demo\n",
    );
    // wp-config with debug off.
    write_file(
        temp.path(),
        "wp-config.php",
        "<?php\ndefine('WP_DEBUG', false);\n",
    );

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("framework-debug-enabled:")));
}

#[test]
fn detects_wp_debug_and_prod_env_app_debug() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-config.php",
        "<?php\ndefine( 'WP_DEBUG', true );\n",
    );
    write_file(
        temp.path(),
        ".env.production",
        "APP_NAME=demo\nAPP_DEBUG=true\n",
    );

    let report = audit_project_with_local_databases(temp.path()).unwrap();
    let flagged: Vec<&str> = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("framework-debug-enabled:"))
        .map(|issue| issue.relative_path.as_str())
        .collect();
    assert!(flagged.contains(&"wp-config.php"));
    assert!(flagged.contains(&".env.production"));
    let env_issue = report
        .issues
        .iter()
        .find(|issue| {
            issue.id.starts_with("framework-debug-enabled:")
                && issue.relative_path == ".env.production"
        })
        .expect("expected production env debug issue");
    assert_eq!(
        env_issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(env_issue
        .confidence_reason
        .as_deref()
        .unwrap_or_default()
        .contains("runtime"));
}

#[test]
fn detects_attacker_controllable_expression_in_run_block() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/greet.yml",
        r#"
name: greet
on: [issues]
jobs:
  greet:
    runs-on: ubuntu-latest
    steps:
      - run: |
          echo "New issue: ${{ github.event.issue.title }}"
      - run: echo "branch ${{ github.head_ref }}"
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("workflow-script-injection:"))
        .expect("expected workflow-script-injection issue");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("trigger and actor"));
    assert!(!issue
        .why_now
        .as_deref()
        .unwrap_or_default()
        .contains("most exploited"));
    let evidence = issue.evidence.as_deref().unwrap_or("");
    assert!(evidence.contains("github.event.issue.title"));
    assert!(evidence.contains("github.head_ref"));
}

#[test]
fn skips_injection_finding_for_env_indirection_and_safe_expressions() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/greet.yml",
        r#"
name: greet
on: [issues]
jobs:
  greet:
    runs-on: ubuntu-latest
    steps:
      - env:
          TITLE: ${{ github.event.issue.title }}
        run: |
          echo "New issue: $TITLE"
          echo "sha ${{ github.sha }} run ${{ github.run_id }}"
          echo "token ${{ secrets.DEPLOY_TOKEN }}"
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("workflow-script-injection:")));
}

#[test]
fn detects_literal_npmrc_auth_token_without_echoing_it() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".npmrc",
        "registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=npm_abcdefghijklmnopqrstuvwxyz0123456789\n", // gitleaks:allow
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("npmrc-committed-token:"))
        .expect("expected npmrc-committed-token issue");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(!issue.title.starts_with("Committed"));
    assert!(issue.description.contains("does not establish"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("If the credential was shared"));
    assert_eq!(issue.line, Some(2));
    // The credential value must never be echoed into evidence or excerpt.
    assert!(issue.source_excerpt.is_none());
    assert!(!issue
        .evidence
        .as_deref()
        .unwrap_or("")
        .contains("npm_abcdef"));
}

#[test]
fn skips_npmrc_env_substitution_token() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".npmrc",
        "//registry.npmjs.org/:_authToken=${NPM_TOKEN}\n# _authToken=commented-out\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("npmrc-committed-token:")));
}

#[test]
fn detects_latest_and_tagless_base_images_only() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "Dockerfile",
        r#"
ARG BASE=node:22-bookworm
FROM node:22-bookworm AS builder
FROM ubuntu:latest AS runtime
FROM builder
FROM $BASE
FROM scratch
FROM redis
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("dockerfile-unpinned-base:"))
        .expect("expected dockerfile-unpinned-base issue");
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or("");
    // Only the moving targets fire::latest and the tagless redis.
    assert!(evidence.contains("`ubuntu:latest`"));
    assert!(evidence.contains("`redis`"));
    // Version tags, stage refs, build args, scratch, and digests do not.
    assert!(!evidence.contains("node:22-bookworm"));
    assert!(!evidence.contains("`builder`"));
    assert!(!evidence.contains("$BASE"));
    assert!(!evidence.contains("scratch"));
}

#[test]
fn skips_dockerfile_finding_when_bases_are_pinned() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "docker/api.dockerfile",
        "FROM node:22-bookworm-slim\nFROM postgres@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("dockerfile-unpinned-base:")));
}

#[test]
fn detects_pipe_to_shell_across_build_surfaces() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "Dockerfile",
        "FROM node:22-bookworm\nRUN curl -fsSL https://get.example.dev/install.sh | sh\n",
    );
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "app",
  "scripts": {
    "setup": "wget -qO- https://tool.example.dev/setup.sh | sudo bash"
  }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let flagged: Vec<&str> = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("remote-pipe-to-shell:"))
        .map(|issue| issue.relative_path.as_str())
        .collect();
    assert!(flagged.contains(&"Dockerfile"));
    assert!(flagged.contains(&"package.json"));
}

#[test]
fn skips_pipe_to_shell_for_verified_download() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "Dockerfile",
        "FROM node:22-bookworm\nRUN curl -fsSL https://get.example.dev/install.sh -o install.sh \\\n && echo \"abc123  install.sh\" | sha256sum -c - \\\n && sh install.sh\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("remote-pipe-to-shell:")));
}

#[test]
fn pipe_to_shell_ignores_comments_and_non_script_manifest_text() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "Dockerfile",
        "FROM node:22-bookworm\n# Do not use: curl https://example.test/install | sh\nRUN echo safe\n",
    );
    write_file(
        temp.path(),
        ".github/workflows/quality.yml",
        "jobs:\n  test:\n    steps:\n      # Never run curl https://example.test/install | bash\n      - run: echo safe\n",
    );
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "app",
  "description": "Migration note: replace curl https://example.test/install | sh",
  "scripts": { "test": "echo safe" }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .all(|issue| !issue.id.starts_with("remote-pipe-to-shell:")));
}

#[test]
fn skips_nextconfig_finding_for_clean_config_and_other_files() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "next.config.js",
        r#"
                module.exports = { reactStrictMode: true };
            "#,
    );
    // The flag text in a NON-config file (docs helper, lint rule) never fires.
    write_file(
        temp.path(),
        "lib/config-docs.ts",
        r#"
                export const warning = "never set ignoreBuildErrors: true in next.config";
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("nextconfig-errors-ignored:")));
}

#[test]
fn detects_request_input_in_exec_command_and_suppresses_shell_injection() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/convert/route.ts",
        r#"
                import { exec } from "node:child_process";

                export async function POST(req: Request) {
                  const body = await req.json();
                  exec(`convert ${req.query.file} out.png`, (err) => {
                    if (err) console.error(err);
                  });
                  return new Response("ok");
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("js-command-injection:"))
        .expect("expected js-command-injection issue");
    assert_eq!(issue.severity, Severity::Critical);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.contains("Request accessor"));
    assert!(issue.description.contains("Static analysis matched"));
    assert!(issue.description.contains("does not establish"));
    let fix = issue.likely_fix.as_deref().unwrap_or_default();
    assert!(fix.contains("fixed executable") && fix.contains("argument array"));
    assert!(fix.contains("leading-option"));
    let verify = issue.verify_hint.as_deref().unwrap_or_default();
    assert!(verify.contains("mock") || verify.contains("test harness"));
    assert!(!verify.contains("marker command"));
    // The precise check owns this file: the fuzzy shell-injection must stand down.
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("shell-injection:")),
        "shell-injection double-fired alongside js-command-injection: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn detects_request_command_as_spawn_first_argument() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "server.js",
        r#"
                const { spawn } = require("child_process");
                app.post("/run", (req, res) => {
                  spawn(req.body.cmd, ["--flag"]);
                  res.end();
                });
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("js-command-injection:")));
}

#[test]
fn skips_command_injection_for_safe_and_unrelated_exec_forms() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/api/safe/route.ts",
        r#"
                import { execFile } from "node:child_process";

                const NAME_RE = /^[a-z]+$/;

                export async function POST(req: Request) {
                  const body = await req.json();
                  // Constant binary, request value only as a bound argument.
                  execFile("convert", [req.body.file, "out.png"]);
                  // RegExp.prototype.exec is not a shell call.
                  NAME_RE.exec(req.body.name);
                  return new Response("ok");
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("js-command-injection:")),
        "safe execFile arg-array and RegExp.exec must stay quiet: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );

    // Without child_process in scope, a bare exec( is not the Node sink.
    let no_import = TempDir::new().unwrap();
    write_file(
        no_import.path(),
        "app/api/other/route.ts",
        r#"
                export async function POST(req: Request) {
                  const body = await req.json();
                  const result = db.exec(`SELECT ${req.query.id}`);
                  return Response.json(result);
                }
            "#,
    );
    let report = audit_project(no_import.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("js-command-injection:")));
}

#[test]
fn detects_lockfile_registry_entry_without_strong_integrity() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "package-lock.json",
        r#"{
            "name": "demo-app",
            "lockfileVersion": 3,
            "packages": {
                "": { "name": "demo-app" },
                "node_modules/strong-pkg": {
                    "version": "1.0.0",
                    "resolved": "https://registry.npmjs.org/strong-pkg/-/strong-pkg-1.0.0.tgz",
                    "integrity": "sha512-aaaaaaaaaaaaaaaaaaaaaa=="
                },
                "node_modules/legacy-pkg": {
                    "version": "1.0.0",
                    "resolved": "https://registry.npmjs.org/legacy-pkg/-/legacy-pkg-1.0.0.tgz",
                    "integrity": "sha1-bbbbbbbbbbbbbbbbbb="
                },
                "node_modules/nohash-pkg": {
                    "version": "1.0.0",
                    "resolved": "https://registry.npmjs.org/nohash-pkg/-/nohash-pkg-1.0.0.tgz"
                },
                "node_modules/local-link": {
                    "resolved": "file:../local-link",
                    "link": true
                }
            }
        }"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("lockfile-integrity-weak:"))
        .expect("expected lockfile-integrity-weak issue");
    assert_eq!(issue.severity, Severity::Medium);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(evidence.contains("legacy-pkg"), "SHA-1 entry: {evidence}");
    assert!(evidence.contains("nohash-pkg"), "missing entry: {evidence}");
    // The SHA-512 entry and the file: link must not be reported.
    assert!(
        !evidence.contains("strong-pkg"),
        "strong entry leaked: {evidence}"
    );
    assert!(
        !evidence.contains("local-link"),
        "link entry leaked: {evidence}"
    );
    assert!(issue.title.contains("missing or weak"), "{}", issue.title);
    assert!(
        !issue.title.contains("no usable"),
        "SHA-1 is weak, but npm still performs a digest check: {}",
        issue.title
    );
    assert!(
        issue
            .description
            .contains("Missing entries provide no digest check")
            && issue
                .description
                .contains("SHA-1 entries still perform a digest check"),
        "missing and SHA-1 cases must not be conflated: {}",
        issue.description
    );
    let fix = issue.likely_fix.as_deref().unwrap_or_default();
    assert!(
        !fix.contains("delete package-lock.json"),
        "unsafe fix: {fix}"
    );
    assert!(
        fix.contains("review the lockfile diff"),
        "fix must preserve intent: {fix}"
    );
    assert!(
        fix.contains("project's package-manager version"),
        "fix must respect lockfile ownership: {fix}"
    );
    let verify = issue.verify_hint.as_deref().unwrap_or_default();
    assert!(
        verify.contains("disposable") && verify.contains("npm ci"),
        "verification must use a clean, bounded install: {verify}"
    );
    assert!(
        !issue
            .why_now
            .as_deref()
            .unwrap_or_default()
            .contains("the one check"),
        "integrity metadata is one control, not the only supply-chain control"
    );
}

#[test]
fn skips_lockfile_integrity_for_all_strong_hashes() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "package-lock.json",
        r#"{
            "name": "demo-app",
            "lockfileVersion": 3,
            "packages": {
                "": { "name": "demo-app" },
                "node_modules/a": {
                    "version": "1.0.0",
                    "resolved": "https://registry.npmjs.org/a/-/a-1.0.0.tgz",
                    "integrity": "sha512-aaaaaaaaaaaaaaaaaaaaaa=="
                }
            }
        }"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("lockfile-integrity-weak:")));
}
