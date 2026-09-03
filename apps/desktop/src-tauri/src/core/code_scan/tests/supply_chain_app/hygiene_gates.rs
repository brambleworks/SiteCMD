use super::super::*;

fn find<'a>(report: &'a CodeScanReport, slug: &str) -> Option<&'a CodeIssue> {
    let prefix = format!("{slug}:");
    report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with(&prefix))
}

fn fires(report: &CodeScanReport, slug: &str) -> bool {
    find(report, slug).is_some()
}

fn write_static_astro_site(root: &Path, extra_scripts: &str) {
    write_file(
        root,
        "package.json",
        &format!(
            r#"{{
  "name": "marketing-site",
  "scripts": {{ "dev": "astro dev", "build": "astro build", "preview": "astro preview"{extra_scripts} }},
  "dependencies": {{ "astro": "^5.0.0" }}
}}"#
        ),
    );
    write_file(
        root,
        "astro.config.mjs",
        "import { defineConfig } from 'astro/config';\nexport default defineConfig({});\n",
    );
    write_file(
        root,
        "src/pages/index.astro",
        "---\nconst title = 'Home';\n---\n<html><body><h1>{title}</h1></body></html>\n",
    );
    write_file(
        root,
        "src/components/Header.astro",
        "<header><nav><a href=\"/\">Home</a></nav></header>\n",
    );
    std::fs::create_dir_all(root.join(".git")).unwrap();
}

fn write_next_app_with_tests(root: &Path) {
    write_file(
        root,
        "package.json",
        r#"{
  "name": "tested-app",
  "scripts": {
    "build": "next build",
    "lint": "eslint .",
    "test": "vitest run"
  },
  "dependencies": { "next": "^16.0.0" },
  "devDependencies": { "eslint": "^9.0.0", "vitest": "^3.0.0" }
}"#,
    );
    write_file(
        root,
        "app/api/contact/route.ts",
        "export async function POST(request: Request) { const body = await request.json(); return Response.json({ ok: Boolean(body.email) }); }\n",
    );
    write_file(
        root,
        "app/api/signup/route.ts",
        "export async function POST() { return Response.json({ ok: true }); }\n",
    );
    write_file(
        root,
        "app/api/contact/route.test.ts",
        "import { test, expect } from 'vitest';\ntest('contact', () => { expect(1).toBe(1); });\n",
    );
    std::fs::create_dir_all(root.join(".git")).unwrap();
}

#[test]
fn static_site_without_any_gate_gets_the_hygiene_findings() {
    let temp = TempDir::new().unwrap();
    write_static_astro_site(temp.path(), "");

    let report = audit_project(temp.path()).unwrap();

    let ci = find(&report, "ci-workflow-missing").expect("a buildable site needs CI review");
    assert_eq!(ci.severity, Severity::Medium);
    let hooks =
        find(&report, "pre-commit-hooks-missing").expect("a buildable site needs hook review");
    assert_eq!(
        hooks.severity,
        Severity::Medium,
        "with no CI, the local hook is the only gate, so its absence is Medium"
    );
    assert!(hooks
        .description
        .contains("No CI quality gate was found either"));
    assert!(fires(&report, "linter-missing"));
    let tests = find(&report, "no-automated-tests").expect("static site still reports the gap");
    assert_eq!(
        tests.severity,
        Severity::Low,
        "no server routes or data access, so missing tests grade Low"
    );
    assert!(tests.description.contains("scanned project"));
    assert!(tests.description.contains("may exist outside"));
    assert!(!fires(&report, "build-script-missing"));
    assert!(!fires(&report, "critical-path-no-test"));
    assert!(!fires(&report, "tests-not-enforced"));
}

