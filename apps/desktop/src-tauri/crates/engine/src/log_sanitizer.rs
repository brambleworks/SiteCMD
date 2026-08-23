use std::sync::LazyLock;

/// Defense-in-depth redaction patterns for UI and log errors.
static SECRET_PATTERNS: LazyLock<Vec<(regex::Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (
            regex::Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._\-]{8,}").unwrap(),
            "Bearer ***",
        ),
        (
            regex::Regex::new(r"\bya29\.[A-Za-z0-9._\-]{8,}").unwrap(),
            "ya29.***",
        ),
        (
            regex::Regex::new(r"\b1//[A-Za-z0-9._\-]{8,}").unwrap(),
            "1//***",
        ),
        (
            regex::Regex::new(r"(?i)\b(?:gh[pousr]_|github_pat_)[A-Za-z0-9_]{12,}").unwrap(),
            "gh_***",
        ),
        (
            regex::Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
            "AKIA***",
        ),
        (
            regex::Regex::new(r"(?i)\bsk_(?:live|test)_[A-Za-z0-9]{12,}").unwrap(),
            "sk_***",
        ),
        // Basic-auth userinfo embedded in a URL (scheme://user:pass@host).
        (
            regex::Regex::new(r"://[^/\s:@]+:[^/\s@]+@").unwrap(),
            "://***:***@",
        ),
    ]
});

static EVIDENCE_URL: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)\bhttps?://[^\s<>\"']+"#).expect("static evidence URL regex")
});

static EVIDENCE_EMAIL: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b")
        .expect("static evidence email regex")
});

static EVIDENCE_SENSITIVE_PAIR: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"(?i)\b(?P<key>access[_-]?token|api[_-]?key|authorization|client[_-]?secret|password|secret|token)\s*(?P<separator>[:=])\s*(?P<value>\"[^\"\r\n]*\"|'[^'\r\n]*'|[^\s,;]+)"#,
    )
    .expect("static sensitive evidence pair regex")
});

/// Scrub known secret/token formats from an arbitrary string. A backstop on the
/// error path so a stray token in an integration error never reaches the UI or
/// the logs verbatim.
pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();
    for (pattern, replacement) in SECRET_PATTERNS.iter() {
        out = pattern.replace_all(&out, *replacement).into_owned();
    }
    out
}

pub fn log_safe_url_target(raw_url: &str) -> String {
    let Ok(parsed) = url::Url::parse(raw_url) else {
        return "[invalid-url]".to_string();
    };
    let Some(host) = parsed.host_str() else {
        return format!("{}://[unknown-host]", parsed.scheme());
    };
    let port = parsed
        .port()
        .map(|value| format!(":{value}"))
        .unwrap_or_default();
    let path_hint = if parsed.path().is_empty() || parsed.path() == "/" {
        ""
    } else {
        "/[path]"
    };
    format!("{}://{}{}{}", parsed.scheme(), host, port, path_hint)
}

/// Preserve a safe path hint for issue evidence; remove credentials and tokens.
pub fn evidence_safe_page_url(raw_url: &str) -> String {
    sanitize_page_url(raw_url, false)
}

/// Same policy, except long letter-and-digit segments are kept when the URL
/// is on the scanned site's own origin: those are its asset names, which the
/// fix needs, while a foreign host may be a CDN carrying signed path tokens.
pub fn evidence_safe_page_url_for_site(raw_url: &str, site_origin: Option<&str>) -> String {
    let same_origin = site_origin.is_some_and(|origin| {
        // Reparse `site_origin` too so a default port embedded in either side
        // folds away the same way; an unparsable or opaque origin can never match.
        let Ok(site_url) = url::Url::parse(origin) else {
            return false;
        };
        let site_origin = site_url.origin();
        site_origin.is_tuple()
            && url::Url::parse(raw_url)
                .map(|parsed| {
                    parsed.origin().ascii_serialization() == site_origin.ascii_serialization()
                })
                .unwrap_or(false)
    });
    sanitize_page_url(raw_url, same_origin)
}

