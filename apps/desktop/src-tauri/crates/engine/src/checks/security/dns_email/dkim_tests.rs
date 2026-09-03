use super::*;

#[test]
fn selector_list_matches_the_vendored_set() {
    assert_eq!(COMMON_SELECTORS.len(), 22);
    for expected in [
        "default",
        "google",
        "selector1",
        "selector2",
        "fm1",
        "fm2",
        "fm3",
        "pm",
        "protonmail",
        "protonmail2",
        "protonmail3",
        "sig1",
    ] {
        assert!(
            COMMON_SELECTORS.contains(&expected),
            "missing selector {}",
            expected
        );
    }
    let mut deduplicated = COMMON_SELECTORS.to_vec();
    deduplicated.sort_unstable();
    deduplicated.dedup();
    assert_eq!(deduplicated.len(), COMMON_SELECTORS.len());
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
fn a_mail_domain_gates_into_the_common_name_sweep() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[]))
    else {
        panic!("a mail-receiving domain must sweep");
    };
    let names = sweep.probe_names();
    assert_eq!(names.len(), COMMON_SELECTORS.len());
    assert_eq!(names[0].1, "default._domainkey.example.com");
    assert!(names
        .iter()
        .any(|(_, name)| name == "protonmail._domainkey.example.com"));
}

#[test]
fn a_proton_mail_domain_passes_on_its_protonmail_selector() {
    // Proton custom domains publish protonmail._domainkey (plus protonmail2
    // and protonmail3) as CNAMEs to a TXT key record; the previous list
    // never probed them and warned on every Proton-hosted domain.
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate(
        "example.com",
        &mx_receiving(),
        &txt(&["v=spf1 include:_spf.protonmail.ch ~all"]),
    ) else {
        panic!("gate must sweep");
    };
    let results = sweep.evaluate(&sweep_outcomes(&[(
        "protonmail",
        &["v=DKIM1;k=rsa;p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAyl1c8A0ip8di"],
    )]));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["selectors_found"],
        serde_json::json!(["protonmail"])
    );
}

#[test]
fn a_non_mail_domain_skips_without_a_sweep_and_names_the_missing_mx() {
    let step = evaluate_dkim_gate(
        "example.com",
        &DnsOutcome::NoRecords,
        &DnsOutcome::NoRecords,
    );
    let DkimStep::Done(results) = step else {
        panic!("a non-mail domain must not sweep");
    };
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert_eq!(results[0].title, "DKIM probe not applicable");
    assert!(
        results[0]
            .description
            .contains("publishes no MX records and has no SPF record"),
        "{}",
        results[0].description
    );
    let raw = results[0].raw_data.as_ref().unwrap();
    assert_eq!(raw["reason"], "no_mail_setup");
    assert_eq!(raw["probed"], false);
}

#[test]
fn a_failed_mx_lookup_with_no_spf_skips_without_claiming_no_mx() {
    let step = evaluate_dkim_gate(
        "example.com",
        &DnsOutcome::Failed("MX query timed out".into()),
        &DnsOutcome::NoRecords,
    );
    let DkimStep::Done(results) = step else {
        panic!("an unanswered MX with no SPF must not sweep");
    };
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert!(
        results[0]
            .description
            .contains("did not answer the MX lookup and has no SPF record"),
        "{}",
        results[0].description
    );
    assert!(!results[0].description.contains("publishes no MX"));
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

#[test]
fn spf_includes_name_the_provider_whose_selectors_should_be_probed() {
    let proton = spf_named_providers(&["v=spf1 include:_spf.protonmail.ch mx ~all".to_string()]);
    assert_eq!(
        proton.iter().map(|p| p.label).collect::<Vec<_>>(),
        ["Proton Mail"]
    );

    // Qualifiers, case, a trailing dot, and a sub-host of the include all
    // resolve to the same provider.
    let google = spf_named_providers(&["V=SPF1 +INCLUDE:_SPF.GOOGLE.COM. -all".to_string()]);
    assert_eq!(
        google.iter().map(|p| p.label).collect::<Vec<_>>(),
        ["Google Workspace"]
    );

    // A TXT record that is not an SPF record contributes nothing.
    assert!(spf_named_providers(&["google-site-verification=abc".to_string()]).is_empty());
    assert!(spf_named_providers(&["v=spf1 -all".to_string()]).is_empty());
}

#[test]
fn selectors_are_derived_from_spf_includes_on_top_of_the_common_list() {
    // Proton's selectors are already common names, so it adds nothing and
    // must not be named as a provider that extended the sweep.
    let proton = selectors_from_spf(&["v=spf1 include:_spf.protonmail.ch ~all".to_string()]);
    assert!(proton.selectors.is_empty());
    assert!(proton.providers.is_empty());

    // A provider whose defaults are not general enough for the common list
    // contributes them only because this domain's SPF names it.
    let mailjet = selectors_from_spf(&["v=spf1 include:spf.mailjet.com ~all".to_string()]);
    assert_eq!(mailjet.selectors, ["mailjet"]);
    assert_eq!(mailjet.providers, ["Mailjet"]);

    let hostinger =
        selectors_from_spf(&["v=spf1 include:_spf.mail.hostinger.com ~all".to_string()]);
    assert_eq!(
        hostinger.selectors,
        ["hostingermail1", "hostingermail2", "hostingermail3"]
    );
    assert_eq!(hostinger.providers, ["Hostinger"]);

    // A record naming both kinds names only the contributor.
    let both = selectors_from_spf(&[
        "v=spf1 include:_spf.google.com include:spf.mailjet.com ~all".to_string(),
    ]);
    assert_eq!(both.selectors, ["mailjet"]);
    assert_eq!(
        both.providers,
        ["Mailjet"],
        "Google Workspace is named by the SPF record but adds no selector"
    );
}

#[test]
fn a_derived_selector_is_probed_and_can_carry_the_verdict() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate(
        "example.com",
        &mx_receiving(),
        &txt(&["v=spf1 include:spf.mailjet.com ~all"]),
    ) else {
        panic!("gate must sweep");
    };
    let names = sweep.probe_names();
    assert_eq!(names.len(), COMMON_SELECTORS.len() + 1);
    assert_eq!(
        names.last().expect("derived name").1,
        "mailjet._domainkey.example.com"
    );
    assert_eq!(sweep.spf_providers(), ["Mailjet"]);

    let outcomes: Vec<(String, DnsOutcome<Vec<String>>)> = names
        .iter()
        .map(|(selector, _)| {
            let outcome = if *selector == "mailjet" {
                txt(&["v=DKIM1; k=rsa; p=MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQ"])
            } else {
                DnsOutcome::NoRecords
            };
            (selector.to_string(), outcome)
        })
        .collect();
    let results = sweep.evaluate(&outcomes);
    assert_eq!(results[0].status, CheckStatus::Pass);
    let evidence = results[0].raw_data.as_ref().expect("dkim evidence");
    assert_eq!(evidence["selectors_found"], serde_json::json!(["mailjet"]));
    assert_eq!(evidence["spf_providers"], serde_json::json!(["Mailjet"]));
    assert_eq!(
        evidence["selectors_probed"],
        serde_json::json!(COMMON_SELECTORS.len() + 1)
    );
}