#[test]
fn vite_single_page_app_is_a_hygiene_project() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "spa",
  "scripts": { "dev": "vite", "build": "vite build" },
  "dependencies": { "react": "^19.0.0", "react-dom": "^19.0.0" },
  "devDependencies": { "vite": "^6.0.0" }
}"#,
    );
    write_file(
        temp.path(),
        "vite.config.ts",
        "import { defineConfig } from 'vite';\nexport default defineConfig({});\n",
    );
    write_file(
        temp.path(),
        "src/main.tsx",
        "import { createRoot } from 'react-dom/client';\ncreateRoot(document.getElementById('root')!).render(<div>hi</div>);\n",
    );
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    assert!(fires(&report, "ci-workflow-missing"));
    assert!(fires(&report, "pre-commit-hooks-missing"));
    assert!(fires(&report, "linter-missing"));
}

#[test]
fn tooling_only_package_json_is_not_a_hygiene_project() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{ "name": "notes", "devDependencies": { "prettier": "^3.0.0" } }"#,
    );
    write_file(temp.path(), "README.md", "# Notes\n");
    write_file(temp.path(), "src/notes.md", "hello\n");
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    assert!(!fires(&report, "ci-workflow-missing"));
    assert!(!fires(&report, "pre-commit-hooks-missing"));
    assert!(!fires(&report, "linter-missing"));
    assert!(!fires(&report, "no-automated-tests"));
}

#[test]
fn npm_placeholder_test_script_is_not_test_infrastructure() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "untested-app",
  "scripts": {
    "build": "next build",
    "test": "echo \"Error: no test specified\" && exit 1"
  },
  "dependencies": { "next": "^16.0.0" }
}"#,
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
    let issue =
        find(&report, "no-automated-tests").expect("the npm placeholder is not a test suite");
    assert_eq!(issue.severity, Severity::Medium);
    assert!(
        issue
            .evidence
            .as_deref()
            .is_some_and(|evidence| evidence.contains("placeholder")),
        "evidence names the placeholder script: {:?}",
        issue.evidence
    );
}

#[test]
fn echo_only_scripts_do_not_count_as_quality_commands() {
    let temp = TempDir::new().unwrap();
    write_static_astro_site(
        temp.path(),
        r#", "lint": "echo lint skipped", "test": "exit 0""#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(fires(&report, "linter-missing"));
    assert!(fires(&report, "no-automated-tests"));
}

#[test]
fn tests_that_exist_but_never_run_automatically_are_flagged() {
    let temp = TempDir::new().unwrap();
    write_next_app_with_tests(temp.path());
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: npm ci\n      - run: npm run build\n",
    );
    write_file(temp.path(), ".husky/pre-commit", "npm run lint\n");
    std::fs::create_dir_all(temp.path().join(".husky/_")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    let issue = find(&report, "tests-not-enforced")
        .expect("CI builds and the hook lints, but nothing runs the tests");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue.relative_path.ends_with("ci.yml"));
    assert!(!fires(&report, "ci-quality-gate-missing"));
    assert!(!fires(&report, "ci-only-builds"));
    assert!(!fires(&report, "no-automated-tests"));
}

#[test]
fn tests_run_by_ci_or_a_hook_are_enforced() {
    let with_ci = TempDir::new().unwrap();
    write_next_app_with_tests(with_ci.path());
    write_file(
        with_ci.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: npm run build\n      - run: npm test\n",
    );
    write_file(with_ci.path(), ".husky/pre-commit", "npm run lint\n");
    std::fs::create_dir_all(with_ci.path().join(".husky/_")).unwrap();
    let report = audit_project(with_ci.path()).unwrap();
    assert!(!fires(&report, "tests-not-enforced"));

    let with_hook = TempDir::new().unwrap();
    write_next_app_with_tests(with_hook.path());
    write_file(
        with_hook.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: npm run build\n",
    );
    write_file(with_hook.path(), ".husky/pre-push", "npm test\n");
    std::fs::create_dir_all(with_hook.path().join(".husky/_")).unwrap();
    let report = audit_project(with_hook.path()).unwrap();
    assert!(!fires(&report, "tests-not-enforced"));

    let by_script_name = TempDir::new().unwrap();
    write_next_app_with_tests(by_script_name.path());
    write_file(
        by_script_name.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: pnpm run --if-present test\n",
    );
    write_file(by_script_name.path(), ".husky/pre-commit", "npm run lint\n");
    std::fs::create_dir_all(by_script_name.path().join(".husky/_")).unwrap();
    let report = audit_project(by_script_name.path()).unwrap();
    assert!(!fires(&report, "tests-not-enforced"));
}

