use super::{
    code_issue_domain, code_scan_domain_label, CodeIssue, CodeScanReport, CodeScanReportFormat,
    CodeScanReportView,
};
use crate::checks::Severity;
use std::fmt::Write as _;
use std::path::Path;
#[tracing::instrument(skip(report, project_path, format), fields(project_path_len = project_path.to_string_lossy().len()))]
pub fn format_report(
    report: &CodeScanReport,
    project_path: &Path,
    format: CodeScanReportFormat,
) -> Result<String, String> {
    match format {
        CodeScanReportFormat::Summary => Ok(format_report_summary(report, project_path)),
        CodeScanReportFormat::Json => {
            serde_json::to_string_pretty(&CodeScanReportView::from(report))
                .map_err(|error| format!("Could not serialize code scan report: {}", error))
        }
        CodeScanReportFormat::Markdown => Ok(format_report_markdown(report, project_path)),
        CodeScanReportFormat::Review => Ok(format_report_review(report, project_path)),
        CodeScanReportFormat::Github => Ok(format_report_github(report)),
    }
}

#[tracing::instrument(skip(report, minimum))]
pub fn has_issue_at_or_above(report: &CodeScanReport, minimum: Severity) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.severity.sort_rank() <= minimum.sort_rank())
}

fn scan_report_labels() -> (&'static str, &'static str, &'static str, &'static str) {
    (
        "SiteCMD Code Scan",
        "code issues",
        "SiteCMD Code Scan Report",
        "SiteCMD Code Scan Review",
    )
}

fn format_report_summary(report: &CodeScanReport, project_path: &Path) -> String {
    let (summary_title, empty_label, _, _) = scan_report_labels();
    let mut output = String::new();
    let _ = writeln!(output, "{} - {}", summary_title, project_path.display());
    let _ = writeln!(output, "Checked: {}", report.checked_at);
    if let Some(framework) = &report.framework {
        let _ = writeln!(output, "Framework: {}", framework);
    }
    let _ = writeln!(
        output,
        "Issues: {} total | critical {} | high {} | medium {} | low {}",
        report.issue_count,
        report.critical_count,
        report.high_count,
        report.medium_count,
        report.low_count
    );

    if report.issues.is_empty() {
        let _ = writeln!(output, "\nNo {} detected.", empty_label);
        return output;
    }

    for severity in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
    ] {
        let issues = issues_for_severity(report, &severity);
        if issues.is_empty() {
            continue;
        }
        let _ = writeln!(output, "\n{} ({})", severity.label(), issues.len());
        for issue in issues {
            let domain = code_scan_domain_label(code_issue_domain(issue));
            let _ = writeln!(
                output,
                "- [{} · {}] {} - {}{}",
                domain,
                issue.category,
                issue.title,
                issue.relative_path,
                issue
                    .line
                    .map(|line| format!(":{}", line))
                    .unwrap_or_default()
            );
        }
    }

    output
}

fn format_report_markdown(report: &CodeScanReport, project_path: &Path) -> String {
    let (_, empty_label, markdown_title, _) = scan_report_labels();
    let mut output = String::new();
    let _ = writeln!(output, "# {}", markdown_title);
    let _ = writeln!(output);
    let _ = writeln!(output, "- Project: `{}`", project_path.display());
    let _ = writeln!(output, "- Checked: `{}`", report.checked_at);
    if let Some(framework) = &report.framework {
        let _ = writeln!(output, "- Framework: `{}`", framework);
    }
    let _ = writeln!(output, "- Total issues: `{}`", report.issue_count);
    let _ = writeln!(output);
    let _ = writeln!(output, "| Severity | Count |");
    let _ = writeln!(output, "| --- | ---: |");
    let _ = writeln!(output, "| Critical | {} |", report.critical_count);
    let _ = writeln!(output, "| High | {} |", report.high_count);
    let _ = writeln!(output, "| Medium | {} |", report.medium_count);
    let _ = writeln!(output, "| Low | {} |", report.low_count);

    if report.issues.is_empty() {
        let _ = writeln!(output, "\nNo {} detected.", empty_label);
        return output;
    }

    for severity in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
    ] {
        let issues = issues_for_severity(report, &severity);
        if issues.is_empty() {
            continue;
        }
        let _ = writeln!(output, "\n## {} ({})", severity.label(), issues.len());
        for issue in issues {
            let domain = code_scan_domain_label(code_issue_domain(issue));
            let _ = writeln!(output, "\n### {}", issue.title);
            let _ = writeln!(output, "- Domain: `{}`", domain);
            let _ = writeln!(output, "- Category: `{}`", issue.category);
            let _ = writeln!(
                output,
                "- File: `{}`{}",
                issue.relative_path,
                issue
                    .line
                    .map(|line| format!(" (line {})", line))
                    .unwrap_or_default()
            );
            if let Some(source_excerpt) = &issue.source_excerpt {
                let _ = writeln!(output, "- Source excerpt:");
                let _ = writeln!(output, "```text");
                let _ = writeln!(output, "{}", source_excerpt);
                let _ = writeln!(output, "```");
            }
            let _ = writeln!(output, "- Why this matters: {}", issue.description);
            if let Some(evidence) = &issue.evidence {
                let _ = writeln!(output, "- Evidence: {}", evidence);
            }
            if let Some(why_now) = &issue.why_now {
                let _ = writeln!(output, "- Why now: {}", why_now);
            }
            if let Some(likely_fix) = &issue.likely_fix {
                let _ = writeln!(output, "- Suggested fix: {}", likely_fix);
            }
            if let Some(verify_hint) = &issue.verify_hint {
                let _ = writeln!(output, "- Verify: {}", verify_hint);
            }
        }
    }

    output
}

