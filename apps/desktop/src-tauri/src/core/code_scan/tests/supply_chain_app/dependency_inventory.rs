use super::super::*;

#[test]
fn detects_undeclared_and_suspicious_imports() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "react": "^19.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenIA from "openia";

                export async function POST() {
                  return Response.json({ ok: !!OpenIA });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let undeclared = report
        .issues
        .iter()
        .find(|issue| {
            issue
                .id
                .contains("undeclared-package:app/api/chat/route.ts:openia")
        })
        .expect("undeclared-package issue"); // allow-expect: test assertion
    let suspicious = report
        .issues
        .iter()
        .find(|issue| {
            issue
                .id
                .contains("suspicious-package:app/api/chat/route.ts:openia")
        })
        .expect("near-match package-name issue");
    assert_eq!(suspicious.severity, Severity::Medium);
    assert_eq!(
        suspicious.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(suspicious.confidence_reason.is_some());
    // Import resolution remains bounded rather than absolute, but manifest,
    // workspace, alias, and local-module filtering make this a strong lead.
    assert_eq!(undeclared.confidence, crate::checks::IssueConfidence::High);
    assert!(undeclared.confidence_reason.is_some());
    assert_eq!(undeclared.severity, Severity::Medium);
    assert!(undeclared.title.contains("not matched"));
    assert!(!undeclared.title.contains("Third-party"));
}

#[test]
fn security_regression_direct_url_dependency_evidence_masks_embedded_token() {
    let temp = TempDir::new().unwrap();
    // Deliberate fake GitHub token embedded in a git dependency URL.
    let manifest = r#"
                {
                  "name": "demo-app",
                  "dependencies": {
                    "acme-ui": "git+https://oauth2:ghp_abcdefghijklmnop1234@github.com/acme/ui.git"
                  }
                }
            "#; // gitleaks:allow
    write_file(temp.path(), "package.json", manifest);

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("direct-url-dependency:"))
        .expect("direct-url-dependency issue"); // allow-expect: test assertion
    let evidence = issue.evidence.as_deref().expect("evidence text"); // allow-expect: test assertion
    assert!(
        !evidence.contains("abcdefghijklmnop1234"),
        "token suffix leaked into evidence: {evidence}"
    );
    assert!(
        !evidence.contains("ghp_"),
        "credential hint leaked: {evidence}"
    );
    assert!(evidence.contains("Git or direct URL source"));
    assert!(!issue.title.contains("pinned"));
    if let Some(excerpt) = issue.source_excerpt.as_deref() {
        assert!(
            !excerpt.contains("abcdefghijklmnop1234"),
            "token suffix leaked into source excerpt: {excerpt}"
        );
    }
}

#[test]
fn detects_suspicious_manifest_dependency() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "demo-app",
                  "dependencies": {
                    "openia": "^1.0.0"
                  }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue
                .id
                .contains("suspicious-manifest-package:package.json:openia")
        })
        .expect("near-match manifest dependency issue");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not prove"));
}

#[test]
fn detects_hardcoded_secret_in_mcp_config() {
    let temp = TempDir::new().unwrap();
    let fake_key = "sk-ant-abcdefghijklmnopqrstuvwxyz123456"; // gitleaks:allow
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "packages/app/.cursor/mcp.json",
        &format!(
            r#"
                {{
                  "servers": {{
                    "anthropic": {{
                      "apiKey": "{fake_key}"
                    }}
                  }}
                }}
            "#
        ),
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue
                .id
                .contains("config-secret:packages/app/.cursor/mcp.json")
        })
        .expect("config-secret issue");

    let secret_suffix = "abcdefghijklmnopqrstuvwxyz123456"; // gitleaks:allow
    for field in [
        issue.evidence.as_deref(),
        Some(issue.description.as_str()),
        Some(issue.title.as_str()),
        issue.source_excerpt.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        assert!(
            !field.contains(fake_key) && !field.contains(secret_suffix),
            "config-secret leaked the credential value: {field}"
        );
    }

    assert_eq!(issue.severity, Severity::High);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not verify"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("revoke or rotate"));
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("credential value format"),
        "value-shaped evidence should name the pattern class: {evidence}"
    );
}

