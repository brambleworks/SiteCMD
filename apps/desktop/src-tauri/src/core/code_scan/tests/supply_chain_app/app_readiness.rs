use super::super::*;

#[test]
fn detects_missing_env_template_and_ai_kill_switch() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "demo-app", "dependencies": { "openai": "^4.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/api/chat/route.ts",
        r#"
                import OpenAI from "openai";

                export async function POST() {
                  const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
                  return Response.json(await client.responses.create({ model: "gpt-4.1-mini", input: "hi" }));
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("env-example-missing:")));
    let kill_switch = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ai-kill-switch-missing:"))
        .expect("AI disable-path review");
    assert_eq!(kill_switch.severity, Severity::Medium);
    assert_eq!(
        kill_switch.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(kill_switch
        .description
        .contains("outside the scanned patterns"));
}

#[test]
fn detects_missing_healthcheck_observability_and_migrations() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "demo-app",
                  "dependencies": {
                    "@prisma/client": "^6.0.0",
                    "next": "^16.0.0"
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
    write_file(
        temp.path(),
        "app/api/orders/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                export async function POST() {
                  const prisma = new PrismaClient();
                  await prisma.order.create({ data: { status: "pending" } });
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/account/route.ts",
        r#"
                export async function PATCH() {
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/billing/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "src/backup.ts",
        "export async function backupRecord() { return { ok: true }; }",
    );

    let report = audit_project(temp.path()).unwrap();
    let health = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("healthcheck-missing:"))
        .expect("health/readiness review");
    assert_eq!(
        health.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert_eq!(health.line, None);
    assert!(health.description.contains("scanned routes"));
    let error_reporting = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("error-reporting-missing:"))
        .expect("expected error-reporting review");
    assert_eq!(
        error_reporting.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(error_reporting.description.contains("scanned dependencies"));
    assert!(error_reporting.description.contains("does not establish"));
    assert_eq!(error_reporting.line, None);
    let structured_logging = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("structured-logging-missing:"))
        .expect("expected structured-logging review");
    assert_eq!(
        structured_logging.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(structured_logging
        .description
        .contains("scanned server code"));
    assert!(structured_logging.description.contains("may still exist"));
    assert_eq!(structured_logging.line, None);
    let error_boundary = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("error-boundary-missing:"))
        .expect("frontend error-surface review");
    assert_eq!(
        error_boundary.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert_eq!(error_boundary.line, None);
    let migration = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("migration-workflow-missing:"))
        .expect("schema-change workflow review");
    assert!(migration.title.contains("schema-change"));
    assert_eq!(migration.line, None);
    let recovery = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("recovery-runbook-missing:"))
        .expect("recovery documentation review");
    assert_eq!(recovery.line, None);
    assert!(recovery.description.contains("outside the scanned tree"));
}

#[test]
fn recognizes_a_structured_log_event_wrapper() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "worker-app", "dependencies": { "hono": "^4.0.0" } }"#,
    );
    for name in ["orders", "account", "billing"] {
        write_file(
            temp.path(),
            &format!("src/routes/{name}.ts"),
            "export async function POST() { return Response.json({ ok: true }); }",
        );
    }
    write_file(
        temp.path(),
        "src/observability.ts",
        r#"
                export function logEvent(level: "error" | "warn", event: string): void {
                  console[level](JSON.stringify({ event, service: "worker-app" }));
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("structured-logging-missing:")),
        "a stable wrapper that emits JSON with an event name is structured logging"
    );
}

#[test]
fn detects_missing_background_job_visibility() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "worker-app",
                  "dependencies": {
                    "bullmq": "^5.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "src/worker.ts",
        r#"
                import { Worker } from "bullmq";

                export const emailWorker = new Worker("email", async (job) => {
                  console.log(job.data);
                });
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("job-visibility-missing:"))
        .expect("background job visibility review");
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.description.contains("does not establish"));
    assert!(!issue.description.contains("built quickly"));
}

#[test]
fn detects_missing_launch_release_safety_infrastructure() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "launch-risk-app",
                  "scripts": {
                    "lint": "eslint ."
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  },
                  "devDependencies": {
                    "eslint": "^9.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ ok: Boolean(body.email) });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );
    // Repo-hygiene checks (ci-workflow-missing) only fire at an actual git repo
    // root, so this fixture must own a `.git`.
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    let ci_issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("ci-workflow-missing:"))
        .expect("expected missing recognized CI workflow review");
    assert_eq!(
        ci_issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(ci_issue.title.contains("recognized CI workflow"));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("build-script-missing:")));
    let hook_issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("pre-commit-hooks-missing:"))
        .expect("expected optional hook review");
    assert_eq!(hook_issue.severity, Severity::Low);
    assert_eq!(
        hook_issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
}