#[test]
fn tests_not_enforced_stays_quiet_when_no_automation_exists_at_all() {
    let temp = TempDir::new().unwrap();
    write_next_app_with_tests(temp.path());

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !fires(&report, "tests-not-enforced"),
        "ci-workflow-missing and pre-commit-hooks-missing already describe this gap"
    );
    assert!(fires(&report, "ci-workflow-missing"));
    assert!(fires(&report, "pre-commit-hooks-missing"));
}

#[test]
fn checked_in_hook_config_that_is_not_installed_in_this_clone_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_next_app_with_tests(temp.path());
    write_file(temp.path(), ".husky/pre-commit", "npm run lint\nnpm test\n");

    let report = audit_project(temp.path()).unwrap();
    let issue = find(&report, "pre-commit-hooks-not-installed")
        .expect("husky config exists but no hook is wired into .git");
    assert_eq!(issue.confidence, crate::checks::IssueConfidence::High);
    assert_eq!(
        issue.severity,
        Severity::Medium,
        "nothing else enforces quality, so an inactive hook is Medium"
    );
    assert!(issue.relative_path.ends_with(".husky/pre-commit"));
    assert!(!fires(&report, "pre-commit-hooks-missing"));
    assert!(!fires(&report, "pre-commit-hooks-weak"));
}

#[test]
fn installed_hooks_are_recognized_through_husky_native_hooks_or_hooks_path() {
    let husky = TempDir::new().unwrap();
    write_next_app_with_tests(husky.path());
    write_file(husky.path(), ".husky/pre-commit", "npm run lint\n");
    write_file(husky.path(), ".husky/_/husky.sh", "#!/bin/sh\n");
    let report = audit_project(husky.path()).unwrap();
    assert!(!fires(&report, "pre-commit-hooks-not-installed"));

    let native = TempDir::new().unwrap();
    write_next_app_with_tests(native.path());
    write_file(
        native.path(),
        "lefthook.yml",
        "pre-commit:\n  commands:\n    lint:\n      run: npm run lint\n",
    );
    write_file(
        native.path(),
        ".git/hooks/pre-commit",
        "#!/bin/sh\nlefthook run pre-commit\n",
    );
    let report = audit_project(native.path()).unwrap();
    assert!(!fires(&report, "pre-commit-hooks-not-installed"));

    let hooks_path = TempDir::new().unwrap();
    write_next_app_with_tests(hooks_path.path());
    write_file(hooks_path.path(), ".husky/pre-commit", "npm run lint\n");
    write_file(
        hooks_path.path(),
        ".git/config",
        "[core]\n\trepositoryformatversion = 0\n\thooksPath = .husky/_\n",
    );
    let report = audit_project(hooks_path.path()).unwrap();
    assert!(!fires(&report, "pre-commit-hooks-not-installed"));
}

