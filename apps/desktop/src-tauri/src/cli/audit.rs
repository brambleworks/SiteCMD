use crate::checks::Severity;
use crate::cli::audit_suppressions::{
    apply_project_suppressions, issue_fingerprint, IgnoredFinding, SuppressedAudit,
    SuppressionState,
};
use crate::core::code_scan::{
    audit_project_with_options, format_report, has_issue_at_or_above, CodeIssueView,
    CodeScanOptions, CodeScanReportFormat, CodeScanReportView,
};
use std::fmt::Write as _;
use std::path::PathBuf;

pub const HELP: &str = concat!(
    "SiteCMD audit - Run Code Scan against a source checkout\n\n",
    "Usage:\n  sitecmd audit <path> [options]\n\n",
    "Reviewed findings can be suppressed through .sitecmd/config.json. JSON reports include occurrence fingerprints and suppression status.\n\n",
    "Options:\n",
    "  --format <FORMAT>      summary, json, markdown, review, or github (default: summary)\n",
    "  --fail-on <SEVERITY>  Exit 1 when a critical, high, medium, or low finding meets the threshold\n",
    "  --output <PATH>        Write the report to a file instead of stdout\n",
    "  --inspect-local-databases\n",
    "                         Opt in to local dotenv target discovery and read-only inspection of project SQLite and loopback PostgreSQL schemas\n",
    "  --help, -h             Show this help\n\n",
    "Exit codes:\n",
    "  0  Audit completed and no configured threshold was met\n",
    "  1  A finding met or exceeded --fail-on\n",
    "  2  Usage, scan, or output error\n\n",
    "Examples:\n",
    "  sitecmd audit .\n",
    "  sitecmd audit . --format review --output sitecmd-review.md\n",
    "  sitecmd audit . --inspect-local-databases\n",
    "  sitecmd audit . --format github --fail-on high\n",
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditArgs {
    pub project_path: PathBuf,
    pub format: CodeScanReportFormat,
    pub fail_on: Option<Severity>,
    pub output: Option<PathBuf>,
    pub inspect_local_databases: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditOutcome {
    pub rendered: String,
    pub threshold_failed: bool,
}

pub fn help_requested(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<AuditArgs, String> {
    let mut args = args.into_iter();
    let mut project_path = None;
    let mut format = CodeScanReportFormat::Summary;
    let mut fail_on = None;
    let mut output = None;
    let mut inspect_local_databases = false;

    while let Some(token) = args.next() {
        match token.as_str() {
            "--format" => {
                let value = next_value(&mut args, "--format")?;
                format = match value.as_str() {
                    "summary" => CodeScanReportFormat::Summary,
                    "json" => CodeScanReportFormat::Json,
                    "markdown" | "md" => CodeScanReportFormat::Markdown,
                    "review" => CodeScanReportFormat::Review,
                    "github" | "gh" => CodeScanReportFormat::Github,
                    _ => {
                        return Err(format!(
                            "Unknown format: {value}. Use: summary, json, markdown, review, github"
                        ))
                    }
                };
            }
            "--fail-on" => {
                let value = next_value(&mut args, "--fail-on")?;
                fail_on = Some(value.parse::<Severity>()?);
            }
            "--output" => {
                output = Some(PathBuf::from(next_value(&mut args, "--output")?));
            }
            "--inspect-local-databases" => {
                inspect_local_databases = true;
            }
            "--help" | "-h" => return Err("help requested".into()),
            option if option.starts_with('-') => {
                return Err(format!("Unknown option: {option}"));
            }
            value => {
                if project_path.is_some() {
                    return Err(format!("Unexpected extra path argument: {value}"));
                }
                project_path = Some(PathBuf::from(value));
            }
        }
    }

    Ok(AuditArgs {
        project_path: project_path.ok_or_else(|| "Missing project path".to_string())?,
        format,
        fail_on,
        output,
        inspect_local_databases,
    })
}

fn next_value(args: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("Missing value for {option}"))
}

pub fn run(args: &AuditArgs) -> Result<AuditOutcome, String> {
    let report = audit_project_with_options(
        &args.project_path,
        CodeScanOptions {
            inspect_local_databases: args.inspect_local_databases,
        },
    )
    .map_err(|error| format!("Code Scan audit failed: {error}"))?;
    let project_root = std::fs::canonicalize(&args.project_path).map_err(|error| {
        format!(
            "Could not resolve Code Scan project path {}: {error}",
            args.project_path.display()
        )
    })?;
    let audit = apply_project_suppressions(&project_root, report, chrono::Utc::now().date_naive())?;
    let rendered = format_audit_report(&audit, &args.project_path, args.format)
        .map_err(|error| format!("Could not render Code Scan report: {error}"))?;

    if let Some(output_path) = &args.output {
        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "Could not create output directory {}: {error}",
                        parent.display()
                    )
                })?;
            }
        }
        std::fs::write(output_path, rendered.as_bytes()).map_err(|error| {
            format!(
                "Could not write report to {}: {error}",
                output_path.display()
            )
        })?;
    }

    Ok(AuditOutcome {
        threshold_failed: args
            .fail_on
            .is_some_and(|severity| has_issue_at_or_above(&audit.report, severity)),
        rendered,
    })
}