#[test]
fn missing_test_infrastructure_is_scoped_to_recognized_project_signals() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "untested-app", "dependencies": { "next": "^16.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/api/users/route.ts",
        "export async function GET() { return Response.json({ ok: true }); }",
    );
    write_file(
        temp.path(),
        "app/api/accounts/route.ts",
        "export async function POST() { return Response.json({ ok: true }); }",
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("no-automated-tests:"))
        .expect("expected missing test-infrastructure review");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.title.contains("No recognized"));
    assert!(issue.description.contains("scanned project"));
    assert!(issue.description.contains("may exist outside"));
    assert!(!issue.description.contains("every change depends entirely"));
}

#[test]
fn env_ignore_review_requires_a_nonexample_env_file_in_the_scanned_project() {
    let temp = TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();
    write_file(temp.path(), ".gitignore", "node_modules\n");
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "env-app", "dependencies": { "next": "^16.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "app/api/users/route.ts",
        "export async function GET() { return Response.json({ url: process.env.API_URL }); }",
    );

    let without_env_file = audit_project(temp.path()).unwrap();
    assert!(!without_env_file
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("gitignore-missing-env:")));

    write_file(temp.path(), ".env.local", "API_URL=http://localhost:3000\n");
    let with_env_file = audit_project(temp.path()).unwrap();
    let issue = with_env_file
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("gitignore-missing-env:"))
        .expect("an actual local env file should trigger the ignore review");
    assert!(issue.description.contains("non-example environment file"));
    assert!(issue
        .likely_fix
        .as_deref()
        .unwrap_or_default()
        .contains("already tracked"));
}

#[test]
fn detects_missing_deploy_rollback_plan_for_launch_apps() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "rollback-risk-app",
                  "scripts": {
                    "build": "next build",
                    "start": "next start"
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "vercel.json",
        r#"{ "buildCommand": "npm run build" }"#,
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("deploy-rollback-plan-missing:"))
        .expect("rollback documentation review");
    assert_eq!(issue.line, None);
    assert!(issue.description.contains("scanned documentation"));
    assert!(issue
        .description
        .to_ascii_lowercase()
        .contains("provider-side"));
}

#[test]
fn skips_deploy_rollback_plan_when_runbook_mentions_rollback() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "rollback-ready-app",
                  "scripts": {
                    "build": "next build",
                    "start": "next start"
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "vercel.json",
        r#"{ "buildCommand": "npm run build" }"#,
    );
    write_file(
        temp.path(),
        "docs/deploy-runbook.md",
        "# Deploy Runbook\n\nRollback by redeploying the last known-good Vercel deployment.\n",
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("deploy-rollback-plan-missing:")));
}

#[test]
fn detects_recovery_notes_that_omit_backup_restore_steps() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "stateful-runbook-app",
                  "dependencies": {
                    "@prisma/client": "^6.0.0",
                    "next": "^16.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "docs/recovery-runbook.md",
        "# Recovery Runbook\n\nRestart the app and check the incident channel.\n",
    );
    write_file(
        temp.path(),
        "app/api/orders/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                export async function POST() {
                  const prisma = new PrismaClient();
                  await prisma.order.create({ data: { status: "pending" } });
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/account/route.ts",
        r#"
                export async function PATCH() {
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/billing/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("recovery-runbook-missing:")));
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("backup-restore-plan-missing:"))
        .expect("backup/restore documentation review");
    assert_eq!(issue.line, None);
    assert!(issue.description.contains("scanned recovery"));
    assert!(issue.description.contains("outside the scanned tree"));
    assert!(issue
        .verify_hint
        .as_deref()
        .is_some_and(|text| text.contains("non-production destination")));
    assert!(!issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("every credential"));
}

