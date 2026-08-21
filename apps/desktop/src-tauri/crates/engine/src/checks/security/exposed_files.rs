//! Probes sensitive paths and requires file-specific response signatures.
//!
//! Catch-all responses and failed probes never prove exposure or absence.

use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};
use crate::probe::ProbeOutcome;

mod content_validation;
use content_validation::{classify_env_body, expected_file_signature, EnvBodyVerdict};

/// Namespace for the per-path ids this check builds from [`SENSITIVE_PATHS`].
/// The ids are dynamic, so the capability manifest covers them with one
/// family entry keyed by exactly this prefix rather than a row per path.
pub const CHECK_ID_PREFIX: &str = "security.exposed_files.";

/// Characters sampled from probe bodies for signatures after leading comments.
pub(crate) const PROBE_SIGNATURE_SAMPLE_CHARS: usize = 4096;

/// Sensitive path, description, and severity.
pub const SENSITIVE_PATHS: &[(&str, &str, Severity)] = &[
    (
        "/.env",
        ".env-format environment configuration",
        Severity::Critical,
    ),
    (
        "/.git/HEAD",
        "Git repository HEAD reference",
        Severity::Medium,
    ),
    (
        "/.git/config",
        "Git repository configuration",
        Severity::Medium,
    ),
    (
        "/wp-config.php",
        "WordPress configuration source",
        Severity::Critical,
    ),
    ("/.DS_Store", "macOS directory metadata", Severity::Low),
    ("/.htaccess", "Apache configuration file", Severity::Medium),
    ("/web.config", "IIS/ASP.NET configuration", Severity::Medium),
    ("/phpinfo.php", "PHP information page", Severity::Medium),
    ("/debug.log", "Debug log output", Severity::Medium),
    ("/error.log", "Error log output", Severity::Medium),
    ("/backup.sql", "SQL database backup", Severity::Critical),
    (
        "/backup.zip",
        "ZIP archive at a common backup path",
        Severity::High,
    ),
    ("/database.sql", "SQL database dump", Severity::Critical),
];

/// High-signal secret identifier names not owned by value-shaped key checks.
const SOURCE_SECRET_IDENTIFIERS: &[(&str, &str)] = &[
    ("database_url", "database URL identifier"),
    ("db_password", "database password identifier"),
    ("aws_secret", "AWS secret identifier"),
    ("stripe_secret", "Stripe secret identifier"),
];

/// Combine source and path results without passing inconclusive probes.
pub fn summarize_exposed_files(
    source_advisory: Option<CheckResult>,
    path_rows: Vec<CheckResult>,
    unjoined_probes: usize,
) -> Vec<CheckResult> {
    let mut results = Vec::new();
    let has_source_advisory = source_advisory.is_some();
    if let Some(result) = source_advisory {
        results.push(result);
    }

    let mut exposed_count = 0;
    let mut inconclusive_count = unjoined_probes;
    for row in path_rows {
        if row.status == CheckStatus::Fail {
            exposed_count += 1;
        } else if row.status == CheckStatus::Skipped {
            inconclusive_count += 1;
        }
        results.push(row);
    }

    if exposed_count == 0 && !has_source_advisory {
        let complete = inconclusive_count == 0;
        results.push(CheckResult {
            check_id: "security.exposed_files.summary".into(),
            category: ScanCategory::Security,
            title: if complete {
                "Sensitive file probes completed".into()
            } else {
                "Sensitive file probes incomplete".into()
            },
            description: if complete {
                format!(
                    "All {} sensitive-path probes completed; none returned content matching the requested file's signature.",
                    SENSITIVE_PATHS.len()
                )
            } else {
                format!(
                    "No matching sensitive-file exposure was found, but {} of {} probes were inconclusive. Review the per-path results or re-run from a network that can reach the target reliably.",
                    inconclusive_count,
                    SENSITIVE_PATHS.len()
                )
            },
            status: if complete {
                CheckStatus::Pass
            } else {
                CheckStatus::Skipped
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "paths_probed": SENSITIVE_PATHS.len(),
                "inconclusive": inconclusive_count,
            })),
            confidence: if complete {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if complete {
                None
            } else {
                Some("At least one path did not return enough evidence for a pass or exposure finding.".into())
            },
            why_it_matters: None,
        });
    }

    results
}

