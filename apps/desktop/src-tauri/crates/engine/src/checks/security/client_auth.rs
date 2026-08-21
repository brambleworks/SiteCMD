//! Detect client-side authentication and authorization logic.
//! Page source cannot prove whether the backend independently enforces the policy.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

static CLIENT_AUTH_PATTERNS: LazyLock<Vec<(&str, regex::Regex)>> = LazyLock::new(|| {
    vec![
        // isAdmin / isAuthenticated checks in JS
        (
            "client-side admin check",
            regex::Regex::new(r#"(?i)\b(isAdmin|is_admin|userRole)\s*[=!]==?\s*["']?(true|admin|superadmin)"#).unwrap(),
        ),
        // localStorage/sessionStorage auth token checks
        (
            "localStorage auth check",
            regex::Regex::new(r#"(?i)localStorage\.(getItem|setItem)\s*\(\s*["'](auth|token|jwt|session|user|isLoggedIn|isAuthenticated)["']"#).unwrap(),
        ),
        // Client-side role-based access in JS
        (
            "client-side role gating",
            regex::Regex::new(r#"(?i)\b(user\.role|currentUser\.role|auth\.role)\s*[=!]==?\s*["'](admin|editor|moderator|owner)"#).unwrap(),
        ),
        // window.location redirect as "auth guard"
        (
            "redirect-based auth guard",
            regex::Regex::new(r#"(?i)if\s*\(\s*!(?:isAuth|isLoggedIn|token|session|user)\b[^)]*\)\s*\{?\s*(?:window\.)?location"#).unwrap(),
        ),
    ]
});

pub struct ClientAuthCheck;

impl Check for ClientAuthCheck {
    fn id(&self) -> &str {
        "security.vibe.client_auth"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut found_patterns: Vec<String> = Vec::new();

        for (label, regex) in CLIENT_AUTH_PATTERNS.iter() {
            if regex.is_match(&ctx.body) {
                found_patterns.push(label.to_string());
            }
        }

        found_patterns.dedup();

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if found_patterns.is_empty() {
                // Neutral title on Pass: the finding-style title asserted the
                // problem even when nothing was detected.
                "Client-side access control patterns".into()
            } else {
                "Access checks appear to run only in the browser".into()
            },
            description: if found_patterns.is_empty() {
                "No obvious browser-only auth or role gating patterns were detected in the fetched page source.".into()
            } else {
                format!(
                    "Found {} {} that suggest auth or role checks may only be happening in the browser: {}. Reading tokens from localStorage is the standard pattern in single-page apps, and SiteCMD cannot observe the server side - this finding means browser-side checks were seen, not that server-side enforcement is missing.",
                    found_patterns.len(),
                    if found_patterns.len() == 1 { "pattern" } else { "patterns" },
                    found_patterns.join(", ")
                )
            },
            status: if found_patterns.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::High,
            fix_prompt: if found_patterns.is_empty() {
                None
            } else {
                Some(format!(
                    "Treat these client-side checks as UX only: {}. Make sure the server or API validates the session and the user's permissions on every protected request. If that server-side enforcement is already in place, no change is needed.",
                    found_patterns.join(", ")
                ))
            },
            manual_fix: if found_patterns.is_empty() {
                None
            } else {
                Some(
                    "1. Check auth and authorization on the server for every protected route or API action\n\
                     2. Keep client redirects only as a convenience, not as the real guard\n\
                     3. Return 401 or 403 from the backend when the user is not allowed\n\
                     4. If your data layer supports row-level rules, turn them on there too\n\
                     5. SiteCMD can only observe the browser side, so this finding will refire on re-scan; once server-side enforcement is confirmed, mark it as not applicable".into()
                )
            },
            raw_data: if found_patterns.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "patterns": found_patterns }))
            },
            confidence: if found_patterns.is_empty() {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if found_patterns.is_empty() {
                None
            } else {
                Some("SiteCMD can only observe browser-side code; it cannot verify whether the server independently enforces these checks, and browser-side token handling is normal in single-page apps.".into())
            },
            why_it_matters: if found_patterns.is_empty() {
                None
            } else {
                Some("Anything enforced only in the browser can usually be bypassed with dev tools, a crafted request, or a small script.".into())
            },
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
        let results = ClientAuthCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn pass_title_does_not_assert_the_finding() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let results = ClientAuthCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(
            !results[0].title.contains("appear to run only"),
            "{}",
            results[0].title
        );
    }

    #[test]
    fn fired_finding_is_needs_review_with_server_side_caveat() {
        // localStorage token reads are the standard SPA pattern and the
        // server side is unobservable, so High confidence was dishonest.
        let html = r#"<script>const token = localStorage.getItem("auth");</script>"#;
        let results = ClientAuthCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].confidence_reason.is_some());
        assert!(
            results[0].description.contains("cannot observe the server"),
            "{}",
            results[0].description
        );
        let fix = results[0].manual_fix.as_deref().unwrap();
        assert!(
            fix.contains("mark it as not applicable"),
            "manual_fix must tell the user how to clear an unobservable finding: {}",
            fix
        );
    }

    #[test]
    fn detects_is_admin_check() {
        let html = r#"<script>if (isAdmin === true) { showAdminPanel(); }</script>"#;
        let results = ClientAuthCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("admin check"));
    }

    #[test]
    fn detects_localstorage_auth() {
        let html = r#"<script>const token = localStorage.getItem("auth");</script>"#;
        let results = ClientAuthCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("localStorage"));
    }

    #[test]
    fn detects_role_gating() {
        let html = r#"<script>if (user.role === "admin") { renderAdmin(); }</script>"#;
        let results = ClientAuthCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("role"));
    }

    #[test]
    fn no_false_positive_on_login_form() {
        let html = r#"<form action="/login"><input type="text" name="username"><input type="password" name="password"></form>"#;
        let results = ClientAuthCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }
}