#[test]
fn seed_script_alone_does_not_masquerade_as_a_schema_change_workflow() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
          "name": "seed-only-app",
          "scripts": { "seed": "tsx db/seed.ts" },
          "dependencies": { "@prisma/client": "^6.0.0" }
        }"#,
    );
    write_file(
        temp.path(),
        "db/seed.ts",
        "export async function seed() {}\n",
    );
    write_file(
        temp.path(),
        "app/api/orders/route.ts",
        "export async function GET() { return Response.json({ ok: true }); }",
    );
    write_file(
        temp.path(),
        "app/api/accounts/route.ts",
        "export async function GET() { return Response.json({ ok: true }); }",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("migration-workflow-missing:")));
}

#[test]
fn skips_launch_release_safety_findings_when_ci_build_and_hooks_exist() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "guarded-launch-app",
                  "scripts": {
                    "build": "next build",
                    "lint": "eslint .",
                    "test": "vitest run",
                    "prepare": "husky"
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  },
                  "devDependencies": {
                    "eslint": "^9.0.0",
                    "husky": "^9.0.0",
                    "vitest": "^3.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: npm run build\n      - run: npm test\n",
    );
    write_file(temp.path(), ".husky/pre-commit", "npm run lint\n");
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ ok: Boolean(body.email) });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ci-workflow-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("build-script-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("pre-commit-hooks-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ci-quality-gate-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("pre-commit-hooks-weak:")));
}

#[test]
fn turbo_ci_workflow_counts_as_a_quality_gate() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "turbo-app",
                  "scripts": {
                    "build": "next build",
                    "lint": "eslint .",
                    "test": "vitest run"
                  },
                  "dependencies": { "next": "^16.0.0" },
                  "devDependencies": { "eslint": "^9.0.0", "vitest": "^3.0.0", "turbo": "^2.0.0" }
                }
            "#,
    );
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: pnpm turbo run build lint test\n",
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ ok: Boolean(body.email) });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("ci-quality-gate-missing:")),
        "a turbo invocation of build/lint/test is a quality gate"
    );
}

#[test]
fn root_level_pytest_file_counts_as_test_infrastructure() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app.py",
        r#"
from flask import Flask
app = Flask(__name__)

@app.route("/api/users")
def users():
    return {"ok": True}
"#,
    );
    write_file(
        temp.path(),
        "test_app.py",
        "def test_users():\n    assert True\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("no-automated-tests:")),
        "root-level test_app.py is test infrastructure"
    );
}

#[test]
fn nested_gitignore_covering_env_is_read() {
    let temp = TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();
    write_file(temp.path(), "apps/web/.gitignore", ".env\nnode_modules\n");
    write_file(
        temp.path(),
        "apps/web/package.json",
        r#"{ "name": "web", "dependencies": { "next": "^16.0.0" } }"#,
    );
    write_file(
        temp.path(),
        "apps/web/app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ key: process.env.RESEND_API_KEY, ok: Boolean(body.email) });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("gitignore-missing-env:")),
        "the nested .gitignore that covers .env must be read"
    );
}

#[test]
fn detects_ci_workflow_that_does_not_run_quality_gates() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "decorative-ci-app",
                  "scripts": {
                    "build": "next build",
                    "lint": "eslint .",
                    "test": "vitest run"
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  },
                  "devDependencies": {
                    "eslint": "^9.0.0",
                    "vitest": "^3.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  noop:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: echo ready\n",
    );
    write_file(temp.path(), ".husky/pre-commit", "npm run lint\n");
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ ok: Boolean(body.email) });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ci-workflow-missing:")));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ci-quality-gate-missing:")));
}

#[test]
fn detects_checked_in_hook_that_does_not_run_quality_gates() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "decorative-hook-app",
                  "scripts": {
                    "build": "next build",
                    "lint": "eslint .",
                    "test": "vitest run"
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  },
                  "devDependencies": {
                    "eslint": "^9.0.0",
                    "husky": "^9.0.0",
                    "vitest": "^3.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: npm test\n",
    );
    write_file(
        temp.path(),
        ".husky/pre-commit",
        "echo collecting metadata\n",
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ ok: Boolean(body.email) });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("pre-commit-hooks-missing:")));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("pre-commit-hooks-weak:")));
}

