//! Release-artifact and content checks for local pre-deploy previews.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

static EXAMPLE_COM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)>([^<]*example\.com[^<]*)<").expect("static example.com regex")
});

/// Detects development-only libraries loaded in the client (e.g., React DevTools, livereload).
pub struct DevDependenciesCheck;

impl Check for DevDependenciesCheck {
    fn id(&self) -> &str {
        "config.dev_dependencies"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        if !ctx.is_localhost {
            return vec![];
        }

        let body_lower = ctx.body_lower();
        let mut found: Vec<&str> = vec![];

        let patterns = [
            ("livereload", "LiveReload"),
            ("browser-sync", "BrowserSync"),
            ("webpack-dev-server", "Webpack Dev Server"),
            ("hot-update.js", "HMR update script"),
            ("__webpack_hmr", "Webpack HMR"),
            ("vite/client", "Vite HMR client"),
            ("react-refresh", "React Fast Refresh"),
            ("_next/webpack-hmr", "Next.js HMR"),
            ("turbopack-hmr", "Turbopack HMR"),
            ("ember-cli-live-reload", "Ember LiveReload"),
            ("browsersync", "BrowserSync"),
        ];

        // Several patterns describe the same runtime (browser-sync/browsersync),
        // so the label list is deduplicated before it reaches the description.
        for (pattern, label) in patterns {
            if body_lower.contains(pattern) && !found.contains(&label) {
                found.push(label);
            }
        }

        if found.is_empty() {
            vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "No dev-only dependencies in client".into(),
                description:
                    "No development-only libraries (LiveReload, HMR, etc.) detected in the HTML."
                        .into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }]
        } else {
            vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Development runtime markers in local preview".into(),
                description: format!("Found recognized development runtime markers in the local preview: {}. HMR, live reload, and refresh clients are expected on a development server. Their presence here does not establish that the production artifact includes them; this finding is a prompt to verify the release build rather than to remove tools from local development.", found.join(", ")),
                status: CheckStatus::Warn,
                severity: Severity::Low,
                fix_prompt: Some("Build and inspect the exact production artifact; ensure development runtime clients are absent there while keeping the normal local-development workflow intact.".into()),
                manual_fix: Some("Run the documented production build and serve that artifact with production configuration, then inspect HTML, client bundles, and network requests for the reported markers. If they remain, correct the build/deploy mode or entry point; do not disable HMR/live reload in the ordinary development server merely to clear this local-preview review.".into()),
                raw_data: Some(serde_json::json!({ "found": found })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Development markers were observed on a localhost preview where they are expected; the production artifact and deployment were not evaluated.".into()),
                why_it_matters: Some("If development runtime clients also ship in production, they add unnecessary code/network activity and may expose debugging surfaces; their presence on localhost is normal.".into()),
            }]
        }
    }
}

/// Detects placeholder text (Lorem Ipsum, sample data, example.com) in rendered HTML.
pub struct PlaceholderContentCheck;

impl Check for PlaceholderContentCheck {
    fn id(&self) -> &str {
        "config.placeholder_content"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Config
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        if !ctx.is_localhost {
            return vec![];
        }

        // The description promises "visible page content", so strip scripts,
        // styles, and comments before matching - a JS string mentioning
        // "john doe" is not placeholder copy.
        let visible_lower = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE
            .replace_all(ctx.body_lower(), " ")
            .into_owned();
        let mut found: Vec<&str> = vec![];

        if visible_lower.contains("lorem ipsum") {
            found.push("Lorem Ipsum text");
        }
        if visible_lower.contains("dolor sit amet") {
            found.push("Lorem Ipsum continuation");
        }
        if visible_lower.contains("placeholder text") {
            found.push("\"Placeholder text\" literal");
        }
        if visible_lower.contains("your name here") {
            found.push("\"Your name here\"");
        }
        if visible_lower.contains("john doe") || visible_lower.contains("jane doe") {
            found.push("John/Jane Doe");
        }
        if visible_lower.contains("test@test.com") || visible_lower.contains("test@example.com") {
            found.push("Test email address");
        }
        // Don't flag example.com in meta tags / comments - only in visible text
        if EXAMPLE_COM_RE.is_match(&ctx.body) {
            found.push("example.com in visible text");
        }

        if found.is_empty() {
            vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "No placeholder content detected".into(),
                description: "No Lorem Ipsum, test data, or placeholder text found.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }]
        } else {
            vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Possible placeholder content in local preview".into(),
                description: format!(
                    "Found text patterns commonly used as placeholder or test content in the visible local preview: {}. The match does not establish a launch defect; documentation, examples, demos, privacy-preserving sample data, or a deliberate design can use these values legitimately.",
                    found.join(", ")
                ),
                status: CheckStatus::Warn,
                severity: Severity::Medium,
                fix_prompt: Some("Review each matched value in page context and replace only content that is unintended filler for the release experience.".into()),
                manual_fix: Some("Open each matched page in the intended release flow. Keep deliberate examples, demos, and reserved example domains when clearly labeled; replace lorem ipsum, fake contact data, sample calls to action, or test identities that users could mistake for real product content.".into()),
                raw_data: Some(serde_json::json!({ "found": found })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The text pattern is visible in a local preview, but page purpose, labeling, production content substitution, and whether the value is a deliberate example were not evaluated.".into()),
                why_it_matters: Some("Unintended filler can confuse users or send them toward fake contact details, while clearly labeled examples and demo data are valid product content.".into()),
            }]
        }
    }
}