#[test]
fn hook_absence_grades_low_once_ci_enforces_quality_or_a_local_hook_exists() {
    let enforced = TempDir::new().unwrap();
    write_next_app_with_tests(enforced.path());
    write_file(
        enforced.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: npm test\n",
    );
    let report = audit_project(enforced.path()).unwrap();
    let issue = find(&report, "pre-commit-hooks-missing").expect("hooks are still recommended");
    assert_eq!(issue.severity, Severity::Low);
    assert!(!issue
        .description
        .contains("No CI quality gate was found either"));

    let local_only = TempDir::new().unwrap();
    write_next_app_with_tests(local_only.path());
    write_file(
        local_only.path(),
        ".git/hooks/pre-push",
        "#!/bin/sh\nsitecmd check --strict\n",
    );
    let report = audit_project(local_only.path()).unwrap();
    let issue =
        find(&report, "pre-commit-hooks-missing").expect("a clone-local hook is not shared");
    assert_eq!(issue.severity, Severity::Low);
    assert!(
        issue
            .evidence
            .as_deref()
            .is_some_and(|evidence| evidence.contains("this clone only")),
        "evidence explains the clone-local hook: {:?}",
        issue.evidence
    );
}

#[test]
fn decorative_hook_grades_medium_without_an_enforcing_ci() {
    let temp = TempDir::new().unwrap();
    write_next_app_with_tests(temp.path());
    write_file(
        temp.path(),
        ".husky/pre-commit",
        "echo collecting metadata\n",
    );
    std::fs::create_dir_all(temp.path().join(".husky/_")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    let issue = find(&report, "pre-commit-hooks-weak").expect("hook runs nothing useful");
    assert_eq!(issue.severity, Severity::Medium);
}

#[test]
fn hosting_config_without_ci_is_named_in_the_ci_evidence() {
    let temp = TempDir::new().unwrap();
    write_static_astro_site(temp.path(), "");
    write_file(
        temp.path(),
        "netlify.toml",
        "[build]\n  command = \"astro build\"\n  publish = \"dist\"\n",
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = find(&report, "ci-workflow-missing").expect("a host build is not a quality gate");
    assert!(
        issue
            .evidence
            .as_deref()
            .is_some_and(|evidence| evidence.contains("netlify.toml")),
        "evidence names the hosting config: {:?}",
        issue.evidence
    );
}

fn write_laravel_app(root: &Path, composer_extra: &str) {
    write_file(
        root,
        "composer.json",
        &format!(
            r#"{{
  "name": "acme/shop",
  "require": {{ "php": "^8.3", "laravel/framework": "^12.0" }},
  "require-dev": {{ "phpunit/phpunit": "^11.0", "laravel/pint": "^1.0"{composer_extra} }},
  "scripts": {{ "test": "vendor/bin/phpunit", "lint": "vendor/bin/pint --test" }}
}}"#
        ),
    );
    write_file(
        root,
        "routes/web.php",
        "<?php\nuse Illuminate\\Support\\Facades\\Route;\nRoute::get('/', fn () => view('welcome'));\n",
    );
    write_file(
        root,
        "routes/api.php",
        "<?php\nuse Illuminate\\Support\\Facades\\Route;\nRoute::post('/orders', [OrderController::class, 'store']);\n",
    );
    write_file(
        root,
        "phpunit.xml",
        "<?xml version=\"1.0\"?>\n<phpunit><testsuites><testsuite name=\"Feature\"><directory>tests/Feature</directory></testsuite></testsuites></phpunit>\n",
    );
    write_file(
        root,
        "tests/Feature/ExampleTest.php",
        "<?php\nnamespace Tests\\Feature;\nuse Tests\\TestCase;\nclass ExampleTest extends TestCase { public function test_home(): void { $this->get('/')->assertStatus(200); } }\n",
    );
    std::fs::create_dir_all(root.join(".git")).unwrap();
}

#[test]
fn composer_projects_get_hygiene_review_and_recognize_php_quality_tools() {
    let temp = TempDir::new().unwrap();
    write_laravel_app(temp.path(), "");

    let report = audit_project(temp.path()).unwrap();
    assert!(!fires(&report, "linter-missing"), "pint is a linter");
    assert!(
        !fires(&report, "no-automated-tests"),
        "phpunit.xml and tests/ exist"
    );
    let ci = find(&report, "ci-workflow-missing").expect("composer roots get CI review");
    assert!(ci.relative_path.ends_with("composer.json"));
    let hooks = find(&report, "pre-commit-hooks-missing").expect("composer roots get hook review");
    assert_eq!(hooks.severity, Severity::Medium);
    assert!(
        !fires(&report, "build-script-missing"),
        "PHP has no build script"
    );
}

