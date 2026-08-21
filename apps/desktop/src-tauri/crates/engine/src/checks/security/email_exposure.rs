//! Find address-shaped strings in HTML and `mailto:` links.
//! This is a publication advisory; placeholders, asset paths, and known DSN hosts are excluded.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

pub struct EmailExposureCheck;

static EMAIL_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b[a-z0-9][a-z0-9._%+-]*@[a-z0-9][a-z0-9.-]*\.[a-z]{2,}\b").unwrap()
});

/// Asset-path suffixes that make an @-match a filename, not an address
/// (e.g. `image@2x.png` matches the email shape with domain `2x.png`).
const ASSET_SUFFIXES: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".avif", ".ico", ".css", ".js", ".mjs",
    ".woff", ".woff2", ".mp4", ".webm",
];

/// Exact domains (and their subdomains) that mean the address is a reserved
/// placeholder or SDK artifact, not a normal published inbox. Boundary-aware
/// matching matters: `domain.com` must not hide `mydomain.com`.
const IGNORED_DOMAINS: &[&str] = &[
    "example.com",
    "example.net",
    "example.org",
    "yourdomain.com",
    "yoursite.com",
    "your-domain.com",
    "domain.com",
    "sentry.io",
    "sentry-cdn.com",
    "schema.org",
    "w3.org",
];

fn is_ignored_domain(domain: &str) -> bool {
    IGNORED_DOMAINS.iter().any(|ignored| {
        domain == *ignored
            || domain
                .strip_suffix(ignored)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

fn collect_exposed_emails(body: &str) -> Vec<String> {
    let mut seen = Vec::new();
    for m in EMAIL_RE.find_iter(body) {
        let candidate = m.as_str();
        let candidate_lower = candidate.to_ascii_lowercase();
        if ASSET_SUFFIXES
            .iter()
            .any(|suffix| candidate_lower.ends_with(suffix))
        {
            continue;
        }
        let Some((_, domain)) = candidate_lower.split_once('@') else {
            continue;
        };
        if is_ignored_domain(domain) {
            continue;
        }
        if !seen
            .iter()
            .any(|observed: &String| observed.eq_ignore_ascii_case(candidate))
        {
            seen.push(candidate.to_string());
        }
    }
    seen
}

impl Check for EmailExposureCheck {
    fn id(&self) -> &str {
        "security.email_exposure"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let emails = collect_exposed_emails(&ctx.body);

        if emails.is_empty() {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Public email addresses in fetched HTML".into(),
                description: "No address-shaped string was found in the fetched HTML after excluding reserved placeholders, common asset-name lookalikes, and known error-reporting DSN hosts. Runtime-injected content and non-email contact identifiers were not inspected.".into(),
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

        let sample: Vec<&str> = emails.iter().take(3).map(|s| s.as_str()).collect();
        let sample_note = if emails.len() > sample.len() {
            format!(
                "{} and {} more",
                sample.join(", "),
                emails.len() - sample.len()
            )
        } else {
            sample.join(", ")
        };

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if emails.len() == 1 {
                "Public email address in fetched HTML".into()
            } else {
                format!("{} public email addresses in fetched HTML", emails.len())
            },
            description: format!(
                "The fetched HTML contains {} address-shaped string{} ({}). Anything delivered to a browser can be collected automatically, but publishing a support, sales, or security inbox may be intentional. Review whether each address is meant to be public and whether its mailbox controls match that role.",
                emails.len(),
                if emails.len() == 1 { "" } else { "es" },
                sample_note,
            ),
            status: CheckStatus::Warn,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: Some("For an address that should not be public, remove it from the delivered page and route contact through an appropriately secured form or authenticated channel. For an address that should remain public, prefer a dedicated role inbox with filtering, monitoring, and a rotation/escalation plan, and keep an accessible `mailto:` link when email is the intended action. Do not rely on JavaScript obfuscation: browser-delivered text remains collectible and obfuscation can make the contact path less accessible.".into()),
            raw_data: Some(serde_json::json!({
                "count": emails.len(),
                "emails": emails,
            })),
            confidence: crate::checks::IssueConfidence::Confirmed,
            confidence_reason: Some("The address-shaped strings are directly observed in fetched HTML, but SiteCMD cannot determine whether they are intentional role inboxes, personal addresses, dead examples, or protected by adequate mailbox filtering and workflow controls.".into()),
            why_it_matters: Some(
                "A public inbox can receive more unsolicited mail and targeted phishing. The practical risk depends on whether the address is intentionally public, who monitors it, and which filtering, authentication, and incident-handling controls protect it.".into(),
            ),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_exposed_emails, EmailExposureCheck};
    use crate::checks::{Check, CheckStatus, IssueConfidence, PageContext};

    fn ctx(body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: http::header::HeaderMap::new(),
            status_code: 200,
            body: body.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn finds_plaintext_and_mailto_addresses_once() {
        let body = r#"<a href="mailto:hello@acme.dev">hello@acme.dev</a> or sales@acme.dev"#;
        let found = collect_exposed_emails(body);
        assert_eq!(found, vec!["hello@acme.dev", "sales@acme.dev"]);
    }

    #[test]
    fn ignores_retina_asset_names() {
        let found =
            collect_exposed_emails(r#"<img src="/img/logo@2x.png"> <img src="hero@3x.webp">"#);
        assert!(found.is_empty(), "asset names matched: {:?}", found);
    }

    #[test]
    fn ignores_sentry_dsn_and_placeholders() {
        let body =
            "dsn: 'abc123@o4505.ingest.sentry.io' contact user@example.com or admin@yourdomain.com";
        let found = collect_exposed_emails(body);
        assert!(found.is_empty(), "matched: {:?}", found);
    }

    #[test]
    fn placeholder_filter_does_not_hide_a_real_domain_containing_domain_dot_com() {
        let found = collect_exposed_emails("Contact Security@MyDomain.com");
        assert_eq!(found, vec!["Security@MyDomain.com"]);
    }

    #[test]
    fn public_address_is_confirmed_evidence_but_still_a_contextual_advisory() {
        let result = EmailExposureCheck
            .run(&ctx(
                r#"<a href="mailto:support@acme.dev">Email support</a>"#,
            ))
            .remove(0);

        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.confidence, IssueConfidence::Confirmed);
        assert_eq!(result.title, "Public email address in fetched HTML");
        assert!(result.description.contains("may be intentional"));
        let fix = result.manual_fix.as_deref().unwrap_or_default();
        assert!(fix.contains("Do not rely on JavaScript obfuscation"));
        assert!(fix.contains("accessible"));
        assert!(fix.contains("mailto:"));
    }

    #[test]
    fn clean_page_passes() {
        let found = collect_exposed_emails("<p>Contact us through the form below.</p>");
        assert!(found.is_empty());
    }
}
