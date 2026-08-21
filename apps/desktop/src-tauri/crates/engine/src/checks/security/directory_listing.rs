//! Portable directory-listing probe plans and verdicts.
//! The desktop executes bounded probes; this module owns detection and grading.

use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};
use crate::probe::ProbeOutcome;

/// Common high-risk directories to probe for generated indexes.
pub const PROBE_DIRS: &[&str] = &[
    "/images/",
    "/assets/",
    "/css/",
    "/js/",
    "/uploads/",
    "/static/",
    "/media/",
    "/files/",
    // WordPress
    "/wp-content/uploads/",
    "/wp-content/",
    // Common dump / backup / admin / temp / leftover-from-dev paths
    "/admin/",
    "/backup/",
    "/backups/",
    "/storage/",
    "/public/uploads/",
    "/tmp/",
    "/temp/",
    "/old/",
    "/test/",
];

/// Server-generated directory index signatures, excluding ambiguous prose.
const LISTING_PATTERNS: &[&str] = &[
    "index of /",
    "directory listing for",
    "<title>index of",
    "[to parent directory]",
    "directory contents",
];

/// The localhost-preview result: directory browsing on a dev server says
/// nothing about the deployed site.
pub fn localhost_skip_result() -> CheckResult {
    CheckResult {
        check_id: "security.directory_listing".into(),
        category: ScanCategory::Security,
        title: "Directory listing".into(),
        description: "Skipped on localhost preview. Directory browsing often reflects the local static file server rather than your deployed site.".into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({"reason": "localhost_preview_server"})),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Return directories with a successful response that resembles a generated index.
pub fn exposed_directories(outcomes: Vec<(String, ProbeOutcome)>) -> Vec<String> {
    let mut exposed = Vec::new();
    for (dir, outcome) in outcomes {
        let ProbeOutcome::Response(response) = outcome else {
            continue;
        };
        if response.status != 200 {
            continue;
        }
        let Some(body) = response.body else { continue };
        if body_indicates_listing(&body.text) {
            exposed.push(dir);
        }
    }
    exposed
}

/// Grade confirmed directory listings at High severity.
pub fn grade_listing_probes(listing_found: Vec<String>) -> CheckResult {
    if listing_found.is_empty() {
        CheckResult {
            check_id: "security.directory_listing".into(),
            category: ScanCategory::Security,
            title: "Directory listing".into(),
            description: format!(
                "No directory listings were detected across the {} common paths we probed.",
                PROBE_DIRS.len()
            ),
            status: CheckStatus::Pass,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    } else {
        CheckResult {
            check_id: "security.directory_listing".into(),
            category: ScanCategory::Security,
            title: "Directory listing enabled".into(),
            description: format!(
                "Directory listing is enabled on {} path{}: {}.",
                listing_found.len(),
                if listing_found.len() == 1 { "" } else { "s" },
                listing_found.join(", ")
            ),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: Some(
                "Turn off directory browsing for public asset paths at the web server or hosting layer. If a directory must stay public, serve only the specific files you intend to expose.".into(),
            ),
            raw_data: Some(serde_json::json!({
                "directories_exposed": listing_found,
            })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("A browseable directory gives people an instant map of file names and paths you probably did not mean to publish.".into()),
        }
    }
}

/// Whether a 200 probe body looks like a server-generated directory index.
fn body_indicates_listing(body: &str) -> bool {
    let body = body.to_lowercase();
    LISTING_PATTERNS.iter().any(|p| body.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ProbeBody, ProbeResponse};

    #[test]
    fn apache_style_index_page_is_a_listing() {
        let body = r#"<html><head><title>Index of /uploads</title></head>
            <body><h1>Index of /uploads</h1><a href="../">Parent Directory</a></body></html>"#;
        assert!(body_indicates_listing(body));
    }

    #[test]
    fn python_http_server_listing_is_detected() {
        let body = "<html><title>Directory listing for /files/</title><body></body></html>";
        assert!(body_indicates_listing(body));
    }

    #[test]
    fn iis_style_listing_is_detected() {
        let body = "<html><body><pre>[To Parent Directory]<br>invoice-2026.pdf</pre></body></html>";
        assert!(body_indicates_listing(body));
    }

    #[test]
    fn help_page_mentioning_parent_directory_in_prose_is_not_a_listing() {
        let body = r#"<html><head><title>Admin help</title></head>
            <body><h1>Uploading files</h1>
            <p>To move a file up one level, open the parent directory in your file
            manager, then drag the file into the folder above.</p></body></html>"#;
        assert!(!body_indicates_listing(body));
    }

    #[test]
    fn ordinary_page_served_from_a_directory_path_is_not_a_listing() {
        let body = r#"<html><head><title>Gallery</title></head>
            <body><h1>Our work</h1><img src="/images/one.jpg"></body></html>"#;
        assert!(!body_indicates_listing(body));
        assert!(!body_indicates_listing(""));
    }

    #[test]
    fn confirmed_listing_fails_at_high_severity() {
        let result = grade_listing_probes(vec!["/uploads/".into()]);
        assert_eq!(result.status, CheckStatus::Fail);
        assert_eq!(result.severity, Severity::High);
        assert!(result.description.contains("/uploads/"));
    }

    #[test]
    fn only_200_bodies_with_index_signatures_count_as_exposed() {
        let listing = ProbeResponse {
            status: 200,
            final_url: "https://example.com/uploads/".into(),
            content_type: Some("text/html".into()),
            content_length: None,
            headers: Vec::new(),
            body: Some(ProbeBody {
                text: "<h1>Index of /uploads</h1>".into(),
                bytes: 26,
                utf8_valid: true,
            }),
        };
        let forbidden = ProbeResponse {
            status: 403,
            final_url: "https://example.com/admin/".into(),
            content_type: None,
            content_length: None,
            headers: Vec::new(),
            body: None,
        };
        let exposed = exposed_directories(vec![
            ("/uploads/".into(), ProbeOutcome::Response(listing)),
            ("/admin/".into(), ProbeOutcome::Response(forbidden)),
        ]);
        assert_eq!(exposed, vec!["/uploads/".to_string()]);
    }
}
