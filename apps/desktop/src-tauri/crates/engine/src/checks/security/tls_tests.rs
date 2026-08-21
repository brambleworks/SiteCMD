use super::*;

fn at(rfc3339: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .expect("static test timestamp")
        .with_timezone(&chrono::Utc)
}

const NOW: &str = "2026-08-05T00:00:00Z";

fn facts(not_after: Option<&str>, names: &[&str], protocol: Option<&str>) -> TlsFacts {
    TlsFacts {
        not_before: Some(at("2026-01-01T00:00:00Z")),
        not_after: not_after.map(at),
        issuer: Some("CN=Test CA".into()),
        subject_names: names.iter().map(|name| name.to_string()).collect(),
        protocol: protocol.map(str::to_string),
        validation: TlsValidation::valid(TrustAuthority::Webpki),
        facts_observed_at: at(NOW),
    }
}

fn row(rows: &[CheckResult], check_id: &str) -> CheckResult {
    rows.iter()
        .find(|row| row.check_id == check_id)
        .unwrap_or_else(|| panic!("row '{check_id}' present"))
        .clone()
}

#[test]
fn every_sub_check_emits_exactly_one_row() {
    let rows = evaluate_tls(
        "example.com",
        &facts(
            Some("2027-01-01T00:00:00Z"),
            &["example.com"],
            Some("TLSv1.3"),
        ),
        at(NOW),
    );
    let ids: Vec<&str> = rows.iter().map(|row| row.check_id.as_str()).collect();
    assert_eq!(ids, TLS_CHECK_IDS);
}

#[test]
fn expiry_grades_the_documented_ladder_from_the_injected_clock() {
    let ladder = [
        (
            "2026-08-04T00:00:00Z",
            CheckStatus::Fail,
            Severity::Critical,
        ),
        ("2026-08-05T00:00:00Z", CheckStatus::Warn, Severity::High),
        ("2026-08-12T00:00:00Z", CheckStatus::Warn, Severity::High),
        ("2026-08-13T00:00:00Z", CheckStatus::Pass, Severity::Low),
        ("2026-12-01T00:00:00Z", CheckStatus::Pass, Severity::Low),
    ];
    for (not_after, status, severity) in ladder {
        let rows = evaluate_tls(
            "example.com",
            &facts(Some(not_after), &["example.com"], None),
            at(NOW),
        );
        let expiry = row(&rows, EXPIRY_CHECK_ID);
        assert_eq!(expiry.status, status, "not_after {not_after}");
        assert_eq!(expiry.severity, severity, "not_after {not_after}");
    }
}

#[test]
fn a_missing_expiry_is_skipped_never_assumed_valid() {
    let rows = evaluate_tls("example.com", &facts(None, &["example.com"], None), at(NOW));
    let expiry = row(&rows, EXPIRY_CHECK_ID);
    assert_eq!(expiry.status, CheckStatus::Skipped);
    assert_eq!(expiry.confidence, IssueConfidence::NeedsReview);
    assert!(!expiry.description.contains("is valid"));
}

#[test]
fn the_eight_to_thirty_day_window_passes_while_disclosing_unknown_renewal_state() {
    let rows = evaluate_tls(
        "example.com",
        &facts(Some("2026-08-20T00:00:00Z"), &["example.com"], None),
        at(NOW),
    );
    let expiry = row(&rows, EXPIRY_CHECK_ID);
    assert_eq!(expiry.status, CheckStatus::Pass);
    assert!(expiry
        .description
        .contains("does not observe renewal state"));
}

#[test]
fn hostname_matching_follows_the_single_label_wildcard_rule() {
    assert!(certificate_name_matches("example.com", "example.com"));
    assert!(certificate_name_matches("EXAMPLE.com", "example.COM"));
    assert!(certificate_name_matches("*.example.com", "www.example.com"));
    // A wildcard covers exactly one label: not the bare parent, not a
    // deeper subdomain.
    assert!(!certificate_name_matches("*.example.com", "example.com"));
    assert!(!certificate_name_matches(
        "*.example.com",
        "a.b.example.com"
    ));
    assert!(!certificate_name_matches("*.example.com", ".example.com"));
    // A lookalike suffix is not a match.
    assert!(!certificate_name_matches("example.com", "notexample.com"));
    assert!(!certificate_name_matches("", "example.com"));
}

