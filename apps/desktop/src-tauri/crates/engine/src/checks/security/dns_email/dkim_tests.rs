use super::*;

#[test]
fn selector_list_matches_the_vendored_set() {
    assert_eq!(COMMON_SELECTORS.len(), 16);
    for expected in ["default", "google", "selector1", "selector2", "fm1", "pm"] {
        assert!(
            COMMON_SELECTORS.contains(&expected),
            "missing selector {}",
            expected
        );
    }
}

#[test]
fn recognizes_dkim_records_by_version_or_key_tag() {
    assert!(looks_like_dkim_record(
        "v=DKIM1; k=rsa; p=MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQ"
    ));
    assert!(looks_like_dkim_record("k=rsa; p=MIGfMA0GCSq"));
    // Revoked key (empty p=) still proves the selector exists.
    assert!(looks_like_dkim_record("v=DKIM1; p="));
    assert!(!looks_like_dkim_record("google-site-verification=abc"));
    assert!(!looks_like_dkim_record("v=spf1 -all"));
}

#[test]
fn sweep_runs_when_mx_or_spf_is_present() {
    assert!(matches!(
        sweep_decision(Some(true), Some(SpfPosture::Absent)),
        SweepDecision::Sweep
    ));
    assert!(matches!(
        sweep_decision(Some(false), Some(SpfPosture::Present)),
        SweepDecision::Sweep
    ));
    assert!(matches!(
        sweep_decision(None, Some(SpfPosture::Present)),
        SweepDecision::Sweep
    ));
    assert!(matches!(
        sweep_decision(Some(true), Some(SpfPosture::NullRecord)),
        SweepDecision::Sweep
    ));
}

#[test]
fn sweep_skips_non_mail_domains() {
    assert!(matches!(
        sweep_decision(Some(false), Some(SpfPosture::Absent)),
        SweepDecision::NotApplicable
    ));
    assert!(matches!(
        sweep_decision(Some(false), None),
        SweepDecision::NotApplicable
    ));
    assert!(matches!(
        sweep_decision(None, Some(SpfPosture::Absent)),
        SweepDecision::NotApplicable
    ));
}

#[test]
fn spf_null_record_without_mx_is_a_declared_no_mail_posture() {
    assert!(matches!(
        sweep_decision(Some(false), Some(SpfPosture::NullRecord)),
        SweepDecision::DeclaredNoMail
    ));
    assert!(matches!(
        sweep_decision(None, Some(SpfPosture::NullRecord)),
        SweepDecision::DeclaredNoMail
    ));
}

#[test]
fn sweep_reports_dns_unavailable_when_both_gates_fail() {
    assert!(matches!(
        sweep_decision(None, None),
        SweepDecision::DnsUnavailable
    ));
}

#[test]
fn empty_p_tag_is_a_revoked_key() {
    assert!(dkim_record_is_revoked("v=DKIM1; p="));
    assert!(dkim_record_is_revoked("v=DKIM1; k=rsa; p= "));
    assert!(!dkim_record_is_revoked("v=DKIM1; p=MIGfMA0GCSq"));
    assert!(!dkim_record_is_revoked("k=rsa; p=MIGfMA0GCSq"));
    // No p= tag at all is not "revoked" (it is not a key record).
    assert!(!dkim_record_is_revoked("v=DKIM1; k=rsa"));
}

#[test]
fn only_a_nonempty_p_tag_is_an_active_dkim_key() {
    assert!(dkim_record_has_active_key("v=DKIM1; k=rsa; p=MIGfMA0GCSq"));
    assert!(!dkim_record_has_active_key("v=DKIM1; k=rsa"));
    assert!(!dkim_record_has_active_key("v=DKIM1; p="));
}

#[test]
fn found_description_does_not_claim_revoked_keys_verify() {
    let revoked_only = dkim_found_description("example.com", &[], &["k1"]);
    assert!(
        !revoked_only.contains("can verify"),
        "revoked-only copy must not claim verification works: {}",
        revoked_only
    );
    assert!(
        revoked_only.contains("revoked"),
        "revoked-only copy should say the key is revoked: {}",
        revoked_only
    );

    let mixed = dkim_found_description("example.com", &["google"], &["k1"]);
    assert!(mixed.contains("publishes key material"));
    assert!(mixed.contains("does not confirm"));
    assert!(mixed.contains("The key for k1 is revoked"));

    let active_only = dkim_found_description("example.com", &["google", "selector1"], &[]);
    assert!(active_only.contains("google, selector1"));
    assert!(active_only.contains("does not confirm"));
    assert!(!active_only.contains("revoked"));
}

