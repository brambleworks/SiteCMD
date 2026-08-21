//! Jira provider - create and resolve issues via the Jira REST API v3.

use crate::checks::{CheckResult, ScanCategory, Severity};
use crate::integrations::issue_tracker::{ExternalTicket, IssueContext};
use crate::integrations::validation::{
    jira_email_domain, normalize_jira_cloud_host, normalize_jira_email, normalize_jira_issue_type,
    normalize_jira_project_key, validate_jira_issue_key,
};

#[tracing::instrument(skip(severity))]
pub(crate) fn severity_str(severity: &Severity) -> &'static str {
    severity.as_str()
}

#[tracing::instrument(skip(category))]
pub(crate) fn category_str(category: &ScanCategory) -> &'static str {
    category.as_str()
}

/// Build a plain-text description for the Jira issue body.
#[tracing::instrument(skip(issue, context))]
pub fn format_jira_description(issue: &CheckResult, context: &IssueContext) -> String {
    let severity = severity_str(&issue.severity);
    let category = category_str(&issue.category);

    let mut text = format!(
        "Severity: {severity}\n\
         Category: {category}\n\
         Estimated Impact: {impact} points\n\
         Site: {site_url}\n\
         Detected On: {scan_timestamp}\n\n\
         {description}",
        severity = severity,
        category = category,
        impact = context.estimated_impact,
        site_url = context.site_url,
        scan_timestamp = context.scan_timestamp,
        description = issue.description,
    );

    if let Some(manual_fix) = &issue.manual_fix {
        text.push_str(&format!("\n\nHow to Fix:\n{}", manual_fix));
    }

    if let Some(fix_prompt) = &issue.fix_prompt {
        text.push_str(&format!("\n\nFix Prompt:\n{}", fix_prompt));
    }

    text.push_str("\n\nCreated by SiteCMD - auto-resolves when the issue passes on rescan.");
    text
}

/// Convert plain-text paragraphs and line breaks to minimal ADF.
#[tracing::instrument(skip(text), fields(text_len = text.len()))]
pub(crate) fn to_adf(text: &str) -> serde_json::Value {
    let content: Vec<serde_json::Value> = text
        .split("\n\n")
        .map(|para| {
            let inline: Vec<serde_json::Value> = para
                .split('\n')
                .enumerate()
                .flat_map(|(i, line)| {
                    let mut nodes: Vec<serde_json::Value> = Vec::new();
                    if i > 0 {
                        nodes.push(serde_json::json!({ "type": "hardBreak" }));
                    }
                    nodes.push(serde_json::json!({
                        "type": "text",
                        "text": line,
                    }));
                    nodes
                })
                .collect();

            serde_json::json!({
                "type": "paragraph",
                "content": inline,
            })
        })
        .collect();

    serde_json::json!({
        "version": 1,
        "type": "doc",
        "content": content,
    })
}

#[tracing::instrument(skip(email, api_token), fields(email_domain = %jira_email_domain(email)))]
pub(crate) fn basic_auth(email: &str, api_token: &str) -> String {
    use base64::Engine as _;
    let credentials = format!("{}:{}", email, api_token);
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(credentials)
    )
}

/// Build the JSON payload Jira's POST /rest/api/3/issue endpoint expects.
/// Pure; tested directly so the labels + ADF body are stable.
#[tracing::instrument(skip(issue, description_adf), fields(project_key = %project_key, issue_type = %issue_type))]
pub(crate) fn build_create_payload(
    project_key: &str,
    issue_type: &str,
    issue: &CheckResult,
    description_adf: serde_json::Value,
) -> serde_json::Value {
    let summary = format!("[SiteCMD] {}", issue.title);
    let sev = severity_str(&issue.severity);
    let cat = category_str(&issue.category);
    let labels = vec!["sitecmd".to_string(), sev.to_string(), cat.to_string()];

    serde_json::json!({
        "fields": {
            "project": { "key": project_key },
            "summary": summary,
            "description": description_adf,
            "issuetype": { "name": issue_type },
            "labels": labels,
        }
    })
}

/// Pull the issue key out of a Jira create-issue response. Returns Err with
/// a stable message when the field is missing - surfaces directly to the UI.
#[tracing::instrument(skip(response))]
pub(crate) fn extract_issue_key(response: &serde_json::Value) -> Result<String, &'static str> {
    response["key"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or("Jira response missing issue key")
}