fn sanitize_page_url(raw_url: &str, keep_long_segments: bool) -> String {
    let Ok(parsed) = url::Url::parse(raw_url) else {
        return "[invalid-url]".to_string();
    };
    let Some(host) = parsed.host_str() else {
        return format!("{}://[unknown-host]", parsed.scheme());
    };
    let port = parsed
        .port()
        .map(|value| format!(":{value}"))
        .unwrap_or_default();
    let mut redact_next_segment = false;
    let safe_path = parsed
        .path()
        .split('/')
        .map(|segment| {
            if segment.is_empty() {
                return segment.to_string();
            }
            if redact_next_segment {
                redact_next_segment = false;
                return "[redacted]".to_string();
            }
            let lower = segment.to_ascii_lowercase();
            // These routes commonly carry a bearer value in the next segment.
            redact_next_segment = matches!(
                lower.as_str(),
                "reset"
                    | "password-reset"
                    | "verify"
                    | "verification"
                    | "magic-link"
                    | "magic"
                    | "invite"
                    | "invitation"
                    | "activate"
                    | "activation"
                    | "confirm"
                    | "confirmation"
                    | "unsubscribe"
            );
            let has_letters = segment.bytes().any(|byte| byte.is_ascii_alphabetic());
            let has_digits = segment.bytes().any(|byte| byte.is_ascii_digit());
            let token_like =
                !keep_long_segments && segment.len() >= 32 && has_letters && has_digits;
            let retina_asset = segment
                .rsplit_once('@')
                .and_then(|(_, suffix)| suffix.split_once('.'))
                .is_some_and(|(scale, extension)| {
                    scale.strip_suffix('x').is_some_and(|digits| {
                        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
                    }) && matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "svg"
                    )
                });
            let email_like = segment.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !retina_asset
            });
            if token_like || email_like || lower.contains("%40") {
                "[redacted]".to_string()
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    bounded_evidence_url(&redact_secrets(&format!(
        "{}://{}{}{}",
        parsed.scheme(),
        host,
        port,
        safe_path
    )))
}

/// Sanitize either an absolute URL or a relative URL-valued HTML attribute
/// while retaining enough path information to locate the declaration. Unlike
/// `evidence_safe_page_url`, this also handles protocol-relative references,
/// non-network schemes, and malformed/relative strings.
pub fn evidence_safe_url_reference(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return "[empty-url]".to_string();
    }

    if trimmed.starts_with("//") {
        let safe = evidence_safe_page_url(&format!("https:{trimmed}"));
        return safe.strip_prefix("https:").unwrap_or(&safe).to_string();
    }

    if let Ok(parsed) = url::Url::parse(trimmed) {
        return if matches!(parsed.scheme(), "http" | "https") {
            evidence_safe_page_url(trimmed)
        } else {
            bounded_evidence_url(&format!("{}:[redacted]", parsed.scheme()))
        };
    }

    if let Ok(base) = url::Url::parse("https://evidence.invalid/") {
        if let Ok(resolved) = base.join(trimmed) {
            let safe = evidence_safe_page_url(resolved.as_str());
            if let Some(relative) = safe.strip_prefix("https://evidence.invalid") {
                return if trimmed.starts_with('/') {
                    relative.to_string()
                } else {
                    relative.strip_prefix('/').unwrap_or(relative).to_string()
                };
            }
        }
    }

    let without_query = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    bounded_evidence_url(&redact_issue_evidence(without_query))
}