fn format_report_review(report: &CodeScanReport, project_path: &Path) -> String {
    let (_, empty_label, _, review_title) = scan_report_labels();
    let mut output = String::new();
    let _ = writeln!(output, "# {}", review_title);
    let _ = writeln!(output);
    let _ = writeln!(output, "- Project: `{}`", project_path.display());
    let _ = writeln!(output, "- Checked: `{}`", report.checked_at);
    if let Some(framework) = &report.framework {
        let _ = writeln!(output, "- Framework: `{}`", framework);
    }
    let _ = writeln!(output, "- Findings: `{}` total", report.issue_count);
    let _ = writeln!(output);
    let _ = writeln!(output, "| Severity | Count |");
    let _ = writeln!(output, "| --- | ---: |");
    let _ = writeln!(output, "| Critical | {} |", report.critical_count);
    let _ = writeln!(output, "| High | {} |", report.high_count);
    let _ = writeln!(output, "| Medium | {} |", report.medium_count);
    let _ = writeln!(output, "| Low | {} |", report.low_count);

    if report.issues.is_empty() {
        let _ = writeln!(output, "\nNo {} detected.", empty_label);
        return output;
    }

    let _ = writeln!(output, "\n## Findings");
    for severity in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
    ] {
        let issues = issues_for_severity(report, &severity);
        if issues.is_empty() {
            continue;
        }
        let _ = writeln!(output, "\n### {} ({})", severity.label(), issues.len());
        for issue in issues {
            let domain = code_scan_domain_label(code_issue_domain(issue));
            let _ = writeln!(
                output,
                "\n#### [{} · {}] {}",
                domain,
                category_label(&issue.category),
                issue.title
            );
            let _ = writeln!(output, "- Location: `{}`", file_location(issue));
            let _ = writeln!(output, "- Why it matters: {}", issue.description);
            if let Some(evidence) = &issue.evidence {
                let _ = writeln!(output, "- Evidence: {}", evidence);
            }
            if let Some(why_now) = &issue.why_now {
                let _ = writeln!(output, "- Why now: {}", why_now);
            }
            if let Some(likely_fix) = &issue.likely_fix {
                let _ = writeln!(output, "- Best first fix: {}", likely_fix);
            }
            if let Some(verify_hint) = &issue.verify_hint {
                let _ = writeln!(output, "- Verify: {}", verify_hint);
            }
            if let Some(source_excerpt) = &issue.source_excerpt {
                let _ = writeln!(output, "- Source excerpt:");
                let _ = writeln!(output, "```text");
                let _ = writeln!(output, "{}", source_excerpt);
                let _ = writeln!(output, "```");
            }
        }
    }

    output
}

fn format_report_github(report: &CodeScanReport) -> String {
    if report.issues.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for issue in &report.issues {
        let level = match issue.severity {
            Severity::Critical | Severity::High => "error",
            Severity::Medium => "warning",
            Severity::Low => "notice",
        };
        let location = file_location(issue);
        let mut message_parts = vec![
            format!("[{}] {}", category_label(&issue.category), issue.title),
            issue.description.clone(),
        ];
        if let Some(evidence) = &issue.evidence {
            message_parts.push(format!("Evidence: {}", evidence));
        }
        if let Some(likely_fix) = &issue.likely_fix {
            message_parts.push(format!("Best first fix: {}", likely_fix));
        }
        if let Some(verify_hint) = &issue.verify_hint {
            message_parts.push(format!("Verify: {}", verify_hint));
        }
        let message = escape_github_data(&message_parts.join(" "));
        let title = escape_github_property(&format!(
            "SiteCMD {} [{} · {}]",
            issue.severity.label(),
            code_scan_domain_label(code_issue_domain(issue)),
            category_label(&issue.category)
        ));
        let file = escape_github_property(&issue.relative_path);
        let line = issue.line.unwrap_or(1);
        let _ = writeln!(
            output,
            "::{} file={},line={},title={}::{}",
            level, file, line, title, message
        );
        let _ = writeln!(
            output,
            "::notice file={},line={},title={}::{}",
            file,
            line,
            escape_github_property("SiteCMD location"),
            escape_github_data(location.as_str())
        );
    }
    output
}

fn issues_for_severity<'a>(report: &'a CodeScanReport, severity: &Severity) -> Vec<&'a CodeIssue> {
    report
        .issues
        .iter()
        .filter(|issue| &issue.severity == severity)
        .collect()
}

fn file_location(issue: &CodeIssue) -> String {
    match issue.line {
        Some(line) => format!("{}:{}", issue.relative_path, line),
        None => issue.relative_path.clone(),
    }
}

fn category_label(category: &str) -> String {
    category
        .split('-')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut label = String::new();
                    label.extend(first.to_uppercase());
                    label.push_str(chars.as_str());
                    label
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape_github_property(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

fn escape_github_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}