/// Detects environment variable values or patterns leaked into client-side HTML/scripts.
pub struct EnvLeakCheck;

static ENV_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Match patterns like REACT_APP_*, NEXT_PUBLIC_*, VITE_*, VUE_APP_* in source
    Regex::new(r#"(?:process\.env\.|import\.meta\.env\.)\w+"#).unwrap()
});

static SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Match patterns that look like leaked secrets. The value must be
    // whitespace-free: prose like password: "Forgot your password?" in
    // an i18n bundle is a sentence, secrets are not (    // same shape as security/hardcoded_secrets.rs).
    Regex::new(r#"(?i)(?:api[_-]?key|api[_-]?secret|secret[_-]?key|private[_-]?key|auth[_-]?token|access[_-]?token|password)\s*[:=]\s*["']([^"'\s]{8,})["']"#).unwrap()
});

/// Masks like `********` are not secret values.
fn secret_value_is_plausible(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => !chars.all(|c| c == first),
        None => false,
    }
}

impl Check for EnvLeakCheck {
    fn id(&self) -> &str {
        "security.env_leak"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        if !ctx.is_localhost {
            return vec![];
        }

        let mut findings: Vec<String> = vec![];

        // Check for un-substituted env var references
        let env_refs: Vec<String> = ENV_PATTERN
            .find_iter(&ctx.body)
            .map(|m| m.as_str().to_string())
            .collect();

        if !env_refs.is_empty() {
            findings.push(format!(
                "{} env var {}",
                env_refs.len(),
                if env_refs.len() == 1 {
                    "reference"
                } else {
                    "references"
                }
            ));
        }

        // Check for patterns that look like hardcoded secrets
        let secret_matches: Vec<String> = SECRET_PATTERN
            .captures_iter(&ctx.body)
            .filter(|caps| secret_value_is_plausible(&caps[1]))
            .map(|caps| {
                let s = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                // Redact the value
                if let Some(eq_pos) = s.find('=').or_else(|| s.find(':')) {
                    format!("{}[REDACTED]", &s[..eq_pos + 1])
                } else {
                    "[REDACTED SECRET PATTERN]".to_string()
                }
            })
            .collect();

        if !secret_matches.is_empty() {
            findings.push(format!(
                "{} potential {}",
                secret_matches.len(),
                if secret_matches.len() == 1 {
                    "secret"
                } else {
                    "secrets"
                }
            ));
        }

        if findings.is_empty() {
            vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "No environment variable leaks detected".into(),
                description:
                    "No env var references or hardcoded secrets found in client-side HTML.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }]
        } else {
            // Secret-shaped values are critical leaks; unsubstituted env
            // references are warnings because no secret value was observed.
            let has_secrets = !secret_matches.is_empty();
            let (title, status, severity, description) = if has_secrets {
                (
                    "Client-visible credential-shaped literal in local preview".to_string(),
                    CheckStatus::Fail,
                    Severity::High,
                    format!(
                        "Found client-visible credential-shaped assignment pattern{} in the local preview HTML: {}. The value itself is redacted. Pattern shape does not verify that it is genuine, live, privileged, tracked, or deployed; test fixtures and public identifiers can match. If it is a real secret, any browser receiving this response can read it.",
                        if secret_matches.len() == 1 { "" } else { "s" },
                        findings.join("; ")
                    ),
                )
            } else {
                (
                    "Environment variable references in page HTML".to_string(),
                    CheckStatus::Warn,
                    Severity::Medium,
                    format!(
                        "Found {} in the local preview HTML. No secret value was seen; the literal reference text may mean the preview build did not substitute a value, may be example/debug text, or may be intentionally handled at runtime. This is a configuration and client-boundary review, not evidence of a leaked credential.",
                        findings.join("; ")
                    ),
                )
            };

            vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title,
                description,
                status,
                severity,
                fix_prompt: Some("Classify the matched value or reference without copying it into logs; rotate any genuine exposed credential and keep privileged values behind a server boundary.".into()),
                manual_fix: Some("Inspect the exact production artifact and deployment without pasting the value into tickets or telemetry. If the literal is a real credential that may have been shared, tracked, logged, bundled, or deployed, revoke/rotate it first, remove it from client output, and route privileged operations through an authenticated and authorized server endpoint. Clearly mark fake fixtures; reserve public-prefixed variables for values explicitly safe to disclose.".into()),
                raw_data: Some(serde_json::json!({
                    "env_refs": env_refs.iter().take(5).collect::<Vec<_>>(),
                    "secret_patterns": secret_matches.iter().take(5).collect::<Vec<_>>(),
                })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some(if has_secrets {
                    "A credential-shaped literal is present in local preview HTML, but validity, privilege, provenance, production deployment, and whether it is an intentional fixture were not established."
                } else {
                    "Only unresolved environment-reference text was observed; no value, production configuration, or server/client ownership was established."
                }.into()),
                why_it_matters: Some("Browser-delivered values are public to recipients. A genuine privileged credential in client output requires containment and rotation; an unresolved reference or fake fixture does not carry that same impact.".into()),
            }]
        }
    }
}