/// Advisory for secret-named identifiers; no secret value is verified.
pub fn source_secrets_result(html: &str) -> Option<CheckResult> {
    let script_content = extract_script_content(html);
    let script_lower = script_content.to_ascii_lowercase();

    let mut matched: Vec<&str> = SOURCE_SECRET_IDENTIFIERS
        .iter()
        .filter(|(pattern, _)| script_lower.contains(pattern))
        .map(|(_, label)| *label)
        .collect();
    matched.dedup();

    if matched.is_empty() {
        return None;
    }

    // Grab a short snippet around the first match for context
    let first_pattern = SOURCE_SECRET_IDENTIFIERS
        .iter()
        .find(|(p, _)| script_lower.contains(p))
        .map(|(p, _)| *p)
        .unwrap_or("");
    let snippet = secret_match_snippet(&script_content, first_pattern);

    Some(CheckResult {
        check_id: "security.exposed_files.source_secrets".into(),
        category: ScanCategory::Security,
        title: "Secret-named identifiers in page scripts".into(),
        description: format!(
            "Found {} secret-named identifier{} in inline scripts: {}.{} These are name references only - no secret value was verified. Actual key values are covered by the exposed API keys and hardcoded secrets checks.",
            matched.len(),
            if matched.len() == 1 { "" } else { "s" },
            matched.join(", "),
            if !snippet.is_empty() {
                format!(" Near: \"{}…\"", snippet.trim())
            } else {
                String::new()
            }
        ),
        status: CheckStatus::Warn,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: Some(format!(
            "Review why {} appear{} in client-side scripts. If a real credential is wired into the frontend, move it to a server-side environment variable and proxy the call through your backend. If the identifier only names a client-safe value, no change is needed.",
            matched.join(", "),
            if matched.len() == 1 { "s" } else { "" }
        )),
        raw_data: Some(serde_json::json!({ "matched_patterns": matched })),
        confidence: crate::checks::IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "Only an identifier name was matched in script source; the scan did not verify that a secret value is present.".into(),
        ),
        why_it_matters: Some(
            "A secret-named identifier can be a harmless property reference or evidence that a server credential entered a client bundle. Reviewing the value flow distinguishes those cases.".into(),
        ),
    })
}

