//! Detects environment references, assignments, and runtime config in page source.
//!
//! Static matches establish public text, not credential validity or provenance.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

/// What a pattern match actually proves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExposureKind {
    /// Secret-named KEY=value text is present; page context and validity are
    /// not established.
    Values,
    /// A literal, un-substituted reference such as `process.env.SECRET_KEY`;
    /// the reference text surviving into the bundle means no value was
    /// inlined, so no secret value was observed.
    Reference,
    /// A `window.__env = {` style config dump; often legitimate public
    /// config, worth a manual look.
    ConfigDump,
}

struct EnvPattern {
    label: &'static str,
    regex: regex::Regex,
    kind: ExposureKind,
}

static ENV_PATTERNS: LazyLock<Vec<EnvPattern>> = LazyLock::new(|| {
    vec![
        EnvPattern {
            label: "process.env reference with secret-like key",
            // process.env.SECRET_KEY, process.env.DATABASE_URL, etc.
            regex: regex::Regex::new(
                r#"process\.env\.(SECRET|PRIVATE|PASSWORD|DATABASE|DB_|MONGO|REDIS|AUTH|JWT|SMTP|MAIL_PASS|API_SECRET|ENCRYPTION)"#
            ).unwrap(),
            kind: ExposureKind::Reference,
        },
        EnvPattern {
            label: ".env file content",
            // Looks like.env file entries: KEY=value pairs with secret-looking keys
            regex: regex::Regex::new(
                r#"(?m)^(?:SECRET_KEY|DATABASE_URL|DB_PASSWORD|PRIVATE_KEY|JWT_SECRET|SMTP_PASSWORD|API_SECRET|ENCRYPTION_KEY)\s*=\s*\S+"#
            ).unwrap(),
            kind: ExposureKind::Values,
        },
        EnvPattern {
            label: "import.meta.env reference with secret-like key",
            // Vite pattern: import.meta.env.SECRET (non-VITE_ prefixed are safe)
            regex: regex::Regex::new(
                r#"import\.meta\.env\.(SECRET|PRIVATE|PASSWORD|DATABASE|DB_|AUTH_SECRET|JWT)"#
            ).unwrap(),
            kind: ExposureKind::Reference,
        },
        EnvPattern {
            label: "__env or window.__env config object",
            // Common pattern: window.__env = {... } dumping runtime config to client
            regex: regex::Regex::new(
                r#"(?i)(?:window\.__env|window\.ENV|globalThis\.__env)\s*=\s*\{"#
            ).unwrap(),
            kind: ExposureKind::ConfigDump,
        },
    ]
});

pub struct EnvExposureCheck;

impl Check for EnvExposureCheck {
    fn id(&self) -> &str {
        "security.vibe.env_exposure"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut found: Vec<(String, ExposureKind)> = Vec::new();

        for pattern in ENV_PATTERNS.iter() {
            if pattern.regex.is_match(&ctx.body) {
                found.push((pattern.label.to_string(), pattern.kind));
            }
        }

        found.dedup_by(|a, b| a.0 == b.0);

        if found.is_empty() {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Environment Variable Exposure".into(),
                description:
                    "No exposed environment variables or .env content detected in page source."
                        .into(),
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

        let labels = found
            .iter()
            .map(|(l, _)| l.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let has_values = found.iter().any(|(_, k)| *k == ExposureKind::Values);
        let has_refs = found.iter().any(|(_, k)| *k == ExposureKind::Reference);
        let raw_data = Some(
            serde_json::json!({ "exposure_types": found.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>() }),
        );

        // Credential-like KEY=value text is client-visible, but a regex match
        // cannot distinguish a real environment dump from documentation or a
        // fake fixture and cannot validate a credential at its provider.
        if has_values {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Credential-like environment assignments in page source".into(),
                description: format!(
                    "Found .env-style secret-named KEY=value text in the fetched page source ({}). The text is client-visible, but the match does not establish that the value is genuine, live, privileged, sourced from the runtime environment, or intentionally deployed; documentation and fake fixtures can match. If a real privileged value is present, every recipient of the page can read it.",
                    labels
                ),
                status: CheckStatus::Fail,
                severity: Severity::High,
                fix_prompt: Some(format!(
                    "Classify the matched client-visible environment-style values without copying them into logs or tickets: {}. If any is a genuine privileged credential, revoke or rotate it and remove it from browser-delivered content; otherwise label fake fixtures or examples clearly.",
                    labels
                )),
                manual_fix: Some(shared_manual_fix()),
                raw_data,
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Secret-named KEY=value text is present in the fetched page, but regex matching cannot establish credential validity, privilege, provenance, or whether the text is an intentional non-secret example.".into()),
                why_it_matters: Some(
                    "A genuine privileged environment value delivered to browsers is public to page recipients and may grant access to its backing service; an example or invalid fixture does not carry that impact.".into(),
                ),
            }];
        }

        // Un-substituted references only: no value was observed.
        if has_refs {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Environment variable references in page source".into(),
                description: format!(
                    "Found {} in page JavaScript. The literal reference text survived into the \
                     served bundle. No secret value was observed. The reference may be unresolved build output, dead or example text, or a runtime access that evaluates differently in the target framework; review the production artifact and intended server/client boundary.",
                    labels
                ),
                status: CheckStatus::Warn,
                severity: Severity::Medium,
                fix_prompt: Some(format!(
                    "Client-side code references server-side environment variables: {}. \
                     No value appears to have been substituted. Confirm whether the expression is reachable in the production client; move genuinely privileged operations behind a server boundary and expose only values explicitly safe for every browser recipient.",
                    labels
                )),
                manual_fix: Some(shared_manual_fix()),
                raw_data,
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Only literal environment-reference text was observed; no value, runtime evaluation, production reachability, or server/client ownership was established.".into()),
            why_it_matters: Some(
                    "Un-substituted references usually mean no value leaked, but they can reveal \
                     server-side variable names or indicate an unresolved client/server build \
                     boundary when the code is reachable. Dead code and examples carry less impact."
                        .into(),
                ),
            }];
        }

        // window.__env-style config dump only: often legitimate public config.
        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: "window.__env configuration object in page source".into(),
            description: format!(
                "Found {} in the page source. This pattern is commonly used to deliver public \
                 runtime configuration such as API base URLs or feature flags and is legitimate \
                 when every value is intentionally public. The scan did not classify the object's values.",
                labels
            ),
            status: CheckStatus::Warn,
            severity: Severity::Medium,
            fix_prompt: Some(
                "A window.__env-style configuration object is assigned in page source. \
                 Review its contents: public values (API base URL, feature flags) are fine, \
                 but any server-side secret placed there is exposed to every visitor and must \
                 move behind a server-side API route."
                    .into(),
            ),
            manual_fix: Some(shared_manual_fix()),
            raw_data,
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some(
                "A window.__env assignment alone does not show which values it holds; exposing \
                 public config this way is a legitimate pattern."
                    .into(),
            ),
            why_it_matters: Some(
                "Config objects dumped into the page are fully public. They are safe only as \
                 long as every value in them is meant to be public."
                    .into(),
            ),
        }]
    }
}

