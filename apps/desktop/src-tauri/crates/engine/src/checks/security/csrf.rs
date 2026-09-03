//! Flags POST forms without recognizable CSRF token markup for review.

use crate::checks::html_attrs::attr_value;
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

// Match <form... method="post"...>. Quotes are optional: minified HTML
// legally writes method=post, and the quoted-only pattern skipped those
// forms entirely.
static POST_FORM_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?si)<form[^>]*method\s*=\s*["']?post\b["']?[^>]*>.*?</form>"#).unwrap()
});

// CSRF token indicators inside a form (quote-agnostic, same reason)
static CSRF_INDICATORS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        // Hidden input with csrf/token in name
        regex::Regex::new(r#"(?i)<input[^>]*type\s*=\s*["']?hidden\b["']?[^>]*name\s*=\s*["']?[^"'\s>]*(?:csrf|_token|xsrf|authenticity_token|__RequestVerificationToken)"#).unwrap(),
        // Hidden input with csrf/token in name (reversed attribute order)
        regex::Regex::new(r#"(?i)<input[^>]*name\s*=\s*["']?[^"'\s>]*(?:csrf|_token|xsrf|authenticity_token|__RequestVerificationToken)[^"'\s>]*["']?[^>]*type\s*=\s*["']?hidden\b"#).unwrap(),
        // Meta tag with csrf token (Rails, Laravel pattern)
        regex::Regex::new(r#"(?i)<meta[^>]*name\s*=\s*["']?csrf-token\b"#).unwrap(),
    ]
});

pub struct CsrfCheck;

/// True when the form's resolved action leaves this registrable site. A
/// same-site token still applies across a site's own subdomains, so the test
/// is `security.form_action_hijack`'s `same_site` predicate rather than an
/// exact host match: anything this returns true for is a destination that
/// check grades, and nothing falls between the two.
fn posts_to_another_site(ctx: &PageContext, form: &str) -> bool {
    let Some(action) = attr_value(form, "action") else {
        return false;
    };
    let action = crate::checks::html_attrs::decode_url_character_references(action.trim());
    if action.is_empty() {
        return false;
    }
    let Ok(resolved) = ctx.url.join(&action) else {
        return false;
    };
    let action_host = resolved.host_str().unwrap_or_default();
    !action_host.is_empty()
        && !crate::checks::security::forms::same_site(
            action_host,
            ctx.url.host_str().unwrap_or_default(),
        )
}

