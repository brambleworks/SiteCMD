//! Issue tracker provider trait and shared types.

use crate::checks::{CheckResult, ScanCategory, Severity};

/// Context about the SiteCMD project/scan for ticket creation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct IssueContext {
    pub project_name: String,
    pub site_url: String,
    pub scan_timestamp: String,
    pub estimated_impact: u32,
}

/// Result of creating an external ticket.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalTicket {
    pub external_id: String,
    pub external_url: String,
}

fn severity_str(severity: &Severity) -> &'static str {
    severity.as_str()
}

fn category_str(category: &ScanCategory) -> &'static str {
    category.as_str()
}

/// Format a Markdown issue with remediation context and an AI prompt.
#[tracing::instrument(skip(issue, context))]
pub fn format_issue_body_markdown(issue: &CheckResult, context: &IssueContext) -> String {
    let severity = severity_str(&issue.severity);
    let category = category_str(&issue.category);

    let mut body = format!(
        "## {title}\n\n\
         | Field | Value |\n\
         |---|---|\n\
         | **Severity** | {severity} |\n\
         | **Category** | {category} |\n\
         | **Estimated Impact** | {impact} points |\n\
         | **Site** | {site_url} |\n\
         | **Detected On** | {scan_timestamp} |\n\n\
         ## Description\n\n\
         {description}\n",
        title = issue.title,
        severity = severity,
        category = category,
        impact = context.estimated_impact,
        site_url = context.site_url,
        scan_timestamp = context.scan_timestamp,
        description = issue.description,
    );

    if let Some(manual_fix) = &issue.manual_fix {
        body.push_str(&format!("\n## How to Fix\n\n{}\n", manual_fix));
    }

    if let Some(fix_prompt) = &issue.fix_prompt {
        body.push_str(&format!(
            "\n<details>\n<summary>Fix Prompt</summary>\n\n```\n{}\n```\n</details>\n",
            fix_prompt
        ));
    }

    body.push_str("\n---\n_Created by SiteCMD - auto-resolves when the issue passes on rescan._\n");

    body
}

/// Formats a resolution comment to be posted when an issue is closed.
#[tracing::instrument(fields(check_id = %check_id, scan_timestamp = %scan_timestamp))]
pub fn format_resolution_comment(check_id: &str, scan_timestamp: &str) -> String {
    format!(
        "Verified fixed by SiteCMD rescan on {scan_timestamp}. Check `{check_id}` is now passing."
    )
}
