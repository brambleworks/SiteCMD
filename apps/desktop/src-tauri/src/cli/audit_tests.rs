use super::*;
use tempfile::TempDir;

fn fixture_token() -> String {
    ["sk", "_live_", "abcdefghijklmnopqrstu"].concat()
}

fn vulnerable_project() -> TempDir {
    let project = TempDir::new().expect("temp project");
    std::fs::create_dir_all(project.path().join("src")).expect("source directory");
    std::fs::write(
        project.path().join("package.json"),
        r#"{ "name": "sitecmd-audit-fixture" }"#,
    )
    .expect("project manifest");
    std::fs::write(
        project.path().join("src/keys.js"),
        format!("const key = \"{}\";\n", fixture_token()),
    )
    .expect("source fixture");
    project
}

#[test]
fn parses_every_supported_report_option() {
    let args = parse_args([
        ".".into(),
        "--format".into(),
        "review".into(),
        "--fail-on".into(),
        "high".into(),
        "--output".into(),
        "reports/sitecmd.md".into(),
    ])
    .expect("valid audit args");

    assert_eq!(args.project_path, PathBuf::from("."));
    assert_eq!(args.format, CodeScanReportFormat::Review);
    assert_eq!(args.fail_on, Some(Severity::High));
    assert_eq!(args.output, Some(PathBuf::from("reports/sitecmd.md")));
    assert!(!args.inspect_local_databases);
}

#[test]
fn local_database_inspection_requires_an_explicit_flag() {
    let args = parse_args([".".into(), "--inspect-local-databases".into()])
        .expect("valid local database opt-in");
    assert!(args.inspect_local_databases);
}

#[test]
fn mysql_inspection_fails_with_an_explicit_unsupported_engine_error() {
    let project = TempDir::new().expect("temp project");
    std::fs::write(
        project.path().join("package.json"),
        r#"{ "name": "mysql-inspection-fixture" }"#,
    )
    .expect("project manifest");
    std::fs::write(
        project.path().join(".env.local"),
        "DATABASE_URL=mysql://root:fixture@localhost:3306/example\n",
    )
    .expect("local env fixture");
    let args = AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Summary,
        fail_on: None,
        output: None,
        inspect_local_databases: true,
        baseline: None,
    };

    let error = run(&args).expect_err("unsupported database inspection must fail closed");

    assert!(error.contains("DATABASE_URL from .env.local"), "{error}");
    assert!(
        error.contains("MySQL and MariaDB inspection is not supported"),
        "{error}"
    );
}

#[test]
fn rejects_missing_paths_and_unknown_formats() {
    assert!(parse_args(Vec::<String>::new())
        .expect_err("path is required")
        .contains("Missing project path"));
    assert!(parse_args([".".into(), "--format".into(), "xml".into()])
        .expect_err("format is bounded")
        .contains("Unknown format"));
}

#[test]
fn runs_the_real_code_scan_and_applies_the_threshold() {
    let project = vulnerable_project();
    let args = AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Json,
        fail_on: Some(Severity::High),
        output: None,
        inspect_local_databases: false,
        baseline: None,
    };

    let outcome = run(&args).expect("audit completes");
    let report: serde_json::Value =
        serde_json::from_str(&outcome.rendered).expect("valid report json");
    assert!(outcome.threshold_failed);
    assert!(report["issues"]
        .as_array()
        .is_some_and(|issues| !issues.is_empty()));
}