#[test]
fn config_secret_name_value_heuristic_is_needs_review() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "packages/app/.cursor/mcp.json",
        r#"
            {
              "servers": {
                "custom": {
                  "apiKey": "your-key-goes-here"
                }
              }
            }
        "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("config-secret:"))
        .expect("config-secret issue"); // allow-expect: test assertion
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(
        issue
            .confidence_reason
            .as_deref()
            .unwrap_or_default()
            .contains("placeholder"),
        "heuristic finding should explain the NeedsReview grade: {:?}",
        issue.confidence_reason
    );
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("secret-named config key"),
        "heuristic evidence should name the pattern class: {evidence}"
    );
    // Hedged title stays.
    assert!(issue.title.contains("may contain"), "got {}", issue.title);
}

#[test]
fn root_mcp_config_secret_is_reported_once_not_twice() {
    let temp = TempDir::new().unwrap();
    let fake_key = "sk-ant-abcdefghijklmnopqrstuvwxyz123456"; // gitleaks:allow
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".mcp.json",
        &format!(r#"{{ "mcpServers": {{ "a": {{ "env": {{ "API_KEY": "{fake_key}" }} }} }} }}"#),
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("agent-instructions-secret:.mcp.json")),
        "the root MCP config secret belongs to agent-instructions-secret"
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("config-secret:")),
        "config-secret must not double-report the same file"
    );
}

#[test]
fn detects_missing_lockfile_for_declared_dependencies() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "demo-app",
                  "dependencies": {
                    "react": "^19.0.0",
                    "zod": "^3.24.0"
                  }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("lockfile-missing:"))
        .expect("expected missing recognized lockfile review");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert_eq!(issue.line, None);
    assert!(issue.title.contains("recognized lockfile"));
}

#[test]
fn workspace_member_is_covered_by_root_lockfile() {
    let temp = TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "monorepo-root", "private": true }"#,
    );
    write_file(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    write_file(
        temp.path(),
        "pnpm-workspace.yaml",
        "packages:\n  - 'apps/*'\n",
    );
    // A member package that declares deps but has no lockfile of its own.
    write_file(
        temp.path(),
        "apps/web/package.json",
        r#"
                {
                  "name": "web",
                  "dependencies": {
                    "react": "^19.0.0"
                  }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("lockfile-missing:")),
        "a workspace member covered by the root lockfile must not be flagged, got: {:?}",
        report
            .issues
            .iter()
            .map(|i| i.id.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn detects_declared_dependency_missing_from_lockfile() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "demo-app",
                  "dependencies": {
                    "react": "^19.0.0",
                    "zod": "^3.24.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "name": "demo-app",
                      "dependencies": {
                        "react": "^19.0.0"
                      }
                    },
                    "node_modules/react": {
                      "version": "19.0.0"
                    }
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/page.tsx",
        r#"
                import React from "react";
                export default function Page() {
                  return <main>Hello</main>;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.contains("lockfile-mismatch:package.json:zod"))
        .expect("expected lockfile parser mismatch review");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("parser"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("frozen install"));
}

#[test]
fn detects_unused_declared_dependency() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "demo-app",
                  "dependencies": {
                    "axios": "^1.8.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "name": "demo-app",
                      "dependencies": {
                        "axios": "^1.8.0"
                      }
                    },
                    "node_modules/axios": {
                      "version": "1.8.1"
                    }
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/page.tsx",
        r#"
                export default function Page() {
                  return <main>Hello</main>;
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.contains("unused-dependency:package.json:axios")));
}

#[test]
fn test_only_dependency_used_in_test_files_is_not_unused() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "supertest": "^7.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "name": "demo-app",
                      "dependencies": { "supertest": "^7.0.0" }
                    },
                    "node_modules/supertest": { "version": "7.0.0" }
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "tests/api.test.ts",
        r#"
                import request from "supertest";
                it("responds", async () => { await request; });
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.contains("unused-dependency")),
        "a dependency imported only from test files is used"
    );
}