/// Remove personal data, credentials, sensitive paths, and URL secrets from diagnostics.
pub fn redact_issue_evidence(input: &str) -> String {
    // Redact token formats before key-value parsing can split bearer credentials.
    let input = redact_secrets(input);
    let urls_redacted = EVIDENCE_URL
        .replace_all(&input, |captures: &regex::Captures<'_>| {
            let matched = captures.get(0).expect("whole URL capture").as_str();
            let url = matched.trim_end_matches(['.', ',', ';', ':', ')', ']', '}']);
            let punctuation = &matched[url.len()..];
            format!("{}{}", evidence_safe_page_url(url), punctuation)
        })
        .into_owned();
    let emails_redacted = EVIDENCE_EMAIL
        .replace_all(&urls_redacted, "[email]")
        .into_owned();
    let pairs_redacted = EVIDENCE_SENSITIVE_PAIR
        .replace_all(&emails_redacted, |captures: &regex::Captures<'_>| {
            let key = captures
                .name("key")
                .map(|value| value.as_str())
                .unwrap_or("");
            let separator = captures
                .name("separator")
                .map(|value| value.as_str())
                .unwrap_or("");
            let value = captures
                .name("value")
                .map(|value| value.as_str())
                .unwrap_or("");
            // For an unquoted assignment at the end of prose, the regex can
            // consume sentence punctuation. Preserve that punctuation while
            // replacing the value itself. Quoted values keep punctuation
            // outside the capture already.
            let punctuation = if value.starts_with(['"', '\'']) {
                ""
            } else {
                let trimmed = value.trim_end_matches(['.', '!', '?', ')', ']', '}']);
                &value[trimmed.len()..]
            };
            format!("{key}{separator}[redacted]{punctuation}")
        })
        .into_owned();
    redact_secrets(&pairs_redacted)
}

/// Maximum characters persisted for one sanitized URL/reference in issue
/// evidence. Long data URLs and adversarial path segments must not create
/// unbounded scan rows even after credentials and query values are removed.
pub const ISSUE_EVIDENCE_URL_MAX_CHARS: usize = 500;

/// Maximum characters persisted for one free-form diagnostic or response
/// header in issue evidence. These values originate outside SiteCMD and can
/// otherwise create oversized issue rows or retain irrelevant response data.
pub const ISSUE_EVIDENCE_TEXT_MAX_CHARS: usize = 1_000;

/// Apply the full issue-evidence redaction policy and cap one free-form field.
/// Use this for network, parser, DNS, TLS, and response-header diagnostics;
/// callers that hold a URL/reference should prefer the URL-specific helpers.
pub fn bounded_issue_evidence(input: &str) -> String {
    let redacted = redact_issue_evidence(input);
    bounded_evidence(&redacted, ISSUE_EVIDENCE_TEXT_MAX_CHARS)
}

fn bounded_evidence_url(value: &str) -> String {
    bounded_evidence(value, ISSUE_EVIDENCE_URL_MAX_CHARS)
}