fn shared_manual_fix() -> String {
    "1. Inspect the exact production HTML and client bundles and classify each matched value or reference without copying it to logs or tickets\n\
     2. Treat public prefixes such as NEXT_PUBLIC_*, VITE_*, and REACT_APP_* as disclosure mechanisms, not safety checks; expose only values safe for every visitor\n\
     3. If using window.__env, keep it to deliberate public configuration such as an API base URL or feature flag\n\
     4. If a genuine credential may have been tracked, shared, logged, bundled, or deployed, revoke or rotate it first, then remove it from client output and repository history according to incident policy\n\
     5. Keep privileged service access behind an authenticated, authorized, and validated server-side operation"
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{IssueConfidence, PageContext};
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
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn process_env_reference_is_warn_not_critical_exposure() {
        let html = r#"<script>const dbUrl = process.env.DATABASE_URL;</script>"#;
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(
            results[0].title.contains("references"),
            "title must describe references, not exposure: {}",
            results[0].title
        );
        assert!(
            results[0]
                .description
                .to_ascii_lowercase()
                .contains("no secret value was observed"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn detects_window_env_dump_as_needs_review_advisory() {
        // window.__env = {... } is a common, often legitimate way to ship
        // public runtime config; it grades Warn/Medium with NeedsReview.
        let html = r#"<script>window.__env = {"API_URL": "https://api.example.com"};</script>"#;
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].confidence_reason.is_some());
    }

    #[test]
    fn detects_dotenv_shaped_content_as_high_needs_review() {
        let html = "DATABASE_URL=postgres://admin:password@db.example.com:5432/prod\nSECRET_KEY=mysupersecretkey";
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::High);
        assert!(results[0].title.contains("Credential-like"));
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].description.contains("does not establish"));
    }

    #[test]
    fn values_take_precedence_over_references() {
        let html = "<script>const x = process.env.SECRET_KEY;</script>\nJWT_SECRET=abc123def456"; // gitleaks:allow
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::High);
    }

    #[test]
    fn fix_prompt_has_no_orphaned_punctuation() {
        for html in [
            r#"<script>const dbUrl = process.env.DATABASE_URL;</script>"#,
            "SECRET_KEY=mysupersecretkey",
            r#"<script>window.__env = {"API_URL": "x"};</script>"#,
        ] {
            let results = EnvExposureCheck.run(&ctx(html));
            let prompt = results[0].fix_prompt.as_deref().unwrap_or("");
            assert!(!prompt.contains(" . "), "orphaned period in: {}", prompt);
        }
    }

    #[test]
    fn safe_vite_env_does_not_trigger() {
        let html = r#"<script>const url = import.meta.env.VITE_API_URL;</script>"#;
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn explicitly_public_process_env_name_does_not_trigger() {
        // NEXT_PUBLIC_ marks browser delivery; it is not a safety guarantee.
        let html = r#"<script>const key = process.env.NEXT_PUBLIC_API_URL;</script>"#;
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn detects_import_meta_env_secret_reference_as_warn() {
        let html = r#"<script>const secret = import.meta.env.SECRET_KEY;</script>"#;
        let results = EnvExposureCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
    }
}
