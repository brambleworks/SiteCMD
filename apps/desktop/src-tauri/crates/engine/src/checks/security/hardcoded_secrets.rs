//! Detects credential-shaped literals in inline HTML and JavaScript.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

struct SecretPattern {
    label: &'static str,
    regex: regex::Regex,
    /// Candidate-value capture group for patterns that need shape validation.
    value_group: Option<usize>,
    /// Whether broad name-value matching limits the result to NeedsReview.
    heuristic: bool,
}

/// Values the password pattern can capture that are never credentials:
/// form-validation keywords and i18n bundle strings.
static NON_SECRET_VALUES: &[&str] = &[
    "required",
    "optional",
    "hidden",
    "text",
    "password",
    "true",
    "false",
    "null",
    "undefined",
    "none",
];

/// Reject validation keywords and repeated-character masks from secret candidates.
fn looks_like_candidate_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if NON_SECRET_VALUES.contains(&lower.as_str()) {
        return false;
    }
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => !chars.all(|c| c == first),
        None => false,
    }
}

static SECRET_PATTERNS: LazyLock<Vec<SecretPattern>> = LazyLock::new(|| {
    vec![
        SecretPattern {
            label: "password assignment",
            // Matches: password = "...", password: "...", "password":"..." (JSON).
            // The value must be whitespace-free: prose explanations
            // ("must be at least 8 characters") and i18n strings
            // ("Forgot your password?") are sentences, secrets are not.
            regex: regex::Regex::new(
                r#"(?i)["']?password["']?\s*[:=]\s*["']([^"'\s]{4,})["']"#
            ).unwrap(),
            value_group: Some(1),
            heuristic: true,
        },
        SecretPattern {
            label: "database connection string",
            // postgres://, mysql://, mongodb://, redis:// with credentials
            regex: regex::Regex::new(
                r#"(?i)(postgres|mysql|mongodb|redis|amqp)://[^:]+:[^@]+@[^\s"'<]{5,}"#
            ).unwrap(),
            value_group: None,
            heuristic: false,
        },
        SecretPattern {
            label: "secret/token assignment",
            // Matches: secret = "...", api_secret = "...", token = "..."
            // but NOT "csrf_token" or "access_token" element names
            regex: regex::Regex::new(
                r#"(?i)["']?(api[_-]?secret|client[_-]?secret|jwt[_-]?secret|auth[_-]?token|bearer[_-]?token|private[_-]?key)["']?\s*[:=]\s*["'][a-zA-Z0-9\-_./+]{8,}["']"#
            ).unwrap(),
            value_group: None,
            heuristic: false,
        },
        SecretPattern {
            label: "basic auth header",
            // Authorization: Basic <base64> - also matches headers["authorization"]
            regex: regex::Regex::new(
                r#"(?i)["']?authorization["']?\s*[\]:]?\s*[:=]\s*["']Basic\s+[A-Za-z0-9+/=]{8,}["']"#
            ).unwrap(),
            value_group: None,
            heuristic: false,
        },
        SecretPattern {
            label: "bearer token header",
            // Authorization: Bearer <token> with substantial length
            regex: regex::Regex::new(
                r#"(?i)["']?authorization["']?\s*[:=]\s*["']Bearer\s+[A-Za-z0-9\-_.]{20,}["']"#
            ).unwrap(),
            value_group: None,
            heuristic: false,
        },
    ]
});

pub struct HardcodedSecretsCheck;

