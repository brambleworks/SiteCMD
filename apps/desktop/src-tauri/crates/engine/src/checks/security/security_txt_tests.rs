//! security.txt verdict tests.

#![cfg(test)]

use super::{grade_security_txt, parse_security_txt};
use crate::checks::CheckStatus;

fn grade(body: &str) -> (CheckStatus, String, String, Option<String>, Vec<String>) {
    let near_future = (chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
        + chrono::Duration::days(180))
    .to_rfc3339();
    let body = body.replace("2099-01-01T00:00:00Z", &near_future);
    grade_exact(&body)
}

fn grade_exact(body: &str) -> (CheckStatus, String, String, Option<String>, Vec<String>) {
    let fields = parse_security_txt(
        body,
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    let (status, _, title, description, fix, issues) = grade_security_txt(
        &fields,
        "/.well-known/security.txt",
        "https://example.com/.well-known/security.txt",
        "https://example.com/.well-known/security.txt",
        Some("text/plain; charset=utf-8"),
        false,
        false,
        body.len(),
        body.lines().count(),
        body.lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or_default(),
        true,
    );
    (status, title, description, fix, issues)
}

#[test]
fn parses_contact_and_future_expires() {
    let fields = parse_security_txt(
        "# our policy\nContact: mailto:security@example.com\nExpires: 2099-01-01T00:00:00Z\n",
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    assert_eq!(fields.contact_count, 1);
    assert_eq!(fields.valid_contact_count, 1);
    assert_eq!(fields.expires_values, ["2099-01-01T00:00:00Z"]);
    assert_eq!(fields.expired, Some(false));
}

#[test]
fn detects_expired_file() {
    let fields = parse_security_txt(
        "Contact: https://example.com/report\nExpires: 2020-01-01T00:00:00Z\n",
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    assert_eq!(fields.valid_contact_count, 1);
    assert_eq!(fields.expired, Some(true));
}

#[test]
fn missing_contact_is_reported() {
    let fields = parse_security_txt(
        "Expires: 2099-01-01T00:00:00Z\n",
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    assert_eq!(fields.contact_count, 0);
}

#[test]
fn empty_contact_value_does_not_count() {
    let fields = parse_security_txt(
        "Contact:\n",
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    assert_eq!(fields.contact_count, 0);
}

#[test]
fn comment_lines_are_ignored() {
    let fields = parse_security_txt(
        "# Contact: mailto:not-real@example.com\n",
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    assert_eq!(fields.contact_count, 0);
}

#[test]
fn malformed_expires_is_unparseable_not_expired() {
    let fields = parse_security_txt(
        "Contact: mailto:s@example.com\nExpires: soon\n",
        chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    assert_eq!(fields.valid_contact_count, 1);
    assert_eq!(fields.expired, None);
    let (_, title, _, _, issues) = grade("Contact: mailto:s@example.com\nExpires: soon\n");
    assert!(title.contains("invalid"));
    assert_eq!(issues, ["invalid_expires"]);
}

#[test]
fn missing_expires_warns_because_rfc_9116_requires_it() {
    let (status, title, description, manual_fix, issues) =
        grade("Contact: mailto:security@example.com\n");
    assert_eq!(status, CheckStatus::Warn);
    assert!(title.contains("Expires"), "{}", title);
    assert!(
        description.contains("required Expires field"),
        "{}",
        description
    );
    assert!(manual_fix.unwrap().contains("Expires:"));
    assert_eq!(issues, ["missing_expires"]);
}

#[test]
fn missing_contact_copy_names_both_required_fields() {
    let (status, title, description, _, issues) = grade("Expires: 2099-01-01T00:00:00Z\n");
    assert_eq!(status, CheckStatus::Warn);
    assert!(title.contains("Contact"));
    assert!(
        description.contains("required Contact field"),
        "{}",
        description
    );
    assert_eq!(issues, ["missing_contact"]);
}

#[test]
fn contact_plus_future_expires_passes() {
    let (status, _, description, _, issues) =
        grade("Contact: mailto:security@example.com\nExpires: 2099-01-01T00:00:00Z\n");
    assert_eq!(status, CheckStatus::Pass);
    assert!(description.contains("syntactically valid Contact URI"));
    assert!(description.contains("not full ABNF"));
    assert!(description.contains("contact ownership"));
    assert!(issues.is_empty());
}

#[test]
fn expired_file_outranks_missing_expires_branch() {
    let (status, title, _, _, issues) =
        grade("Contact: mailto:security@example.com\nExpires: 2020-01-01T00:00:00Z\n");
    assert_eq!(status, CheckStatus::Warn);
    assert!(title.contains("expired"));
    assert_eq!(issues, ["expired"]);
}

#[test]
fn duplicate_expires_fields_are_not_accepted() {
    let (_, title, description, _, issues) = grade(
        "Contact: mailto:s@example.com\nExpires: 2099-01-01T00:00:00Z\nExpires: 2099-02-01T00:00:00Z\n",
    );
    assert!(title.contains("duplicate"));
    assert!(description.contains("requires exactly one"));
    assert_eq!(issues, ["duplicate_expires"]);
}

#[test]
fn contact_must_be_a_uri_and_web_contacts_must_use_https() {
    let (_, _, _, _, invalid_text) =
        grade("Contact: security@example.com\nExpires: 2099-01-01T00:00:00Z\n");
    let (_, _, _, _, insecure_web) =
        grade("Contact: http://example.com/report\nExpires: 2099-01-01T00:00:00Z\n");
    assert_eq!(invalid_text, ["invalid_contact"]);
    assert_eq!(insecure_web, ["invalid_contact"]);
}

#[test]
fn date_without_rfc3339_time_is_invalid() {
    let (_, _, _, _, issues) = grade("Contact: mailto:s@example.com\nExpires: 2099-01-01\n");
    assert_eq!(issues, ["invalid_expires"]);
}

#[test]
fn legacy_location_is_a_warning_not_a_standard_path_pass() {
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
    let (status, _, title, description, _, issues) = grade_security_txt(
        &fields,
        "/security.txt",
        "https://example.com/security.txt",
        "https://example.com/security.txt",
        Some("text/plain"),
        true,
        false,
        body.len(),
        body.lines().count(),
        body.lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or_default(),
        true,
    );
    assert_eq!(status, CheckStatus::Warn);
    assert!(title.contains("legacy path"));
    assert!(description.contains("required /.well-known/security.txt"));
    assert_eq!(issues, ["legacy_location"]);
}