/// Pick the first transition whose name matches Done / Resolved / Closed.
/// Returns None when none is available (e.g. the workflow is locked or the
/// issue is already in a terminal state).
#[tracing::instrument(skip(transitions))]
pub(crate) fn find_done_transition_id(transitions: &serde_json::Value) -> Option<String> {
    let target_names = ["Done", "Resolved", "Closed"];
    transitions["transitions"]
        .as_array()?
        .iter()
        .find(|t| {
            t["name"]
                .as_str()
                .map(|n| target_names.contains(&n))
                .unwrap_or(false)
        })
        .and_then(|t| t["id"].as_str())
        .map(String::from)
}

/// Create a Jira issue for a SiteCMD finding.
#[tracing::instrument(skip(issue, context, api_token, instance_url, email, project_key, issue_type), fields(email_domain = %jira_email_domain(email)))]
pub async fn create_jira_issue(
    instance_url: &str,
    email: &str,
    api_token: &str,
    project_key: &str,
    issue_type: &str,
    issue: &CheckResult,
    context: &IssueContext,
) -> Result<ExternalTicket, String> {
    let instance_url = normalize_jira_cloud_host(instance_url)?;
    let email = normalize_jira_email(email)?;
    let project_key = normalize_jira_project_key(project_key)?;
    let issue_type = normalize_jira_issue_type(issue_type)?;
    let client = crate::http_client::credentialed_service_client();
    let plain_description = format_jira_description(issue, context);
    let description_adf = to_adf(&plain_description);
    let payload = build_create_payload(&project_key, &issue_type, issue, description_adf);

    let url = format!("https://{}/rest/api/3/issue", instance_url);
    let resp = client
        .post(&url)
        .header("Authorization", basic_auth(&email, api_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Jira create issue error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!("Jira create issue returned {}", status));
    }

    let created: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Jira create issue parse error: {}", e))?;
    let key = validate_jira_issue_key(extract_issue_key(&created).map_err(String::from)?.as_str())?;
    let external_url = format!("https://{}/browse/{}", instance_url, key);

    Ok(ExternalTicket {
        external_id: key,
        external_url,
    })
}