#[test]
fn a_certificate_naming_the_host_passes_and_one_that_does_not_fails_critical() {
    let covered = evaluate_tls(
        "www.example.com",
        &facts(Some("2027-01-01T00:00:00Z"), &["*.example.com"], None),
        at(NOW),
    );
    assert_eq!(row(&covered, HOSTNAME_CHECK_ID).status, CheckStatus::Pass);

    let uncovered = evaluate_tls(
        "example.com",
        &facts(Some("2027-01-01T00:00:00Z"), &["other.example.net"], None),
        at(NOW),
    );
    let hostname = row(&uncovered, HOSTNAME_CHECK_ID);
    assert_eq!(hostname.status, CheckStatus::Fail);
    assert_eq!(hostname.severity, Severity::Critical);
    assert_eq!(hostname.confidence, IssueConfidence::High);
}

#[test]
fn absent_certificate_names_are_skipped_not_a_mismatch() {
    let rows = evaluate_tls(
        "example.com",
        &facts(Some("2027-01-01T00:00:00Z"), &[], None),
        at(NOW),
    );
    let hostname = row(&rows, HOSTNAME_CHECK_ID);
    assert_eq!(hostname.status, CheckStatus::Skipped);
    assert!(!hostname.description.contains("does not cover"));
}

#[test]
fn chain_verdicts_record_the_authority_and_split_definitive_from_trust_difference() {
    let mut invalid = facts(Some("2027-01-01T00:00:00Z"), &["example.com"], None);
    invalid.validation =
        TlsValidation::invalid(TrustAuthority::Webpki, "invalid peer certificate: Expired");
    let definitive = row(
        &evaluate_tls("example.com", &invalid, at(NOW)),
        CHAIN_CHECK_ID,
    );
    assert_eq!(definitive.status, CheckStatus::Fail);
    assert_eq!(definitive.severity, Severity::Critical);
    assert_eq!(definitive.confidence, IssueConfidence::High);

    let mut unknown_issuer = facts(Some("2027-01-01T00:00:00Z"), &["example.com"], None);
    unknown_issuer.validation = TlsValidation::invalid(
        TrustAuthority::Webpki,
        "invalid peer certificate: UnknownIssuer",
    );
    let hedged = row(
        &evaluate_tls("example.com", &unknown_issuer, at(NOW)),
        CHAIN_CHECK_ID,
    );
    assert_eq!(hedged.status, CheckStatus::Warn);
    assert_eq!(hedged.severity, Severity::High);
    assert_eq!(hedged.confidence, IssueConfidence::NeedsReview);

    let valid = row(
        &evaluate_tls(
            "example.com",
            &facts(Some("2027-01-01T00:00:00Z"), &["example.com"], None),
            at(NOW),
        ),
        CHAIN_CHECK_ID,
    );
    assert_eq!(valid.status, CheckStatus::Pass);
    assert_eq!(
        valid.raw_data.as_ref().unwrap()["authority"],
        TrustAuthority::Webpki.as_str()
    );
}

#[test]
fn an_unavailable_chain_verdict_is_skipped_not_a_pass() {
    let mut unknown = facts(Some("2027-01-01T00:00:00Z"), &["example.com"], None);
    unknown.validation = TlsValidation::unavailable(TrustAuthority::Chromium);
    let chain = row(
        &evaluate_tls("example.com", &unknown, at(NOW)),
        CHAIN_CHECK_ID,
    );
    assert_eq!(chain.status, CheckStatus::Skipped);
    assert_eq!(chain.raw_data.as_ref().unwrap()["authority"], "chromium");
}

#[test]
fn cloudflare_workers_is_a_distinct_chain_authority() {
    let mut hosted = facts(
        Some("2027-01-01T00:00:00Z"),
        &["example.com"],
        Some("TLSv1.3"),
    );
    hosted.validation = TlsValidation::valid(TrustAuthority::CloudflareWorkers);
    let chain = row(
        &evaluate_tls("example.com", &hosted, at(NOW)),
        CHAIN_CHECK_ID,
    );

    assert_eq!(chain.status, CheckStatus::Pass);
    assert_eq!(
        chain.raw_data.as_ref().unwrap()["authority"],
        "cloudflare_workers"
    );
}

