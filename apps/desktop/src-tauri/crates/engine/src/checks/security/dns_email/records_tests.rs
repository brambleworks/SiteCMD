use super::*;

fn mx(records: &[(u16, &str)]) -> Vec<MxRecord> {
    records
        .iter()
        .map(|(preference, exchange)| MxRecord {
            preference: *preference,
            exchange: exchange.to_string(),
        })
        .collect()
}

#[test]
fn real_mx_records_classify_as_receiving() {
    let posture = classify_mx(&mx(&[
        (1, "aspmx.l.google.com"),
        (5, "alt1.aspmx.l.google.com"),
    ]));
    assert!(matches!(posture, MxPosture::Receiving(2)));
}

#[test]
fn null_mx_is_recognized() {
    assert!(matches!(classify_mx(&mx(&[(0, ".")])), MxPosture::NullMx));
}

#[test]
fn empty_answer_classifies_as_no_records() {
    assert!(matches!(classify_mx(&[]), MxPosture::NoRecords));
}

#[test]
fn mixed_null_and_real_mx_counts_only_real_exchanges() {
    let posture = classify_mx(&mx(&[(0, "."), (10, "mail.example.com")]));
    assert!(matches!(posture, MxPosture::Receiving(1)));
}

#[test]
fn receiving_domain_passes_and_lists_exchanges() {
    let results = evaluate_mx(
        "example.com",
        DnsOutcome::Records(mx(&[(1, "aspmx.l.google.com")])),
    );
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].description.contains("aspmx.l.google.com"));
    assert_eq!(results[0].raw_data.as_ref().unwrap()["has_mx"], true);
}

#[test]
fn a_failed_mx_lookup_is_a_skip() {
    let results = evaluate_mx(
        "example.com",
        DnsOutcome::Failed("MX query timed out".into()),
    );
    assert_eq!(results[0].status, CheckStatus::Skipped);
}

#[test]
fn dnskey_presence_does_not_claim_full_dnssec_validation() {
    let result = dnssec_records_result("security.dns.dnssec", "example.com", 2);
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.description.contains("key publication only"));
    assert!(result.description.contains("RRSIG"));
    assert!(result.description.contains("DS chain"));
    assert!(!result.description.contains("zone is set up"));
}

#[test]
fn dnssec_migration_guidance_requires_a_coordinated_transition() {
    let result = dnssec_records_result("security.dns.dnssec", "example.com", 0);
    let fix = result.manual_fix.as_deref().unwrap_or_default();
    assert!(fix.contains("multi-signer or double-sign rollover"));
    assert!(fix.contains("cache/TTL waiting"));
    assert!(!fix.contains("disable DNSSEC before the move"));
}

#[test]
fn an_empty_dnskey_answer_grades_as_zero_keys() {
    let results = evaluate_dnssec("example.com", DnsOutcome::NoRecords);
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].raw_data.as_ref().unwrap()["dnskey_count"], 0);
}

fn caa(records: &[(&str, &str)]) -> Vec<CaaRecord> {
    records
        .iter()
        .map(|(tag, value)| CaaRecord {
            tag: tag.to_string(),
            value: value.to_string(),
        })
        .collect()
}

#[test]
fn caa_issuers_include_issue_and_issuewild_only() {
    let records = caa(&[
        ("issue", "letsencrypt.org"),
        ("issuewild", "pki.goog"),
        ("iodef", "mailto:security@example.com"),
    ]);
    assert_eq!(caa_issuers(&records), vec!["letsencrypt.org", "pki.goog"]);
}

#[test]
fn caa_issuers_empty_when_only_iodef_present() {
    let records = caa(&[("iodef", "mailto:s@example.com")]);
    assert!(caa_issuers(&records).is_empty());
}

#[test]
fn iodef_only_caa_warns_like_no_caa() {
    let records = caa(&[("iodef", "mailto:s@example.com")]);
    let result = caa_records_result("security.dns.caa", "example.com", &records);
    assert_eq!(result.status, CheckStatus::Warn);
    assert!(
        result.description.contains("no CAA authorization limit"),
        "copy should state the non-restricting CAA posture: {}",
        result.description
    );
    assert!(
        result
            .manual_fix
            .as_deref()
            .unwrap_or_default()
            .contains("issue"),
        "fix should tell the user to add issue records"
    );
}

#[test]
fn issue_restricting_caa_passes() {
    let records = caa(&[("issue", "letsencrypt.org")]);
    let result = caa_records_result("security.dns.caa", "example.com", &records);
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.description.contains("letsencrypt.org"));
    assert!(result.description.contains("issuance-time requests"));
    assert!(result.description.contains("not a browser"));
}

#[test]
fn a_missing_caa_set_warns_with_publish_guidance() {
    let results = evaluate_caa("example.com", DnsOutcome::NoRecords);
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].title, "No CAA records");
}