fn format_audit_report(
    audit: &SuppressedAudit,
    project_path: &std::path::Path,
    format: CodeScanReportFormat,
) -> Result<String, String> {
    if format == CodeScanReportFormat::Json {
        return format_audit_json(audit);
    }

    let mut rendered = format_report(&audit.report, project_path, format)?;
    if audit.suppressions.is_empty() {
        return Ok(rendered);
    }

    match format {
        CodeScanReportFormat::Summary => {
            let _ = writeln!(
                rendered,
                "\nSuppressions: {} ignored | {} stale or expired",
                audit.ignored_findings.len(),
                audit.stale_suppression_count()
            );
            append_text_suppression_details(&mut rendered, audit);
        }
        CodeScanReportFormat::Markdown | CodeScanReportFormat::Review => {
            let _ = writeln!(rendered, "\n## Suppressions");
            let _ = writeln!(
                rendered,
                "\n- Ignored findings: `{}`",
                audit.ignored_findings.len()
            );
            let _ = writeln!(
                rendered,
                "- Stale or expired entries: `{}`",
                audit.stale_suppression_count()
            );
            append_text_suppression_details(&mut rendered, audit);
        }
        CodeScanReportFormat::Github => {
            let _ = writeln!(
                rendered,
                "::notice title=SiteCMD suppressions::{} finding(s) ignored; {} stale or expired suppression(s)",
                audit.ignored_findings.len(),
                audit.stale_suppression_count()
            );
        }
        CodeScanReportFormat::Json => unreachable!("JSON returned above"),
    }
    Ok(rendered)
}

fn format_audit_json(audit: &SuppressedAudit) -> Result<String, String> {
    let mut value = serde_json::to_value(CodeScanReportView::from(&audit.report))
        .map_err(|error| format!("Could not serialize code scan report: {error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "Could not serialize Code Scan report as an object".to_string())?;

    if let Some(issues) = object
        .get_mut("issues")
        .and_then(serde_json::Value::as_array_mut)
    {
        for (value, issue) in issues.iter_mut().zip(&audit.report.issues) {
            if let Some(issue_object) = value.as_object_mut() {
                issue_object.insert(
                    "fingerprint".into(),
                    serde_json::Value::String(issue_fingerprint(issue)),
                );
            }
        }
    }

    object.insert(
        "ignoredCount".into(),
        serde_json::json!(audit.ignored_findings.len()),
    );
    object.insert(
        "staleSuppressionCount".into(),
        serde_json::json!(audit.stale_suppression_count()),
    );
    object.insert(
        "ignoredFindings".into(),
        serde_json::Value::Array(
            audit
                .ignored_findings
                .iter()
                .map(ignored_finding_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    object.insert(
        "suppressions".into(),
        serde_json::to_value(&audit.suppressions)
            .map_err(|error| format!("Could not serialize Code Scan suppressions: {error}"))?,
    );
    serde_json::to_string_pretty(&value)
        .map_err(|error| format!("Could not serialize code scan report: {error}"))
}

fn ignored_finding_json(finding: &IgnoredFinding) -> Result<serde_json::Value, String> {
    let mut value = serde_json::to_value(CodeIssueView::from(&finding.issue))
        .map_err(|error| format!("Could not serialize ignored Code Scan finding: {error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "Could not serialize ignored Code Scan finding as an object".to_string())?;
    object.insert(
        "fingerprint".into(),
        serde_json::Value::String(finding.fingerprint.clone()),
    );
    object.insert(
        "reason".into(),
        serde_json::Value::String(finding.reason.clone()),
    );
    object.insert(
        "expires".into(),
        finding
            .expires
            .clone()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "suppressionIndex".into(),
        serde_json::json!(finding.suppression_index + 1),
    );
    Ok(value)
}

fn append_text_suppression_details(rendered: &mut String, audit: &SuppressedAudit) {
    for finding in &audit.ignored_findings {
        let _ = writeln!(
            rendered,
            "- Ignored `{}` in `{}` ({}) - {}",
            finding.issue.check_id,
            finding.issue.relative_path,
            finding.fingerprint,
            finding.reason
        );
    }
    for (index, suppression) in audit.suppressions.iter().enumerate() {
        if suppression.state == SuppressionState::Active {
            continue;
        }
        let _ = writeln!(
            rendered,
            "- Suppression {} is {:?}: {}",
            index + 1,
            suppression.state,
            suppression.reason
        );
    }
}

#[cfg(test)]
mod tests {
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
        ] {
            let args = AuditArgs {
                project_path: project.path().to_path_buf(),
                format,
                fail_on: None,
                output: None,
                inspect_local_databases: false,
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
        };

        let outcome = run(&args).expect("audit completes");
        assert_eq!(
            std::fs::read_to_string(output).expect("report file"),
            outcome.rendered
        );
    }
}