#[test]
fn deprecated_protocol_versions_fail_and_current_ones_pass() {
    for deprecated in ["TLSv1.0", "TLSv1", "TLSv1.1", "SSLv3"] {
        let rows = evaluate_tls(
            "example.com",
            &facts(
                Some("2027-01-01T00:00:00Z"),
                &["example.com"],
                Some(deprecated),
            ),
            at(NOW),
        );
        let protocol = row(&rows, PROTOCOL_CHECK_ID);
        assert_eq!(protocol.status, CheckStatus::Fail, "{deprecated}");
        assert_eq!(protocol.severity, Severity::High, "{deprecated}");
    }
    for current in ["TLSv1.2", "TLSv1.3"] {
        let rows = evaluate_tls(
            "example.com",
            &facts(
                Some("2027-01-01T00:00:00Z"),
                &["example.com"],
                Some(current),
            ),
            at(NOW),
        );
        assert_eq!(
            row(&rows, PROTOCOL_CHECK_ID).status,
            CheckStatus::Pass,
            "{current}"
        );
    }
}

#[test]
fn the_protocol_verdict_discloses_that_it_reflects_this_client_hello() {
    let rows = evaluate_tls(
        "example.com",
        &facts(
            Some("2027-01-01T00:00:00Z"),
            &["example.com"],
            Some("TLSv1.3"),
        ),
        at(NOW),
    );
    let protocol = row(&rows, PROTOCOL_CHECK_ID);
    assert!(protocol.description.contains("depends on the client hello"));
    assert!(!protocol.description.contains("the server only supports"));
}

#[test]
fn an_absent_protocol_is_skipped() {
    let rows = evaluate_tls(
        "example.com",
        &facts(Some("2027-01-01T00:00:00Z"), &["example.com"], None),
        at(NOW),
    );
    assert_eq!(row(&rows, PROTOCOL_CHECK_ID).status, CheckStatus::Skipped);
}

#[test]
fn unavailable_facts_emit_one_coverage_exception_per_sub_check() {
    // Each sub-check really was not evaluated, so each reports its own
    // coverage exception rather than one row standing in for four.
    for reason in [
        TlsUnavailable::NotHttps,
        TlsUnavailable::NoHost,
        TlsUnavailable::Transport {
            detail: "connection reset".into(),
        },
        TlsUnavailable::ProbeFailed {
            detail: "task panicked".into(),
        },
    ] {
        let rows = tls_unavailable_results(&reason);
        let ids: Vec<&str> = rows.iter().map(|row| row.check_id.as_str()).collect();
        assert_eq!(ids, TLS_CHECK_IDS, "{reason:?}");
        assert!(rows.iter().all(|row| row.status == CheckStatus::Skipped));
        assert!(rows.iter().all(|row| row.severity == Severity::Low));
    }
}

#[test]
fn a_transport_failure_never_becomes_a_certificate_finding() {
    let rows = tls_unavailable_results(&TlsUnavailable::Transport {
        detail: "connection reset".into(),
    });
    assert!(rows.iter().all(|row| row.status == CheckStatus::Skipped));
    assert!(rows[0]
        .description
        .contains("most likely a transient network issue"));
    assert_eq!(rows[0].confidence, IssueConfidence::NeedsReview);
}

#[test]
fn a_non_https_scan_target_is_a_plain_not_applicable_not_a_review_item() {
    let rows = tls_unavailable_results(&TlsUnavailable::NotHttps);
    assert!(rows
        .iter()
        .all(|row| row.confidence == IssueConfidence::High));
    assert!(rows[0].description.contains("HTTPS enforcement check"));
}

#[test]
fn a_leaf_certificate_parses_into_facts() {
    let der = include_bytes!("../../../fixtures/tls/leaf.der");
    let parsed = parse_leaf_certificate(der).expect("fixture certificate parses");
    assert!(parsed.subject_names.contains(&"sitecmd.test".to_string()));
    assert!(parsed
        .subject_names
        .contains(&"www.sitecmd.test".to_string()));
    assert!(parsed.not_before.is_some());
    assert!(parsed.not_after.is_some());
    assert!(parsed
        .issuer
        .as_deref()
        .is_some_and(|issuer| issuer.contains("sitecmd.test")));
}

#[test]
fn garbage_der_yields_no_facts_rather_than_empty_ones() {
    assert!(parse_leaf_certificate(&[0x30, 0x00]).is_none());
    assert!(parse_leaf_certificate(b"not a certificate").is_none());
}