impl Check for CsrfCheck {
    fn id(&self) -> &str {
        "security.vibe.csrf"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let (cross_host_forms, post_forms): (Vec<&str>, Vec<&str>) = POST_FORM_RE
            .find_iter(&ctx.body)
            .map(|m| m.as_str())
            .partition(|form| posts_to_another_site(ctx, form));

        if post_forms.is_empty() {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "CSRF Protection".into(),
                description: if cross_host_forms.is_empty() {
                    "No POST forms found on this page.".into()
                } else {
                    format!(
                        "No POST form submitting to this site was found on this page. {} POST {} to another site, where a same-site token cannot apply; security.form_action_hijack grades those destinations.",
                        cross_host_forms.len(),
                        if cross_host_forms.len() == 1 {
                            "form submits"
                        } else {
                            "forms submit"
                        }
                    )
                },
                status: CheckStatus::Pass,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        // Check if there's a page-level CSRF meta tag
        let has_meta_csrf = CSRF_INDICATORS
            .last()
            .map(|re| re.is_match(&ctx.body))
            .unwrap_or(false);

        let mut unprotected_count = 0;
        for form in &post_forms {
            let has_csrf_field = CSRF_INDICATORS
                .iter()
                .take(2) // only the hidden input patterns
                .any(|re| re.is_match(form));

            if !has_csrf_field && !has_meta_csrf {
                unprotected_count += 1;
            }
        }

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if unprotected_count == 0 {
                "POST forms include recognized CSRF token markers".into()
            } else {
                "POST forms lack a recognized CSRF token marker".into()
            },
            description: if unprotected_count == 0 {
                format!(
                    "{} a recognized hidden-field or page-level CSRF token marker. This confirms the markup signal, not server-side validation, token binding, entropy, expiry, or authorization behavior.",
                    if post_forms.len() == 1 {
                        "The 1 detected POST form has".to_string()
                    } else {
                        format!("All {} detected POST forms have", post_forms.len())
                    }
                )
            } else {
                format!(
                    "{} of {} detected POST {} no recognized CSRF token marker in their markup. This does not establish missing protection: the endpoint may be unauthenticated or session-less, or middleware may validate a custom header, Origin/Referer, or another anti-forgery signal. If a state-changing endpoint relies on ambient cookies and no effective server-side defense exists, a cross-site page may be able to trigger an action with the user's session.",
                    unprotected_count,
                    post_forms.len(),
                    if post_forms.len() == 1 { "form has" } else { "forms have" }
                )
            },
            status: if unprotected_count == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::High,
            fix_prompt: if unprotected_count == 0 {
                None
            } else {
                Some(format!(
                    "{} POST {} on this page {} a recognized markup token. Trace the server-side endpoint and add a maintained anti-forgery control only if an equivalent cookie-session defense is not already enforced.",
                    unprotected_count,
                    if unprotected_count == 1 { "form" } else { "forms" },
                    if unprotected_count == 1 { "lacks" } else { "lack" }
                ))
            },
            manual_fix: if unprotected_count == 0 {
                None
            } else {
                Some(
                    "1. Trace the form action and confirm whether authentication uses ambient cookies and whether framework or edge middleware already enforces CSRF protection\n\
                     2. Where a gap remains, use a maintained synchronizer-token or signed double-submit implementation and validate it server-side before side effects\n\
                     3. For same-origin JavaScript requests, send the implementation's token in its configured custom header; for ordinary forms, include the generated hidden field\n\
                     4. Use an exact public-origin check where appropriate, and set an intentional SameSite cookie policy as defense in depth rather than the sole control\n\
                     5. SiteCMD can only see the form markup, so header- or cookie-based defenses will not clear this finding on re-scan; once protection is enforced server-side, mark it as not applicable".into()
                )
            },
            raw_data: if unprotected_count == 0 {
                None
            } else {
                Some(serde_json::json!({
                    "total_post_forms": post_forms.len(),
                    "unprotected_forms": unprotected_count,
                }))
            },
            confidence: if unprotected_count == 0 {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if unprotected_count == 0 {
                None
            } else {
                Some("Only form markup is observable: middleware, custom-header tokens, strict origin checks, cookie SameSite behavior, and session-less endpoints can change the CSRF posture without a recognized hidden field.".into())
            },
            why_it_matters: if unprotected_count == 0 {
                None
            } else {
                Some("When a state-changing endpoint accepts ambient browser credentials, a missing anti-forgery boundary can let another origin cause the user's browser to submit an unintended action.".into())
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
    fn no_forms_passes() {
        let html = "<html><body><p>No forms here</p></body></html>";
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn get_form_ignored() {
        let html = r#"<form method="get" action="/search"><input name="q"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn post_form_with_csrf_passes() {
        let html = r#"<form method="post" action="/submit"><input type="hidden" name="csrf_token" value="abc123"><input type="text" name="name"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("recognized hidden-field or page-level CSRF token marker"));
        assert!(!results[0].description.contains("have CSRF protection"));
    }

    #[test]
    fn post_form_without_csrf_warns() {
        let html = r#"<form method="post" action="/submit"><input type="text" name="name"><button>Submit</button></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].title.contains("recognized"));
        assert!(results[0].description.contains("does not establish"));
    }

    #[test]
    fn csrf_warn_is_needs_review_with_markup_only_caveat() {
        let html = r#"<form method="post" action="/submit"><input type="text" name="name"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].confidence_reason.is_some());
        let fix = results[0].manual_fix.as_deref().unwrap();
        assert!(
            fix.contains("mark it as not applicable"),
            "manual_fix must explain how to clear a markup-invisible defense: {}",
            fix
        );
    }

    #[test]
    fn csrf_pass_branches_keep_high_confidence() {
        let html = r#"<form method="post" action="/submit"><input type="hidden" name="csrf_token" value="abc123"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].confidence, crate::checks::IssueConfidence::High);
    }

    #[test]
    fn meta_csrf_tag_covers_all_forms() {
        let html = r#"<head><meta name="csrf-token" content="abc123"></head>
        <body><form method="post" action="/submit"><input type="text" name="name"></form></body>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn laravel_token_detected() {
        let html = r#"<form method="post" action="/submit"><input type="hidden" name="_token" value="abc123"><input type="text" name="name"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn unquoted_post_form_without_csrf_warns() {
        let html = r#"<form method=post action=/submit><input type=text name=email><button>Go</button></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Warn,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn unquoted_post_form_with_unquoted_csrf_input_passes() {
        let html = r#"<form method=post action=/submit><input type=hidden name=csrf_token value=abc123></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn a_form_posting_to_this_site_own_subdomain_is_still_graded() {
        // The page's cookies and token are same-site with api.example.com, so
        // this is a real CSRF surface. security.form_action_hijack treats it as
        // first-party too, which is exactly why this check must keep it.
        let html = r#"<form method="post" action="https://api.example.com/account"><input type="text" name="handle"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Warn,
            "{}",
            results[0].description
        );
        let evidence = results[0].raw_data.as_ref().expect("form evidence");
        assert_eq!(evidence["total_post_forms"], 1);
        assert_eq!(evidence["unprotected_forms"], 1);
    }

    #[test]
    fn a_form_posting_to_another_site_is_not_this_site_csrf_surface() {
        let html = r#"<form action="https://buttondown.email/api/emails/embed-subscribe/astro" method="post" target="popupwindow"><input type="email" name="email"><button>Subscribe</button></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
        assert!(
            results[0].description.contains("another site"),
            "{}",
            results[0].description
        );
        assert!(results[0].raw_data.is_none());
    }

    #[test]
    fn a_same_host_form_beside_a_cross_host_form_is_still_graded() {
        let html = r#"<form action="https://buttondown.email/subscribe" method="post"><input type="email" name="email"></form>
        <form action="/account" method="post"><input type="text" name="handle"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        let evidence = results[0].raw_data.as_ref().expect("form evidence");
        assert_eq!(evidence["total_post_forms"], 1);
        assert_eq!(evidence["unprotected_forms"], 1);
    }

    #[test]
    fn the_single_form_pass_description_reads_as_singular() {
        let html = r#"<form method="post" action="/submit"><input type="hidden" name="csrf_token" value="abc123"></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert!(
            results[0]
                .description
                .starts_with("The 1 detected POST form has"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn postal_code_form_is_not_a_post_form() {
        // `method=poster` or similar words must not match `post`.
        let html = r#"<form method="get" action="/s" data-method=posting><input name=q></form>"#;
        let results = CsrfCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }
}