/// Grade one sensitive-path probe outcome. Verifies actual content for
/// every 200: SPA catch-all hosts return 200 + an HTML shell for ANY path,
/// so status-only probes reported paths as "Exposed" on every such site.
pub fn grade_path_probe(
    path: &str,
    description: &str,
    severity: &Severity,
    outcome: ProbeOutcome,
) -> CheckResult {
    let resp = match outcome {
        ProbeOutcome::Failure(_) => {
            return inconclusive_probe_result(
                path,
                *severity,
                "The request failed before an HTTP response was received, so public exposure was not determined.",
            )
        }
        ProbeOutcome::Response(response) => response,
    };
    let status_code = resp.status;
    let content_type = resp
        .content_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let content_length = resp.content_length;

    if status_code != 200 {
        return if matches!(status_code, 401 | 403 | 404 | 410) {
            not_exposed_result(
                path,
                *severity,
                format!(
                    "{} returned HTTP {}; it was not publicly retrievable during this probe.",
                    path, status_code
                ),
            )
        } else {
            inconclusive_probe_result(
                path,
                *severity,
                &format!("{} returned HTTP {}; that response does not establish whether the file exists or is publicly retrievable.", path, status_code),
            )
        };
    }

    let body = match resp.body {
        Some(body) => body
            .text
            .chars()
            .take(PROBE_SIGNATURE_SAMPLE_CHARS)
            .collect::<String>(),
        None => {
            return inconclusive_probe_result(
                path,
                *severity,
                "HTTP 200 was returned, but the response body could not be read within the scan limits; exposure was not determined.",
            );
        }
    };

    if body.trim().is_empty() || content_length == Some(0) {
        return not_exposed_result(
            path,
            *severity,
            format!(
                "{} returned HTTP 200 with an empty body; no file content was exposed.",
                path
            ),
        );
    }
    if is_html_soft_404(path, &content_type, &body) {
        return not_exposed_result(
            path,
            *severity,
            format!("{} returned the site's HTML catch-all/error page, not content matching the requested file.", path),
        );
    }
    let Some(signature) = expected_file_signature(path, &content_type, &body) else {
        return inconclusive_probe_result(
            path,
            *severity,
            &format!("{} returned HTTP 200, but the sampled body did not match the expected content signature. The scan did not report the file as exposed; review unusual catch-all routing if this persists.", path),
        );
    };

    let effective_severity =
        if path == "/.env" && classify_env_body(&body) == EnvBodyVerdict::EnvFormatOnly {
            Severity::High
        } else {
            *severity
        };
    let impact = match effective_severity {
        Severity::Critical => "The matched response contains a sensitive configuration or database artifact. If its credential or data values are current, an unauthenticated visitor can copy them and use the access they confer.",
        Severity::High => "The matched response is a public configuration or backup artifact. Its actual impact depends on the archive or values it contains, but it may disclose source, configuration, credentials, or data.",
        Severity::Medium => "The matched artifact can disclose repository, server, or diagnostic details that reduce an attacker's uncertainty; the concrete impact depends on its contents and surrounding controls.",
        Severity::Low => "The matched metadata can reveal file and directory names. That is usually limited information disclosure, but it can help map the deployed site.",
    };
    CheckResult {
        check_id: format!("{CHECK_ID_PREFIX}{}", sanitize_check_id(path)),
        category: ScanCategory::Security,
        title: format!("Publicly accessible sensitive path: {}", path),
        description: format!(
            "GET {} returned HTTP 200, and the sampled response matched {}. Detected artifact: {}. The scan did not validate whether any contained credential is active or inspect the complete file.",
            path, signature, description
        ),
        status: CheckStatus::Fail,
        severity: effective_severity,
        fix_prompt: None,
        manual_fix: Some(blocked_access_fix(path)),
        raw_data: Some(serde_json::json!({
            "path": path,
            "status_code": status_code,
            "content_length": content_length,
            "matched_signature": signature,
            "sample_bytes": body.len(),
        })),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: Some(format!(
            "An unauthenticated GET returned HTTP 200 and the sampled body matched the file-specific signature: {}. Contents beyond the sample and credential validity were not assessed.",
            signature
        )),
        why_it_matters: Some(impact.into()),
    }
}