#[test]
fn skips_recovery_observability_issues_when_surfaces_exist() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "observed-app",
                  "dependencies": {
                    "@prisma/client": "^6.0.0",
                    "next": "^16.0.0",
                    "bullmq": "^5.0.0",
                    "pino": "^9.0.0"
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
    write_file(
        temp.path(),
        "app/error.tsx",
        r#"
                "use client";

                export default function Error() {
                  return <main>Something went wrong</main>;
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/orders/route.ts",
        r#"
                import { PrismaClient } from "@prisma/client";

                export async function POST() {
                  const prisma = new PrismaClient();
                  await prisma.order.create({ data: { status: "pending" } });
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        "src/worker.ts",
        r#"
                import { Worker } from "bullmq";
                import pino from "pino";

                const logger = pino();
                export const emailWorker = new Worker("email", async (job) => {
                  logger.info({ job_id: job.id, queue_name: "email" }, "processing");
                });

                emailWorker.on("failed", (job) => {
                  logger.error({ job_id: job?.id }, "failed");
                });
            "#,
    );
    write_file(
        temp.path(),
        "docs/recovery-runbook.md",
        "# Recovery Runbook\n\nRestore from latest backup before retrying migrations.\n",
    );
    write_file(
        temp.path(),
        "prisma/schema.prisma",
        "datasource db { provider = \"postgresql\" url = env(\"DATABASE_URL\") }\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("error-boundary-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("job-visibility-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("recovery-runbook-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("backup-restore-plan-missing:")));
}

#[test]
fn skips_repo_hygiene_findings_for_monorepo_subpackage_without_git() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
            {
              "name": "subpackage-app",
              "scripts": { "lint": "eslint ." },
              "dependencies": { "next": "^16.0.0" },
              "devDependencies": { "eslint": "^9.0.0" }
            }
        "#,
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
            export async function POST(request: Request) {
              const body = await request.json();
              return Response.json({ ok: Boolean(body.email) });
            }
        "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("ci-workflow-missing:")));
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("gitignore-missing")));
}

#[test]
fn ci_with_build_but_no_quality_scripts_is_flagged_as_ci_only_builds() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "build-only-app",
                  "scripts": {
                    "build": "next build"
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ ok: Boolean(body.email) });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        r#"
                name: CI
                on: [push]
                jobs:
                  build:
                    runs-on: ubuntu-latest
                    steps:
                      - uses: actions/checkout@v4
                      - run: npm run build
            "#,
    );
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("ci-only-builds:")),
        "expected ci-only-builds, got: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn project_without_any_linter_config_is_flagged_as_linter_missing() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "unlinted-app",
                  "scripts": {
                    "build": "next build"
                  },
                  "dependencies": {
                    "next": "^16.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/contact/route.ts",
        r#"
                export async function POST(request: Request) {
                  const body = await request.json();
                  return Response.json({ ok: Boolean(body.email) });
                }
            "#,
    );
    write_file(
        temp.path(),
        "app/api/signup/route.ts",
        r#"
                export async function POST() {
                  return Response.json({ ok: true });
                }
            "#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("linter-missing:")),
        "expected linter-missing, got: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}

#[test]
fn todo_littered_project_is_flagged_as_placeholder_density() {
    let temp = TempDir::new().unwrap();
    for (name, marker) in [
        ("src/lib/auth.ts", "TODO"),
        ("src/lib/billing.ts", "FIXME"),
        ("src/lib/email.ts", "HACK"),
    ] {
        write_file(
            temp.path(),
            name,
            &format!(
                r#"
                // {marker}: wire up the real implementation
                // {marker}: handle the failure path
                // {marker}: remove this stub before launch
                export function pending() {{
                  return null;
                }}
            "#
            ),
        );
    }

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("placeholder-density:"))
        .unwrap_or_else(|| {
            panic!(
                "expected placeholder-density, got: {:?}",
                report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
            )
        });
    assert_ne!(issue.severity, Severity::High);
    assert_eq!(issue.confidence, crate::checks::IssueConfidence::Confirmed);
    assert!(issue.title.contains("needs review"));
    assert!(!issue.description.contains("generated quickly"));
    assert!(!issue.description.contains("shipped before"));
    assert!(!issue.description.contains("Each marker is"));
}

#[test]
fn sparse_markers_in_a_large_project_are_not_flagged_as_placeholder_density() {
    let temp = TempDir::new().unwrap();
    for index in 0..40 {
        let marker = if index < 15 {
            "// TODO: tracked work\n"
        } else {
            ""
        };
        write_file(
            temp.path(),
            &format!("src/lib/module-{index}.ts"),
            &format!("{marker}export const value{index} = {index};\n"),
        );
    }

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("placeholder-density:")),
        "sparse markers flagged: {:?}",
        report
            .issues
            .iter()
            .map(|issue| &issue.id)
            .collect::<Vec<_>>()
    );
}