#[test]
fn dependency_used_only_in_vue_component_is_not_unused() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "axios": "^1.8.0" } }"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "name": "demo-app",
                      "dependencies": { "axios": "^1.8.0" }
                    },
                    "node_modules/axios": { "version": "1.8.1" }
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "src/App.vue",
        "<script setup>\nimport axios from \"axios\";\n</script>\n<template><div /></template>\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.contains("unused-dependency")),
        "a dependency imported from a Vue component is used"
    );
}

#[test]
fn stylesheet_package_import_is_not_unused() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "normalize.css": "^8.0.1" } }"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "name": "demo-app",
                      "dependencies": { "normalize.css": "^8.0.1" }
                    },
                    "node_modules/normalize.css": { "version": "8.0.1" }
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "src/main.css",
        "@import \"normalize.css\";\nbody { margin: 0; }\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.contains("unused-dependency")),
        "a package pulled in via a stylesheet @import is used"
    );
}

#[test]
fn jsx_import_source_dependency_is_not_unused() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "preact": "^10.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "name": "demo-app",
                      "dependencies": { "preact": "^10.0.0" }
                    },
                    "node_modules/preact": { "version": "10.0.0" }
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "tsconfig.json",
        r#"
                {
                  "compilerOptions": {
                    "jsx": "react-jsx",
                    "jsxImportSource": "preact"
                  }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.contains("unused-dependency:package.json:preact")),
        "jsxImportSource is direct configuration usage of its package"
    );
}

#[test]
fn duplicate_utility_libraries_are_scoped_per_manifest() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "workspace-root", "private": true, "workspaces": ["apps/*"] }"#,
    );
    write_file(
        temp.path(),
        "apps/a/package.json",
        r#"{ "name": "app-a", "dependencies": { "dayjs": "^1.11.0" } }"#,
    );
    write_file(
        temp.path(),
        "apps/b/package.json",
        r#"{ "name": "app-b", "dependencies": { "date-fns": "^3.6.0" } }"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.contains("duplicate-utility-deps")),
        "different members choosing different libraries is a normal monorepo"
    );
}

#[test]
fn duplicate_utility_libraries_in_one_manifest_still_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "dayjs": "^1.11.0", "date-fns": "^3.6.0" } }"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue.id.contains("duplicate-utility-deps") && issue.relative_path == "package.json"
        })
        .expect("multiple utility libraries should prompt a scoped review");
    assert_eq!(issue.severity, Severity::Low);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("may be intentional"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("If their roles overlap"));
}

#[test]
fn legitimate_popular_packages_are_not_flagged_as_suspicious() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "demo-app",
                  "dependencies": {
                    "openai": "^4.0.0",
                    "react": "^19.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenAI from "openai";

                export async function POST() {
                  return Response.json({ ok: !!OpenAI });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("suspicious-package:")),
        "legitimate import flagged: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("suspicious-manifest-package:")),
        "legitimate manifest dependency flagged: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn expected_registry_hosts_are_not_flagged_as_mismatch() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        ".npmrc",
        "@acme:registry=https://npm.acme.dev/\n",
    );
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "registry-app",
                  "dependencies": {
                    "@acme/ui": "^1.0.0",
                    "axios": "^1.7.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"
                {
                  "name": "registry-app",
                  "lockfileVersion": 3,
                  "packages": {
                    "": {
                      "dependencies": {
                        "@acme/ui": "1.2.0",
                        "axios": "1.7.2"
                      }
                    },
                    "node_modules/@acme/ui": {
                      "version": "1.2.0",
                      "resolved": "https://npm.acme.dev/@acme/ui/-/ui-1.2.0.tgz"
                    },
                    "node_modules/axios": {
                      "version": "1.7.2",
                      "resolved": "https://registry.npmjs.org/axios/-/axios-1.7.2.tgz"
                    }
                  }
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("registry-host-mismatch:")),
        "expected-host dependency flagged: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}