#[test]
fn project_config_suppresses_an_exact_rule_and_path_without_hiding_it() {
    let project = vulnerable_project();
    let sitecmd_dir = project.path().join(".sitecmd");
    std::fs::create_dir_all(&sitecmd_dir).expect("sitecmd config directory");
    std::fs::write(
        sitecmd_dir.join("config.json"),
        r#"{
  "version": 1,
  "url": "https://example.com",
  "name": "suppression fixture",
  "code_scan": {
    "suppressions": [
      {
        "match": {
          "rule": "code_scan.hardcoded-secret",
          "path": "src/keys.js"
        },
        "reason": "The credential-shaped value is an inert scanner fixture."
      }
    ]
  }
}"#,
    )
    .expect("suppression config");
    let args = AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Json,
        fail_on: Some(Severity::High),
        output: None,
        inspect_local_databases: false,
        baseline: None,
    };

    let outcome = run(&args).expect("audit completes");
    let report: serde_json::Value =
        serde_json::from_str(&outcome.rendered).expect("valid report json");

    assert!(!outcome.threshold_failed);
    assert_eq!(report["issueCount"], 0);
    assert_eq!(report["ignoredCount"], 1);
    assert_eq!(report["staleSuppressionCount"], 0);
    assert_eq!(report["issues"], serde_json::json!([]));
    assert_eq!(
        report["ignoredFindings"][0]["checkId"],
        "code_scan.hardcoded-secret"
    );
    assert!(report["ignoredFindings"][0]["fingerprint"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
}

#[test]
fn sitecmd_web_instructional_examples_require_explicit_suppressions() {
    let project = TempDir::new().expect("temp project");
    let guide_path = "apps/sitecmd-catalog/content/guides/code/security.ts";
    let guide = project.path().join(guide_path);
    std::fs::create_dir_all(guide.parent().expect("guide parent")).expect("guide directory");
    std::fs::write(
        project.path().join("package.json"),
        r#"{ "name": "sitecmd-web-acceptance" }"#,
    )
    .expect("project manifest");
    std::fs::write(
        &guide,
        r#"export const guidance = [
  "Remove rejectUnauthorized: false from production clients.",
  "Replace cors({ origin: true, credentials: true }) with an exact allowlist.",
];"#,
    )
    .expect("instructional guide");
    let args = AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Json,
        fail_on: Some(Severity::High),
        output: None,
        inspect_local_databases: false,
        baseline: None,
    };

    let unsuppressed = run(&args).expect("unsuppressed audit completes");
    assert!(unsuppressed.threshold_failed);

    let sitecmd_dir = project.path().join(".sitecmd");
    std::fs::create_dir_all(&sitecmd_dir).expect("sitecmd config directory");
    std::fs::write(
        sitecmd_dir.join("config.json"),
        format!(
            r#"{{
  "version": 1,
  "url": "https://sitecmd.com",
  "name": "SiteCMD Web acceptance",
  "code_scan": {{
    "suppressions": [
      {{
        "match": {{
          "rule": "code_scan.tls-verification-disabled",
          "path": "{guide_path}"
        }},
        "reason": "The catalog names this insecure setting so users can remove it; the text is not executable."
      }},
      {{
        "match": {{
          "rule": "code_scan.cors-origin-reflection",
          "path": "{guide_path}"
        }},
        "reason": "The catalog names this insecure setting so users can replace it; the text is not executable."
      }}
    ]
  }}
}}"#
        ),
    )
    .expect("suppression config");

    let outcome = run(&args).expect("suppressed audit completes");
    let report: serde_json::Value =
        serde_json::from_str(&outcome.rendered).expect("valid report json");
    assert!(!outcome.threshold_failed);
    assert_eq!(report["issueCount"], 0);
    assert_eq!(report["ignoredCount"], 2);
    assert_eq!(report["staleSuppressionCount"], 0);
    assert_eq!(report["ignoredFindings"].as_array().map(Vec::len), Some(2));
}

#[test]
fn redacts_detected_credentials_from_every_report_format() {
    let raw_token = fixture_token();
    let project = vulnerable_project();

    for format in [
        CodeScanReportFormat::Summary,
        CodeScanReportFormat::Json,
        CodeScanReportFormat::Markdown,
        CodeScanReportFormat::Review,
        CodeScanReportFormat::Github,
        CodeScanReportFormat::Sarif,
    ] {
        let args = AuditArgs {
            project_path: project.path().to_path_buf(),
            format,
            fail_on: None,
            output: None,
            inspect_local_databases: false,
            baseline: None,
        };

        let outcome = run(&args).expect("audit completes");
        assert!(
            !outcome.rendered.contains(&raw_token),
            "{format:?} output exposed the detected credential"
        );
    }
}

#[test]
fn writes_the_requested_report_file() {
    let project = vulnerable_project();
    let output = project.path().join("reports/sitecmd-review.md");
    let args = AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Review,
        fail_on: None,
        output: Some(output.clone()),
        inspect_local_databases: false,
        baseline: None,
    };

    let outcome = run(&args).expect("audit completes");
    assert_eq!(
        std::fs::read_to_string(output).expect("report file"),
        outcome.rendered
    );
}

#[test]
fn baseline_from_a_previous_json_report_keeps_old_findings_from_failing() {
    let project = vulnerable_project();
    let first = run(&AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Json,
        fail_on: Some(Severity::High),
        output: None,
        inspect_local_databases: false,
        baseline: None,
    })
    .expect("first audit");
    assert!(first.threshold_failed);
    let baseline_path = project.path().join("sitecmd-baseline.json");
    std::fs::write(&baseline_path, &first.rendered).expect("baseline file");

    let second = run(&AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Json,
        fail_on: Some(Severity::High),
        output: None,
        inspect_local_databases: false,
        baseline: Some(baseline_path),
    })
    .expect("baselined audit");
    let report: serde_json::Value = serde_json::from_str(&second.rendered).expect("json");
    assert!(
        !second.threshold_failed,
        "known findings must not fail forever"
    );
    assert_eq!(report["baselineCount"], 1);
    assert_eq!(report["issues"][0]["inBaseline"], true);
}