impl Check for HardcodedSecretsCheck {
    fn id(&self) -> &str {
        "security.vibe.hardcoded_secrets"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut found_labels: Vec<String> = Vec::new();
        let mut has_value_shaped = false;

        for pattern in SECRET_PATTERNS.iter() {
            let matched = match pattern.value_group {
                Some(group) => pattern.regex.captures_iter(&ctx.body).any(|caps| {
                    caps.get(group)
                        .map(|value| looks_like_candidate_value(value.as_str()))
                        .unwrap_or(false)
                }),
                None => pattern.regex.is_match(&ctx.body),
            };
            if matched {
                found_labels.push(pattern.label.to_string());
                if !pattern.heuristic {
                    has_value_shaped = true;
                }
            }
        }

        found_labels.dedup();

        if found_labels.is_empty() {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Hardcoded secrets in client source".into(),
                description: "No hardcoded passwords, tokens, or connection strings were detected in the fetched client source.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        let labels = found_labels.join(", ");
        let manual_fix = "1. Inspect the matched value locally without copying it into logs, tickets, or third-party tools and determine whether it is a genuine credential or a fixture/example\n\
             2. If genuine and possibly tracked, shared, logged, bundled, or deployed, revoke or rotate it first, then remove it from client output and history according to incident policy\n\
             3. Store the replacement in an appropriate server-side secret store and expose only a narrow authenticated, authorized, and validated backend operation\n\
             4. If it is deliberately fake, use a provider-documented non-secret test value or label the fixture clearly";

        // Value-shaped evidence (connection strings with credentials,
        // Authorization headers, secret/token assignments) is stronger, but
        // it is not provider validation and examples can still match.
        if has_value_shaped {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Credential-shaped value in client-side source".into(),
                description: format!(
                    "Found {} credential-shaped pattern{} in fetched client-visible source: {}. The match confirms that the text was delivered, but it does not establish that the value is genuine, live, privileged, or accepted by the named service; fixtures and documentation examples can match. If it is a real credential, every page recipient can copy it.",
                    found_labels.len(),
                    if found_labels.len() == 1 { "" } else { "s" },
                    labels
                ),
                status: CheckStatus::Fail,
                severity: Severity::High,
                fix_prompt: Some(format!(
                    "Classify these client-visible credential-shaped values without reproducing them: {}. Revoke or rotate any genuine credential that may have been exposed, then keep the replacement behind a server-side boundary.",
                    labels
                )),
                manual_fix: Some(manual_fix.into()),
                raw_data: Some(serde_json::json!({ "secret_types": found_labels })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("A specific credential-shaped value is present in fetched client source, but static matching cannot establish provider validity, account ownership, privilege, revocation state, or whether the text is a deliberate fixture.".into()),
                why_it_matters: Some("Client-delivered values are public to page recipients. A genuine live credential there can be copied and used within its permissions; an invalid or documented fixture cannot.".into()),
            }];
        }

        // Heuristic-only evidence (a quoted value next to a password-ish
        // name): frequently an i18n label or placeholder, so this grades
        // Warn/NeedsReview rather than asserting a credential.
        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: "Possible hardcoded password in client source".into(),
            description: format!(
                "Found a quoted value assigned to a password-named field in client-visible source ({}). This is often an i18n label, placeholder, or validation string rather than a live credential - review the value to confirm.",
                labels
            ),
            status: CheckStatus::Warn,
            severity: Severity::Medium,
            fix_prompt: Some(
                "A password-named field in client-visible source has a quoted value assigned. If it is a real credential, move it server-side and rotate it; if it is an i18n string or placeholder, no change is needed.".into(),
            ),
            manual_fix: Some(manual_fix.into()),
            raw_data: Some(serde_json::json!({ "secret_types": found_labels })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some(
                "Only a name-and-value pattern was matched; translated interface strings and placeholders match the same shape as real passwords.".into(),
            ),
            why_it_matters: Some("Anything in client code is effectively public. If this value is a live credential, someone else can copy and use it.".into()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::HeaderMap;

    fn ctx(body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: HeaderMap::new(),
            status_code: 200,
            body: body.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn clean_page_passes() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn password_assignment_is_needs_review_warn_not_critical() {
        let html = r#"<script>const password = "hunter2secret";</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].confidence_reason.is_some());
        assert!(results[0].description.contains("password"));
    }

    #[test]
    fn i18n_password_value_is_not_a_critical_secret() {
        let html = r#"<script>var es = {"password":"Contrasena","user":"Usuario"};</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_ne!(results[0].status, CheckStatus::Fail);
        assert_ne!(results[0].severity, Severity::Critical);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn value_shaped_secret_is_high_needs_review_even_with_password_match() {
        // A connection string is strong review evidence, but static text does
        // not verify that the credential is genuine, live, or privileged.
        let html = r#"<script>const db = "postgres://admin:s3cret@db.example.com:5432/mydb"; const password = "hunter2secret";</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::High);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("does not establish"));
    }

    #[test]
    fn pass_title_does_not_assert_a_finding() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(!results[0].title.contains("appear"), "{}", results[0].title);
    }

    #[test]
    fn detects_database_url() {
        let html =
            r#"<script>const db = "postgres://admin:s3cret@db.example.com:5432/mydb";</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert!(results[0].description.contains("database"));
    }

    #[test]
    fn detects_jwt_secret() {
        let html = r#"<script>const jwt_secret = "super_secret_jwt_signing_key_12345";</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
    }

    #[test]
    fn detects_basic_auth_header() {
        let html = r#"<script>headers["authorization"] = "Basic dXNlcjpwYXNzd29yZA==";</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
    }

    #[test]
    fn no_false_positive_on_login_form() {
        // A password input field should NOT trigger
        let html =
            r#"<form><input type="password" name="password" placeholder="Enter password"></form>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn no_false_positive_on_short_password() {
        // Very short values like password="***" shouldn't trigger (likely placeholder)
        let html = r#"<script>password = "***";</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn no_false_positive_on_explanatory_copy() {
        let html = r#"<p>Your password: "must be at least 8 characters"</p>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn no_false_positive_on_i18n_bundle_string() {
        let html = r#"<script>var messages = {"password": "Forgot your password?"};</script>"#;
        let results = HardcodedSecretsCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn no_false_positive_on_masked_or_keyword_values() {
        let masked = r#"<script>password = "********";</script>"#;
        assert_eq!(
            HardcodedSecretsCheck.run(&ctx(masked))[0].status,
            CheckStatus::Pass
        );
        let keyword = r#"<script>var rules = {"password": "required"};</script>"#;
        assert_eq!(
            HardcodedSecretsCheck.run(&ctx(keyword))[0].status,
            CheckStatus::Pass
        );
    }
}
