use super::*;

fn grade_exact(body: &str) -> (CheckStatus, String, String, Vec<String>) {
    grade_with_profile(
        body,
        "https://example.com/.well-known/security.txt",
        "https://example.com/.well-known/security.txt",
        body.len(),
        body.lines().count(),
        body.lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or_default(),
        true,
    )
}

fn grade_with_profile(
    body: &str,
    requested_url: &str,
    final_url: &str,
    body_bytes: usize,
    line_count: usize,
    max_line_chars: usize,
    utf8_valid: bool,
) -> (CheckStatus, String, String, Vec<String>) {
    let fields = parse_security_txt(
        body,
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    let (status, _, title, description, _, issues) = grade_security_txt(
        &fields,
        "/.well-known/security.txt",
        requested_url,
        final_url,
        Some("text/plain; charset=utf-8"),
        false,
        false,
        body_bytes,
        line_count,
        max_line_chars,
        utf8_valid,
    );
    (status, title, description, issues)
}

#[test]
fn expiration_more_than_one_year_ahead_is_a_freshness_warning() {
    let body = "Contact: mailto:s@example.com\nExpires: 2099-01-01T00:00:00Z\n";
    let fields = parse_security_txt(
        body,
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    assert_eq!(fields.expires_too_far, Some(true));
    let (status, title, description, issues) = grade_exact(body);
    assert_eq!(status, CheckStatus::Warn);
    assert!(title.contains("more than one year"), "{title}");
    assert!(description.contains("recommends"), "{description}");
    assert_eq!(issues, ["expires_too_far"]);
}

#[test]
fn redirect_and_canonical_helpers_distinguish_trust_boundaries() {
    assert!(!redirect_crosses_origin(
        "https://example.com/.well-known/security.txt",
        "https://example.com/security/contact.txt"
    ));
    assert!(redirect_crosses_origin(
        "https://example.com/.well-known/security.txt",
        "https://reports.example.net/security.txt"
    ));
    assert!(canonical_names_final_url(
        &["https://example.com/.well-known/security.txt".into()],
        "https://example.com/.well-known/security.txt"
    ));
    assert!(!canonical_names_final_url(
        &["https://other.example/security.txt".into()],
        "https://example.com/.well-known/security.txt"
    ));
}

#[test]
fn cross_origin_redirect_and_canonical_mismatch_are_reported() {
    let expiration = (chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
        + chrono::Duration::days(180))
    .to_rfc3339();
    let body = format!(
        "Contact: mailto:s@example.com\nExpires: {expiration}\nCanonical: https://other.example/security.txt\n"
    );
    let (_, title, description, issues) = grade_with_profile(
        &body,
        "https://example.com/.well-known/security.txt",
        "https://reports.example.net/security.txt",
        body.len(),
        body.lines().count(),
        64,
        true,
    );
    assert!(title.contains("validation issues"), "{title}");
    assert!(description.contains("different origin"), "{description}");
    assert_eq!(issues, ["cross_origin_redirect", "canonical_mismatch"]);
}

#[test]
fn utf8_and_conservative_parser_limits_are_distinct_evidence() {
    let expiration = (chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
        + chrono::Duration::days(180))
    .to_rfc3339();
    let body = format!("Contact: mailto:s@example.com\nExpires: {expiration}\n");
    let (status, _, description, issues) = grade_with_profile(
        &body,
        "https://example.com/.well-known/security.txt",
        "https://example.com/.well-known/security.txt",
        CONSERVATIVE_BODY_BYTES + 1,
        2,
        64,
        false,
    );
    assert_eq!(status, CheckStatus::Warn);
    assert!(description.contains("not valid UTF-8"), "{description}");
    assert!(
        description.contains("not itself a MUST-level"),
        "{description}"
    );
    assert_eq!(issues, ["invalid_utf8", "conservative_parser_limit"]);
}

#[test]
fn media_type_html_and_transport_are_validated() {
    assert!(content_type_is_plain_text(Some(
        "text/plain; charset=UTF-8"
    )));
    assert!(content_type_is_plain_text(Some("text/plain")));
    assert!(!content_type_is_plain_text(Some("text/html")));
    assert!(!content_type_is_plain_text(Some(
        "text/plain; charset=iso-8859-1"
    )));
    assert!(!content_type_is_plain_text(Some("text/plain; malformed")));

    let expiration = (chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
        + chrono::Duration::days(180))
    .to_rfc3339();
    let body = format!("Contact: mailto:s@example.com\nExpires: {expiration}\n");
    let fields = parse_security_txt(
        &body,
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    let (_, _, _, _, _, issues) = grade_security_txt(
        &fields,
        "/.well-known/security.txt",
        "https://example.com/.well-known/security.txt",
        "http://example.com/.well-known/security.txt",
        Some("text/html"),
        false,
        true,
        body.len(),
        body.lines().count(),
        body.lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or_default(),
        true,
    );
    assert_eq!(
        issues,
        [
            "insecure_transport",
            "cross_origin_redirect",
            "html_response"
        ]
    );
}