#[test]
fn sarif_output_lists_rules_results_locations_and_fingerprints() {
    let project = vulnerable_project();
    let outcome = run(&AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Sarif,
        fail_on: None,
        output: None,
        inspect_local_databases: false,
        baseline: None,
    })
    .expect("sarif audit");
    let sarif: serde_json::Value = serde_json::from_str(&outcome.rendered).expect("json");
    assert_eq!(sarif["version"], "2.1.0");
    let run = &sarif["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "SiteCMD Code Scan");
    let rule = &run["tool"]["driver"]["rules"][0];
    assert_eq!(rule["id"], "code_scan.hardcoded-secret");
    let result = &run["results"][0];
    assert_eq!(result["ruleId"], "code_scan.hardcoded-secret");
    assert_eq!(result["level"], "error");
    assert_eq!(
        result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/keys.js"
    );
    assert_eq!(
        result["locations"][0]["physicalLocation"]["region"]["startLine"],
        1
    );
    assert!(result["partialFingerprints"]["sitecmd/v1"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(result["baselineState"], "new");
    assert!(
        !outcome.rendered.contains(&fixture_token()),
        "SARIF must not leak the credential"
    );
}

#[test]
fn sarif_carries_suppressed_findings_as_suppressed_results() {
    let project = vulnerable_project();
    let sitecmd_dir = project.path().join(".sitecmd");
    std::fs::create_dir_all(&sitecmd_dir).expect("config dir");
    std::fs::write(
        sitecmd_dir.join("config.json"),
        r#"{"version":1,"url":"https://example.com","name":"s","code_scan":{"suppressions":[{"match":{"rule":"code_scan.hardcoded-secret","path":"src/keys.js"},"reason":"Inert fixture."}]}}"#,
    )
    .expect("config");
    let outcome = run(&AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Sarif,
        fail_on: Some(Severity::High),
        output: None,
        inspect_local_databases: false,
        baseline: None,
    })
    .expect("sarif audit");
    let sarif: serde_json::Value = serde_json::from_str(&outcome.rendered).expect("json");
    assert!(!outcome.threshold_failed);
    let results = sarif["runs"][0]["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["suppressions"][0]["kind"], "external");
    assert_eq!(
        results[0]["suppressions"][0]["justification"],
        "Inert fixture."
    );
}

#[test]
fn the_json_report_omits_the_generating_machines_checkout_path() {
    let project = vulnerable_project();
    let project_dir = project.path().to_string_lossy().into_owned();
    let args = AuditArgs {
        project_path: project.path().to_path_buf(),
        format: CodeScanReportFormat::Json,
        fail_on: None,
        output: None,
        inspect_local_databases: false,
        baseline: None,
    };

    let active = run(&args).expect("audit completes");
    let report: serde_json::Value =
        serde_json::from_str(&active.rendered).expect("valid report json");
    assert_eq!(report["issues"][0]["relativePath"], "src/keys.js");
    assert!(
        report["issues"][0].get("absolutePath").is_none(),
        "{}",
        active.rendered
    );
    assert!(
        !active.rendered.contains(&project_dir),
        "a committed baseline would publish {project_dir}"
    );

    let sitecmd_dir = project.path().join(".sitecmd");
    std::fs::create_dir_all(&sitecmd_dir).expect("sitecmd config directory");
    std::fs::write(
        sitecmd_dir.join("config.json"),
        r#"{
  "version": 1,
  "url": "https://example.com",
  "name": "path disclosure fixture",
  "code_scan": {
    "suppressions": [
      {
        "match": {
          "rule": "code_scan.hardcoded-secret",
          "path": "src/keys.js"
        },
        "reason": "The credential-shaped value is an inert scanner fixture."
      }
    ]
  }
}"#,
    )
    .expect("suppression config");

    let suppressed = run(&args).expect("suppressed audit completes");
    let suppressed_report: serde_json::Value =
        serde_json::from_str(&suppressed.rendered).expect("valid report json");
    assert_eq!(suppressed_report["ignoredCount"], 1);
    assert!(
        suppressed_report["ignoredFindings"][0]
            .get("absolutePath")
            .is_none(),
        "{}",
        suppressed.rendered
    );
    assert!(
        !suppressed.rendered.contains(&project_dir),
        "a committed baseline would publish {project_dir}"
    );
}