#[test]
fn composer_test_and_pint_commands_count_as_ci_gates() {
    let temp = TempDir::new().unwrap();
    write_laravel_app(temp.path(), "");
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: composer install --no-interaction\n      - run: vendor/bin/pint --test\n      - run: composer test\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!fires(&report, "ci-workflow-missing"));
    assert!(!fires(&report, "ci-quality-gate-missing"));
    assert!(!fires(&report, "tests-not-enforced"));
    let hooks = find(&report, "pre-commit-hooks-missing").expect("hooks still recommended");
    assert_eq!(hooks.severity, Severity::Low);
}

#[test]
fn composer_ci_that_only_lints_leaves_tests_unenforced() {
    let temp = TempDir::new().unwrap();
    write_laravel_app(temp.path(), "");
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: vendor/bin/pint --test\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(fires(&report, "tests-not-enforced"));
}

#[test]
fn a_test_runner_dependency_alone_does_not_make_tests_unenforced() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "site",
  "scripts": { "build": "astro build", "lint": "biome lint" },
  "dependencies": { "astro": "^5.0.0" },
  "devDependencies": { "@playwright/test": "^1.50.0", "@biomejs/biome": "^2.0.0" }
}"#,
    );
    write_file(temp.path(), "astro.config.mjs", "export default {};\n");
    write_file(temp.path(), "src/pages/index.astro", "<h1>Home</h1>\n");
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        "name: CI\non: [push]\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: pnpm run lint\n",
    );
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !fires(&report, "tests-not-enforced"),
        "a dependency is not a suite; nothing exists for CI to run"
    );
    assert!(!fires(&report, "no-automated-tests"));
}

#[test]
fn a_staged_file_formatter_is_a_quality_hook() {
    let temp = TempDir::new().unwrap();
    write_next_app_with_tests(temp.path());
    write_file(
        temp.path(),
        ".husky/pre-commit",
        "#!/usr/bin/env sh\n. \"$(dirname -- \"$0\")/_/husky.sh\"\n\nnpx pretty-quick --staged\n",
    );
    std::fs::create_dir_all(temp.path().join(".husky/_")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    assert!(!fires(&report, "pre-commit-hooks-weak"));
}

#[test]
fn tooling_only_package_json_in_a_php_project_owes_no_build_script() {
    let temp = TempDir::new().unwrap();
    write_laravel_app(temp.path(), "");
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "shop-tooling",
  "scripts": { "test:e2e": "playwright test" },
  "devDependencies": { "@playwright/test": "^1.50.0" }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!fires(&report, "build-script-missing"));
    assert!(fires(&report, "ci-workflow-missing"));
}

#[test]
fn laravel_mix_production_script_counts_as_the_build_script() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
                {
                  "name": "theme",
                  "scripts": {
                    "dev": "mix",
                    "watch": "mix watch",
                    "production": "mix --production"
                  },
                  "devDependencies": {
                    "bootstrap": "^5.3.3",
                    "laravel-mix": "^6.0.18",
                    "webpack": "^5.0.0"
                  }
                }
            "#,
    );
    write_file(
        temp.path(),
        "webpack.mix.js",
        "const mix = require('laravel-mix');\nmix.js('src/js/main.js', 'build/js').sass('src/scss/main.scss', 'build/css');\n",
    );
    write_file(
        temp.path(),
        "src/js/main.js",
        "import 'bootstrap/js/dist/collapse';\nconsole.log('ready');\n",
    );
    std::fs::create_dir_all(temp.path().join(".git")).unwrap();

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !fires(&report, "build-script-missing"),
        "`mix --production` is Laravel Mix's documented production build, got {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}