fn mx_receiving() -> DnsOutcome<Vec<MxRecord>> {
    DnsOutcome::Records(vec![MxRecord {
        preference: 10,
        exchange: "mail.example.com".into(),
    }])
}

fn txt(values: &[&str]) -> DnsOutcome<Vec<String>> {
    DnsOutcome::Records(values.iter().map(|v| v.to_string()).collect())
}

// One answered sweep: every listed selector gets the given records, every
// other selector authoritatively has none.
fn sweep_outcomes(answered: &[(&str, &[&str])]) -> Vec<(String, DnsOutcome<Vec<String>>)> {
    COMMON_SELECTORS
        .iter()
        .map(|selector| {
            let outcome = answered
                .iter()
                .find(|(name, _)| name == selector)
                .map(|(_, records)| txt(records))
                .unwrap_or(DnsOutcome::NoRecords);
            (selector.to_string(), outcome)
        })
        .collect()
}

#[test]
fn a_mail_domain_gates_into_the_sixteen_name_sweep() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[]))
    else {
        panic!("a mail-receiving domain must sweep");
    };
    let names = sweep.probe_names();
    assert_eq!(names.len(), 16);
    assert_eq!(names[0].1, "default._domainkey.example.com");
}

#[test]
fn a_non_mail_domain_completes_without_a_sweep() {
    let step = evaluate_dkim_gate(
        "example.com",
        &DnsOutcome::NoRecords,
        &DnsOutcome::NoRecords,
    );
    let DkimStep::Done(results) = step else {
        panic!("a non-mail domain must not sweep");
    };
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert_eq!(results[0].title, "DKIM probe not applicable");
}

#[test]
fn a_declared_no_mail_domain_completes_without_a_sweep() {
    let step = evaluate_dkim_gate(
        "example.com",
        &DnsOutcome::NoRecords,
        &txt(&["v=spf1 -all"]),
    );
    let DkimStep::Done(results) = step else {
        panic!("a declared no-mail domain must not sweep");
    };
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].title.contains("sends no mail"));
}

#[test]
fn both_gate_lookups_failing_skips() {
    let step = evaluate_dkim_gate(
        "example.com",
        &DnsOutcome::Failed("MX query timed out".into()),
        &DnsOutcome::Failed("TXT query timed out".into()),
    );
    let DkimStep::Done(results) = step else {
        panic!("unavailable DNS completes as Skipped");
    };
    assert_eq!(results[0].status, CheckStatus::Skipped);
}

#[test]
fn an_active_selector_passes_the_sweep() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[]))
    else {
        panic!("gate must sweep");
    };
    let results = sweep.evaluate(&sweep_outcomes(&[(
        "google",
        &["v=DKIM1; k=rsa; p=MIGfMA0GCSq"],
    )]));
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].description.contains("google"));
}

#[test]
fn a_revoked_only_sweep_warns_without_claiming_verification() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[]))
    else {
        panic!("gate must sweep");
    };
    let results = sweep.evaluate(&sweep_outcomes(&[("k1", &["v=DKIM1; p="])]));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].description.contains("revoked"));
}

#[test]
fn an_empty_sweep_on_a_null_spf_mail_receiver_is_consistent_not_a_gap() {
    let DkimStep::Sweep(sweep) =
        evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&["v=spf1 -all"]))
    else {
        panic!("MX present must sweep even with null SPF");
    };
    let results = sweep.evaluate(&sweep_outcomes(&[]));
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0]
        .description
        .contains("consistent with that posture"));
}

#[test]
fn an_empty_sweep_on_a_sending_domain_warns_with_review_confidence() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[]))
    else {
        panic!("gate must sweep");
    };
    let results = sweep.evaluate(&sweep_outcomes(&[]));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
}

#[test]
fn a_sweep_where_every_probe_failed_skips() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[]))
    else {
        panic!("gate must sweep");
    };
    let outcomes: Vec<(String, DnsOutcome<Vec<String>>)> = COMMON_SELECTORS
        .iter()
        .map(|selector| (selector.to_string(), DnsOutcome::Failed("timed out".into())))
        .collect();
    let results = sweep.evaluate(&outcomes);
    assert_eq!(results[0].status, CheckStatus::Skipped);
}