fn bounded_evidence(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut bounded = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        bounded.push('…');
    }
    bounded
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_issue_evidence, evidence_safe_page_url, evidence_safe_page_url_for_site,
        evidence_safe_url_reference, log_safe_url_target, redact_issue_evidence,
    };

    #[test]
    fn log_safe_url_target_removes_query_fragment_and_path_tokens() {
        let redacted = log_safe_url_target(
            "https://example.com/reset/secret-token?token=abc123&email=person@example.com#frag",
        );

        assert_eq!(redacted, "https://example.com/[path]");
        assert!(!redacted.contains("secret-token"));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("person@example.com"));
        assert!(!redacted.contains("frag"));
    }

    #[test]
    fn log_safe_url_target_keeps_localhost_port_without_query() {
        assert_eq!(
            log_safe_url_target("http://127.0.0.1:4321/?preview_token=abc"),
            "http://127.0.0.1:4321"
        );
    }

    #[test]
    fn evidence_safe_page_url_keeps_path_but_removes_secrets() {
        let safe = evidence_safe_page_url(
            "https://user:pass@example.com/docs/abc12345678901234567890123456789?token=secret#part",
        );
        assert_eq!(safe, "https://example.com/docs/[redacted]");
        assert!(!safe.contains("user"));
        assert!(!safe.contains("pass"));
        assert!(!safe.contains("secret"));
        assert!(!safe.contains("part"));
    }

    #[test]
    fn evidence_safe_page_url_redacts_short_tokens_after_sensitive_routes() {
        let safe = evidence_safe_page_url(
            "https://example.com/account/reset/short-token?continue=/settings",
        );
        assert_eq!(safe, "https://example.com/account/reset/[redacted]");
        assert!(!safe.contains("short-token"));
        assert!(!safe.contains("settings"));
    }

    #[test]
    fn evidence_safe_page_url_keeps_retina_asset_names_but_redacts_email_paths() {
        assert_eq!(
            evidence_safe_page_url("https://cdn.example.com/img/logo@2x.png?token=secret"),
            "https://cdn.example.com/img/logo@2x.png"
        );
        assert_eq!(
            evidence_safe_page_url("https://example.com/users/person@example.com/profile"),
            "https://example.com/users/[redacted]/profile"
        );
    }

    #[test]
    fn redact_secrets_masks_known_token_formats() {
        use super::redact_secrets;
        assert_eq!(
            redact_secrets("token refresh failed: 1//0gabcdEFGHijkl_mnop"),
            "token refresh failed: 1//***"
        );
        assert_eq!(
            redact_secrets("github gho_abcdefghijklmnopqrstuvwxyz rejected"),
            "github gh_*** rejected"
        );
        let basic = redact_secrets("connect https://user:s3cretPass@db.example.com failed");
        assert!(
            basic.contains("https://***:***@db.example.com"),
            "got {basic}"
        );
        assert!(!basic.contains("s3cretPass"), "secret leaked: {basic}");
    }

    #[test]
    fn issue_evidence_redacts_urls_emails_and_sensitive_pairs() {
        let safe = redact_issue_evidence(
            "Failed for person@example.com at https://example.com/reset/short-token?token=supersecret password=hunter2.",
        );
        assert_eq!(
            safe,
            "Failed for [email] at https://example.com/reset/[redacted] password=[redacted]."
        );
    }

    #[test]
    fn issue_evidence_redacts_authorization_bearer_token() {
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.signature";
        let detail = format!("request failed with Authorization: Bearer {token}");

        for redacted in [
            redact_issue_evidence(&detail),
            bounded_issue_evidence(&detail),
        ] {
            assert!(
                !redacted.contains(token),
                "bearer token leaked into issue evidence: {redacted}"
            );
            assert!(
                !redacted.contains("payload.signature"),
                "token tail leaked: {redacted}"
            );
        }
    }

    #[test]
    fn bounded_issue_evidence_scrubs_and_caps_untrusted_diagnostics() {
        let unsafe_detail = format!(
            "request to https://person:pass@example.com/reset/short-token?api_key=secret for person@example.com failed: password=hunter2 {}",
            "x".repeat(1_000)
        );
        let safe = bounded_issue_evidence(&unsafe_detail);

        assert!(safe.chars().count() <= super::ISSUE_EVIDENCE_TEXT_MAX_CHARS + 1);
        assert!(safe.ends_with('…'));
        for secret in [
            "person:pass",
            "short-token",
            "secret",
            "person@example.com",
            "hunter2",
        ] {
            assert!(!safe.contains(secret), "secret leaked in: {safe}");
        }
        assert!(safe.contains("https://example.com/reset/[redacted]"));
    }

    #[test]
    fn same_origin_asset_names_survive_but_sensitive_routes_and_foreign_tokens_do_not() {
        let site = Some("https://sitecmd.com");
        let own_asset = "https://sitecmd.com/images/screenshots/problem/dashboard-health-score-before-fix-2026.png";
        assert_eq!(evidence_safe_page_url_for_site(own_asset, site), own_asset);

        let own_reset = "https://sitecmd.com/account/reset/abcdefghijklmnopqrstuvwxyz0123456789";
        assert_eq!(
            evidence_safe_page_url_for_site(own_reset, site),
            "https://sitecmd.com/account/reset/[redacted]"
        );

        let foreign = "https://cdn.example.com/images/screenshots/problem/dashboard-health-score-before-fix-2026.png";
        assert_eq!(
            evidence_safe_page_url_for_site(foreign, site),
            "https://cdn.example.com/images/screenshots/problem/[redacted]"
        );
        assert_eq!(
            evidence_safe_page_url_for_site(own_asset, None),
            "https://sitecmd.com/images/screenshots/problem/[redacted]"
        );
    }

    #[test]
    fn evidence_safe_page_url_for_site_handles_origin_edge_cases() {
        let site = Some("https://sitecmd.com");

        // (a) A different subdomain is never the scanned site's origin, so it
        // gets exactly the treatment the non-site-aware sanitizer already gave
        // it (nothing here is long enough to redact either way).
        let subdomain_url = "https://cdn.sitecmd.com/a/b.js";
        assert_eq!(
            evidence_safe_page_url_for_site(subdomain_url, site),
            evidence_safe_page_url(subdomain_url),
            "a different subdomain must not receive the same-origin exemption"
        );
        assert_eq!(
            evidence_safe_page_url_for_site(subdomain_url, site),
            "https://cdn.sitecmd.com/a/b.js"
        );

        // (b) An explicit default port on either side folds away, because
        // `site_origin` is reparsed and re-serialized the same way as the URL.
        let long_name = "https://sitecmd.com:443/assets/dashboard-health-score-a1b2c3d4e5f67890.js";
        let bare_long_name =
            "https://sitecmd.com/assets/dashboard-health-score-a1b2c3d4e5f67890.js";
        assert_eq!(
            evidence_safe_page_url_for_site(long_name, site),
            bare_long_name
        );
        // The reverse also folds: an explicit default port embedded in
        // `site_origin` itself normalizes away before the comparison.
        assert_eq!(
            evidence_safe_page_url_for_site(
                "https://sitecmd.com/assets/app.js",
                Some("https://sitecmd.com:443"),
            ),
            "https://sitecmd.com/assets/app.js"
        );
        assert_eq!(
            evidence_safe_page_url_for_site(bare_long_name, Some("https://sitecmd.com:443")),
            bare_long_name
        );

        // An unparsable site_origin can never be same-origin, so it behaves
        // exactly like passing None.
        assert_eq!(
            evidence_safe_page_url_for_site(bare_long_name, Some("not a url")),
            evidence_safe_page_url_for_site(bare_long_name, None)
        );

        // (c) Userinfo, query, and fragment are stripped even for a
        // same-origin URL; only the path survives.
        assert_eq!(
            evidence_safe_page_url_for_site(
                "https://user:pw@sitecmd.com/assets/app.js?token=abc#frag",
                site,
            ),
            "https://sitecmd.com/assets/app.js"
        );

        // (d) Scheme and host compare case-insensitively (URL parsing
        // lowercases both), but the retained path keeps its original case.
        assert_eq!(
            evidence_safe_page_url_for_site("HTTPS://SITECMD.COM/Assets/App.js", site),
            "https://sitecmd.com/Assets/App.js"
        );
    }

    #[test]
    fn url_reference_sanitizer_handles_relative_protocol_and_opaque_values() {
        assert_eq!(
            evidence_safe_url_reference("/account/reset/short-token?token=secret#part"),
            "/account/reset/[redacted]"
        );
        assert_eq!(
            evidence_safe_url_reference("//cdn.example.com/app.js?key=secret"),
            "//cdn.example.com/app.js"
        );
        assert_eq!(
            evidence_safe_url_reference("data:image/png;base64,private"),
            "data:[redacted]"
        );
        assert_eq!(
            evidence_safe_url_reference("assets/card-1.png?token=secret"),
            "assets/card-1.png",
            "relative evidence should remain grep-compatible with source markup"
        );
    }
}