fn not_exposed_result(path: &str, severity: Severity, description: String) -> CheckResult {
    CheckResult {
        check_id: format!("{CHECK_ID_PREFIX}{}", sanitize_check_id(path)),
        category: ScanCategory::Security,
        title: format!("{} not publicly exposed", path),
        description,
        status: CheckStatus::Pass,
        severity,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

fn inconclusive_probe_result(path: &str, severity: Severity, detail: &str) -> CheckResult {
    CheckResult {
        check_id: format!("{CHECK_ID_PREFIX}{}", sanitize_check_id(path)),
        category: ScanCategory::Security,
        title: format!("{} exposure check inconclusive", path),
        description: detail.into(),
        status: CheckStatus::Skipped,
        severity,
        fix_prompt: None,
        manual_fix: Some(format!(
            "Repeat an unauthenticated GET request to {} from the same network. Confirm the response is 403, 404, or 410 and that no sensitive content is returned; do not copy any returned secret values into tickets or chat.",
            path
        )),
        raw_data: None,
        confidence: crate::checks::IssueConfidence::NeedsReview,
        confidence_reason: Some("The probe did not produce enough matching response evidence for a pass or an exposure finding.".into()),
        why_it_matters: None,
    }
}

/// Stack-specific instructions that block public access to the exact probed path.
fn blocked_access_fix(path: &str) -> String {
    format!(
        "Block public access to {path}. Apply ONE of these depending on your stack:\n\
         • Nginx: `location {path} {{ return 404; }}`\n\
         • Apache (.htaccess): `Redirect 404 {path}` (matches nested paths too)\n\
         • Caddy: `respond {path} 404`\n\
         • Vercel: remove the file from the deployment output (move it out of `public/`); static files that ship with the deploy cannot be blocked afterwards\n\
         • Netlify (_redirects): `{path} /404 404!`\n\
         • Cloudflare Pages: add a Worker route or _redirects entry; the file should not be in your build output at all\n\
         • Static-site / framework: move the file OUT of the public/static/dist directory so it never gets deployed. After fixing, repeat an unauthenticated GET and confirm a 403, 404, or 410; a HEAD-only check can miss servers that handle GET differently. Do not paste any returned secrets into tickets or chat.",
        path = path,
    )
}

/// Detects HTML catch-all responses masquerading as exposed sensitive files.
/// `phpinfo.php` is excluded because its legitimate response is HTML.
fn is_html_soft_404(path: &str, content_type: &str, body_snippet: &str) -> bool {
    if !content_type.contains("text/html") || path.contains("phpinfo") {
        return false;
    }
    let lower = body_snippet.to_lowercase();
    lower.contains("<html")
        || lower.contains("<!doctype")
        || lower.contains("not found")
        || lower.contains("404")
}

fn sanitize_check_id(path: &str) -> String {
    path.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .to_lowercase()
}

fn secret_match_snippet(script_content: &str, pattern: &str) -> String {
    let lower = script_content.to_ascii_lowercase();
    let Some(position) = lower.find(pattern) else {
        return String::new();
    };
    let start = crate::checks::floor_char_boundary(script_content, position.saturating_sub(20));
    let end = crate::checks::ceil_char_boundary(
        script_content,
        (position + pattern.len() + 40).min(script_content.len()),
    );
    script_content[start..end]
        .chars()
        .take(80)
        .collect::<String>()
        .replace('\n', " ")
        .replace('\r', "")
}

/// Extracts all content from <script> tags (concatenated)
fn extract_script_content(html: &str) -> String {
    let mut result = String::new();
    let lower = html.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(start) = lower[search_from..].find("<script") {
        let abs_start = search_from + start;
        // Find the end of opening tag
        if let Some(gt) = lower[abs_start..].find('>') {
            let content_start = abs_start + gt + 1;
            if let Some(end) = lower[content_start..].find("</script") {
                let content_end = content_start + end;
                if content_end > content_start {
                    result.push(' ');
                    result.push_str(&html[content_start..content_end]);
                }
                // Advance past the matched prefix; truncated tags may omit `>`.
                search_from = content_end + "</script".len();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::IssueConfidence;

    #[test]
    fn script_extraction_preserves_offsets_after_unicode_case_expansion() {
        let html = format!(
            "<html><body>{}<script>const db_password = 'public-example';</script></body></html>",
            "İ".repeat(32)
        );

        assert_eq!(
            extract_script_content(&html).trim(),
            "const db_password = 'public-example';"
        );
    }

    #[test]
    fn secret_snippet_uses_valid_unicode_boundaries() {
        let script = format!("{}db_password = 'public-example';", "é".repeat(11));

        let snippet = secret_match_snippet(&script, "db_password");

        assert!(snippet.contains("db_password"));
        assert!(snippet.is_char_boundary(snippet.len()));
    }

    #[test]
    fn script_extraction_survives_truncated_closing_tag() {
        let truncated = "<html><body><script>db_password = 'x';</script";
        let out = extract_script_content(truncated);
        assert!(out.contains("db_password"), "{out}");

        // Malformed close followed by a multibyte char must also not panic.
        let malformed = "<script>secret_value = 1;</scriptФ rest";
        let _ = extract_script_content(malformed);
    }

    #[test]
    fn firebase_config_apikey_is_not_flagged_as_source_secret() {
        let html = r#"<script>const config = { apiKey: "AIzaSyDxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", authDomain: "x.firebaseapp.com" };</script>"#;
        assert!(source_secrets_result(html).is_none());
    }

    #[test]
    fn stripe_live_key_is_left_to_exposed_keys_check() {
        // sk_live_ keys are flagged (value-shaped, Critical) by
        // security.vibe.exposed_keys; this check must not double-report them.
        let html = r#"<script>const key = "sk_live_1234567890abcdefghijklmn";</script>"#; // gitleaks:allow
        assert!(source_secrets_result(html).is_none());
    }

    #[test]
    fn remaining_identifier_matches_are_needs_review_advisories() {
        let html = r#"<script>fetch(config.database_url); const x = settings.aws_secret;</script>"#;
        let result = source_secrets_result(html).expect("identifiers should match");
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.severity, Severity::Medium);
        assert_eq!(result.confidence, IssueConfidence::NeedsReview);
        assert!(result.confidence_reason.is_some());
        assert!(
            result.description.contains("2 secret-named identifiers"),
            "{}",
            result.description
        );
        assert!(
            result.description.contains("no secret value was verified"),
            "{}",
            result.description
        );
    }

    #[test]
    fn single_identifier_match_is_singular_not_one_matches() {
        let html = r#"<script>const conn = config.db_password;</script>"#;
        let result = source_secrets_result(html).expect("identifier should match");
        assert!(
            result.description.contains("1 secret-named identifier in"),
            "{}",
            result.description
        );
        assert!(!result.description.contains("1 matches"));
    }

    #[test]
    fn security_txt_is_owned_by_the_dedicated_check() {
        // The HEAD-only /.well-known/security.txt probe false-passed on SPA
        // catch-all hosts and duplicated security.security_txt findings.
        assert!(
            !SENSITIVE_PATHS
                .iter()
                .any(|(path, _, _)| path.contains("security.txt")),
            "security.txt must not be probed by exposed_files; security_txt.rs owns it"
        );
    }

    #[test]
    fn env_body_with_secret_assignments_is_verified_critical_content() {
        let body =
            "# prod config\nDATABASE_URL=postgres://admin:pw@db:5432/prod\nSMTP_PASSWORD=hunter2\n";
        assert_eq!(classify_env_body(body), EnvBodyVerdict::SecretAssignments);
    }

    #[test]
    fn env_body_without_secret_keys_is_env_format_only() {
        let body = "APP_NAME=myapp\nAPP_COLOR=blue\n";
        assert_eq!(classify_env_body(body), EnvBodyVerdict::EnvFormatOnly);
    }

    #[test]
    fn non_env_bodies_are_not_treated_as_exposed_env_files() {
        // A catch-all route serving prose, JSON, or JS must not produce the
        // cap-listed Critical ".env exposed" verdict.
        assert_eq!(
            classify_env_body("Welcome to our site! Everything is fine."),
            EnvBodyVerdict::NotEnvContent
        );
        assert_eq!(
            classify_env_body("{\"status\":\"ok\",\"version\":\"1.2.3\"}"),
            EnvBodyVerdict::NotEnvContent
        );
        assert_eq!(classify_env_body(""), EnvBodyVerdict::NotEnvContent);
    }

    #[test]
    fn blocked_access_fix_options_actually_block() {
        let fix = blocked_access_fix("/.git/HEAD");
        assert!(
            !fix.contains("x-block"),
            "header-only advice does not block"
        );
        assert!(!fix.contains("<Files"), "Files matches basenames only");
        assert!(!fix.contains("\\n"), "no literal backslash-n in fix text");
        assert!(!fix.contains('\u{2014}'), "no em-dashes in fix text");
        assert!(fix.contains("Redirect 404 /.git/HEAD"));
    }

    #[test]
    fn spa_html_shell_is_soft_404_for_medium_paths() {
        let shell = "<!doctype html><html><head><title>App</title></head><body><div id=\"root\"></div></body></html>";
        assert!(is_html_soft_404(
            "/.DS_Store",
            "text/html; charset=utf-8",
            shell
        ));
        assert!(is_html_soft_404("/error.log", "text/html", shell));
    }

    #[test]
    fn real_file_content_is_not_soft_404() {
        // A genuine.DS_Store or log served as octet-stream/plain must
        // still count as exposed.
        assert!(!is_html_soft_404(
            "/.DS_Store",
            "application/octet-stream",
            "Bud1\u{0}\u{0}\u{0}"
        ));
        assert!(!is_html_soft_404(
            "/error.log",
            "text/plain",
            "[error] PHP Warning: undefined index"
        ));
    }

    #[test]
    fn phpinfo_html_is_not_soft_404() {
        assert!(!is_html_soft_404(
            "/phpinfo.php",
            "text/html",
            "<html><body><h1>PHP Version 8.3</h1></body></html>"
        ));
    }
}