#[test]
fn an_empty_sweep_counts_only_the_selectors_that_answered() {
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[]))
    else {
        panic!("gate must sweep");
    };
    let mut outcomes = sweep_outcomes(&[]);
    outcomes[0].1 = DnsOutcome::Failed("timed out".into());
    let results = sweep.evaluate(&outcomes);
    let evidence = results[0].raw_data.as_ref().expect("dkim evidence");
    assert_eq!(evidence["failed_probes"], 1);
    assert_eq!(
        evidence["selectors_answered"],
        serde_json::json!(COMMON_SELECTORS.len() - 1)
    );
    assert!(
        results[0]
            .description
            .contains("did not answer and were not evaluated")
            || results[0]
                .description
                .contains("did not answer and was not evaluated"),
        "{}",
        results[0].description
    );
}

#[test]
fn the_planned_selector_union_covers_every_derivation() {
    // A runtime that plans its DNS questions up front gathers
    // COMMON_SELECTORS plus this union. If any provider could derive a
    // selector outside it, that name would come back ungathered on the hosted
    // path and read as a failed probe.
    let planned: Vec<&str> = COMMON_SELECTORS
        .iter()
        .copied()
        .chain(all_provider_selectors())
        .collect();
    let mut deduplicated = planned.clone();
    deduplicated.sort_unstable();
    deduplicated.dedup();
    assert_eq!(
        deduplicated.len(),
        planned.len(),
        "the planned set must have no duplicate lookups"
    );

    for provider in SPF_MAIL_PROVIDERS {
        for include in provider.includes {
            let record = format!("v=spf1 include:{include} ~all");
            for selector in selectors_from_spf(std::slice::from_ref(&record)).selectors {
                assert!(
                    planned.contains(&selector),
                    "{} selector {selector} is derivable but not planned",
                    provider.label
                );
            }
            // And the sweep for such a domain never asks for an unplanned name.
            let DkimStep::Sweep(sweep) =
                evaluate_dkim_gate("example.com", &mx_receiving(), &txt(&[&record]))
            else {
                panic!("{} must gate into a sweep", provider.label);
            };
            for (selector, _) in sweep.probe_names() {
                assert!(
                    planned.contains(&selector),
                    "{} sweep asks for unplanned selector {selector}",
                    provider.label
                );
            }
        }
    }
}

#[test]
fn the_sweep_copy_names_only_the_providers_that_added_a_selector() {
    // Google Workspace's `google` selector is already a common name, so an SPF
    // record naming both it and Mailjet extended the sweep by exactly one
    // name, from one provider.
    let DkimStep::Sweep(sweep) = evaluate_dkim_gate(
        "example.com",
        &mx_receiving(),
        &txt(&["v=spf1 include:_spf.google.com include:spf.mailjet.com ~all"]),
    ) else {
        panic!("gate must sweep");
    };
    let outcomes: Vec<(String, DnsOutcome<Vec<String>>)> = sweep
        .probe_names()
        .iter()
        .map(|(selector, _)| (selector.to_string(), DnsOutcome::NoRecords))
        .collect();
    let results = sweep.evaluate(&outcomes);

    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(
        results[0]
            .description
            .contains("the 1 default selector name published by Mailjet"),
        "{}",
        results[0].description
    );
    assert!(
        !results[0].description.contains("Google Workspace"),
        "a provider that added nothing must not be named in copy about the probe list: {}",
        results[0].description
    );
    // The raw data still records every provider the SPF record names.
    let evidence = results[0].raw_data.as_ref().expect("dkim evidence");
    assert_eq!(
        evidence["spf_providers"],
        serde_json::json!(["Google Workspace", "Mailjet"])
    );
    assert_eq!(
        evidence["spf_derived_selectors"],
        serde_json::json!(["mailjet"])
    );
}