/// Add a resolution comment to a Jira issue, then transition it to Done/Resolved/Closed.
#[tracing::instrument(skip(api_token, comment, instance_url, email, issue_key), fields(email_domain = %jira_email_domain(email)))]
pub async fn resolve_jira_issue(
    instance_url: &str,
    email: &str,
    api_token: &str,
    issue_key: &str,
    comment: &str,
) -> Result<(), String> {
    let instance_url = normalize_jira_cloud_host(instance_url)?;
    let email = normalize_jira_email(email)?;
    let issue_key = validate_jira_issue_key(issue_key)?;
    let client = crate::http_client::credentialed_service_client();
    let auth = basic_auth(&email, api_token);

    // Post the resolution comment (ADF)
    let comment_body = to_adf(comment);
    let comment_url = format!(
        "https://{}/rest/api/3/issue/{}/comment",
        instance_url, issue_key
    );
    let comment_payload = serde_json::json!({ "body": comment_body });

    let resp = client
        .post(&comment_url)
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&comment_payload)
        .send()
        .await
        .map_err(|e| format!("Jira comment error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Jira comment returned {}", resp.status()));
    }

    let transitions_url = format!(
        "https://{}/rest/api/3/issue/{}/transitions",
        instance_url, issue_key
    );
    let resp = client
        .get(&transitions_url)
        .header("Authorization", &auth)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Jira transitions fetch error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Jira transitions returned {}", resp.status()));
    }

    let transitions: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Jira transitions parse error: {}", e))?;

    let transition_id = find_done_transition_id(&transitions)
        .ok_or("No Done/Resolved/Closed transition found for Jira issue")?;

    let do_transition_url = format!(
        "https://{}/rest/api/3/issue/{}/transitions",
        instance_url, issue_key
    );
    let transition_payload = serde_json::json!({
        "transition": { "id": transition_id }
    });

    let resp = client
        .post(&do_transition_url)
        .header("Authorization", &auth)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&transition_payload)
        .send()
        .await
        .map_err(|e| format!("Jira transition error: {}", e))?;

    // 204 No Content is the success response for transitions
    if !resp.status().is_success() {
        return Err(format!("Jira transition returned {}", resp.status()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    fn fixture_issue() -> CheckResult {
        CheckResult {
            check_id: "security.csp".to_string(),
            category: ScanCategory::Security,
            title: "Missing Content-Security-Policy header".to_string(),
            description: "Set a CSP to block injected scripts.".to_string(),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: Some("Add `Content-Security-Policy: default-src 'self'`".into()),
            manual_fix: Some("Set the CSP header in nginx.conf".into()),
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    fn fixture_context() -> IssueContext {
        IssueContext {
            project_name: "Acme".into(),
            site_url: "https://example.com".into(),
            scan_timestamp: "2026-04-19T10:00:00Z".into(),
            estimated_impact: 12,
        }
    }

    #[test]
    fn severity_str_covers_all_levels() {
        assert_eq!(severity_str(&Severity::Critical), "critical");
        assert_eq!(severity_str(&Severity::High), "high");
        assert_eq!(severity_str(&Severity::Medium), "medium");
        assert_eq!(severity_str(&Severity::Low), "low");
    }

    #[test]
    fn category_str_covers_all_scan_categories() {
        assert_eq!(category_str(&ScanCategory::Security), "security");
        assert_eq!(category_str(&ScanCategory::Performance), "performance");
        assert_eq!(category_str(&ScanCategory::Seo), "seo");
        assert_eq!(category_str(&ScanCategory::Accessibility), "accessibility");
        assert_eq!(category_str(&ScanCategory::Compliance), "compliance");
        assert_eq!(category_str(&ScanCategory::Config), "config");
        assert_eq!(category_str(&ScanCategory::Polish), "polish");
    }

    #[test]
    fn format_jira_description_includes_metadata_block() {
        let body = format_jira_description(&fixture_issue(), &fixture_context());
        assert!(body.contains("Severity: high"));
        assert!(body.contains("Category: security"));
        assert!(body.contains("Estimated Impact: 12 points"));
        assert!(body.contains("Site: https://example.com"));
        assert!(body.contains("Detected On: 2026-04-19T10:00:00Z"));
        assert!(body.contains("Set a CSP to block injected scripts."));
    }

    #[test]
    fn format_jira_description_appends_manual_fix_when_present() {
        let body = format_jira_description(&fixture_issue(), &fixture_context());
        assert!(body.contains("How to Fix:"));
        assert!(body.contains("Set the CSP header in nginx.conf"));
    }

    #[test]
    fn format_jira_description_appends_fix_prompt_when_present() {
        let body = format_jira_description(&fixture_issue(), &fixture_context());
        assert!(body.contains("Fix Prompt:"));
        assert!(body.contains("Add `Content-Security-Policy"));
    }

    #[test]
    fn format_jira_description_skips_optional_sections_when_absent() {
        // Issue with only the bare-minimum fields shouldn't render empty headers.
        let mut issue = fixture_issue();
        issue.manual_fix = None;
        issue.fix_prompt = None;
        let body = format_jira_description(&issue, &fixture_context());
        assert!(!body.contains("How to Fix:"));
        assert!(!body.contains("Fix Prompt:"));
    }

    #[test]
    fn format_jira_description_includes_attribution_footer() {
        let body = format_jira_description(&fixture_issue(), &fixture_context());
        assert!(body.contains("Created by SiteCMD"));
    }

    #[test]
    fn to_adf_wraps_single_line_in_one_paragraph() {
        let adf = to_adf("just one line");
        assert_eq!(adf["version"], 1);
        assert_eq!(adf["type"], "doc");
        let content = adf["content"].as_array().expect("content");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "paragraph");
        let inline = content[0]["content"].as_array().expect("inline");
        assert_eq!(inline.len(), 1);
        assert_eq!(inline[0]["type"], "text");
        assert_eq!(inline[0]["text"], "just one line");
    }

    #[test]
    fn to_adf_splits_paragraphs_on_double_newline() {
        let adf = to_adf("first paragraph\n\nsecond paragraph\n\nthird");
        let content = adf["content"].as_array().expect("content");
        assert_eq!(
            content.len(),
            3,
            "expected 3 paragraphs, got {}",
            content.len()
        );
        assert_eq!(content[0]["content"][0]["text"], "first paragraph");
        assert_eq!(content[1]["content"][0]["text"], "second paragraph");
        assert_eq!(content[2]["content"][0]["text"], "third");
    }

    #[test]
    fn to_adf_inserts_hard_break_between_lines_in_one_paragraph() {
        // Single \n inside a paragraph becomes a hardBreak ADF node so the
        // Jira UI shows a line break without splitting the paragraph.
        let adf = to_adf("line one\nline two");
        let content = adf["content"].as_array().expect("content");
        assert_eq!(content.len(), 1, "single paragraph");
        let inline = content[0]["content"].as_array().expect("inline");
        // Expected: [text, hardBreak, text]
        assert_eq!(inline.len(), 3);
        assert_eq!(inline[0]["type"], "text");
        assert_eq!(inline[0]["text"], "line one");
        assert_eq!(inline[1]["type"], "hardBreak");
        assert_eq!(inline[2]["type"], "text");
        assert_eq!(inline[2]["text"], "line two");
    }

    #[test]
    fn basic_auth_base64_encodes_email_and_token() {
        // base64("user@example.com:secret") = "dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ="
        assert_eq!(
            basic_auth("user@example.com", "secret"),
            "Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ=",
        );
    }

    #[test]
    fn basic_auth_handles_empty_inputs() {
        // base64(":") = "Og=="
        assert_eq!(basic_auth("", ""), "Basic Og==");
    }

    #[test]
    fn build_create_payload_produces_jira_field_shape() {
        let payload = build_create_payload(
            "WEB",
            "Bug",
            &fixture_issue(),
            serde_json::json!({"placeholder": "adf"}),
        );
        let fields = &payload["fields"];
        assert_eq!(fields["project"]["key"], "WEB");
        assert_eq!(fields["issuetype"]["name"], "Bug");
        assert_eq!(
            fields["summary"],
            "[SiteCMD] Missing Content-Security-Policy header"
        );
        assert_eq!(fields["description"]["placeholder"], "adf");
    }

    #[test]
    fn build_create_payload_labels_issue_with_severity_and_category() {
        // Labels surface in Jira - make sure they reflect the issue's
        // severity and category for downstream filtering.
        let payload = build_create_payload("WEB", "Bug", &fixture_issue(), serde_json::json!(null));
        let labels: Vec<&str> = payload["fields"]["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(labels, vec!["sitecmd", "high", "security"]);
    }

    #[test]
    fn extract_issue_key_returns_key_string() {
        let key = extract_issue_key(&serde_json::json!({"key": "WEB-42"})).unwrap();
        assert_eq!(key, "WEB-42");
    }

    #[test]
    fn extract_issue_key_errors_when_missing() {
        let err = extract_issue_key(&serde_json::json!({"id": "12345"})).unwrap_err();
        assert_eq!(err, "Jira response missing issue key");
    }

    #[test]
    fn extract_issue_key_errors_when_not_a_string() {
        // Defensive: a numeric or array key shouldn't slip through.
        let err = extract_issue_key(&serde_json::json!({"key": 42})).unwrap_err();
        assert_eq!(err, "Jira response missing issue key");
    }

    #[test]
    fn find_done_transition_id_picks_done_first() {
        let transitions = serde_json::json!({
            "transitions": [
                {"id": "11", "name": "In Progress"},
                {"id": "31", "name": "Done"},
                {"id": "41", "name": "Resolved"},
            ]
        });
        assert_eq!(find_done_transition_id(&transitions).as_deref(), Some("31"));
    }

    #[test]
    fn find_done_transition_id_falls_through_to_resolved_when_done_missing() {
        // Some workflows don't have "Done" - must accept Resolved or Closed.
        let transitions = serde_json::json!({
            "transitions": [
                {"id": "11", "name": "In Progress"},
                {"id": "41", "name": "Resolved"},
            ]
        });
        assert_eq!(find_done_transition_id(&transitions).as_deref(), Some("41"));
    }

    #[test]
    fn find_done_transition_id_accepts_closed() {
        let transitions = serde_json::json!({
            "transitions": [
                {"id": "51", "name": "Closed"},
            ]
        });
        assert_eq!(find_done_transition_id(&transitions).as_deref(), Some("51"));
    }

    #[test]
    fn find_done_transition_id_returns_none_when_no_terminal_transition() {
        // Locked workflow scenario - only "In Progress" is available.
        let transitions = serde_json::json!({
            "transitions": [
                {"id": "11", "name": "In Progress"},
                {"id": "21", "name": "In Review"},
            ]
        });
        assert!(find_done_transition_id(&transitions).is_none());
    }

    #[test]
    fn find_done_transition_id_returns_none_when_transitions_field_absent() {
        assert!(find_done_transition_id(&serde_json::json!({})).is_none());
        assert!(find_done_transition_id(&serde_json::json!({"transitions": null})).is_none());
    }

    #[test]
    fn find_done_transition_id_skips_unnamed_transitions() {
        // A transition without a "name" field shouldn't poison the find loop.
        let transitions = serde_json::json!({
            "transitions": [
                {"id": "11"}, // missing name
                {"id": "31", "name": "Done"},
            ]
        });
        assert_eq!(find_done_transition_id(&transitions).as_deref(), Some("31"));
    }
}
