use super::*;

#[test]
fn detects_spf_records_case_insensitively() {
    assert!(is_spf_record("v=spf1 include:_spf.google.com -all"));
    assert!(is_spf_record("V=SPF1 -all"));
    assert!(!is_spf_record("v=spf10 include:example.com"));
    assert!(!is_spf_record("google-site-verification=abc123"));
    assert!(!is_spf_record(""));
}

#[test]
fn counts_direct_dns_querying_mechanisms() {
    let analysis = analyze_spf(
        "v=spf1 include:_spf.google.com include:sendgrid.net a mx ptr exists:%{i}.spf.example.com -all",
    );
    assert_eq!(analysis.lookup_mechanisms, 6);
    assert!(!analysis.allows_any_sender);
    assert_eq!(analysis.all_qualifier, Some('-'));
}

#[test]
fn redirect_counts_but_exp_and_ip_terms_do_not() {
    let analysis = analyze_spf(
        "v=spf1 ip4:192.0.2.0/24 ip6:2001:db8::/32 exp=explain.example.com redirect=_spf.example.com",
    );
    assert_eq!(analysis.lookup_mechanisms, 1);
    assert!(analysis.has_redirect);
    assert_eq!(analysis.all_qualifier, None);
}

#[test]
fn qualified_mechanisms_still_count() {
    let analysis = analyze_spf(
        "v=spf1 +a:mail.example.com ~mx -include:deny.example.com ?exists:x.example.com -all",
    );
    assert_eq!(analysis.lookup_mechanisms, 4);
}

#[test]
fn a_with_cidr_suffix_counts_once() {
    let analysis = analyze_spf("v=spf1 a/24 mx/24 -all");
    assert_eq!(analysis.lookup_mechanisms, 2);
}

#[test]
fn plus_all_and_bare_all_allow_any_sender() {
    let explicit = analyze_spf("v=spf1 include:_spf.google.com +all");
    assert!(explicit.allows_any_sender);
    assert_eq!(explicit.all_qualifier, Some('+'));

    let bare = analyze_spf("v=spf1 all");
    assert!(bare.allows_any_sender);
    assert_eq!(bare.all_qualifier, Some('+'));
}

#[test]
fn softfail_and_hardfail_do_not_allow_any_sender() {
    assert!(!analyze_spf("v=spf1 ~all").allows_any_sender);
    assert!(!analyze_spf("v=spf1 -all").allows_any_sender);
    assert_eq!(analyze_spf("v=spf1 ?all").all_qualifier, Some('?'));
}

#[test]
fn uppercase_terms_are_normalized() {
    let analysis = analyze_spf("v=spf1 INCLUDE:_SPF.GOOGLE.COM -ALL");
    assert_eq!(analysis.lookup_mechanisms, 1);
    assert_eq!(analysis.all_qualifier, Some('-'));
}

#[test]
fn ip_only_record_has_zero_lookups() {
    let analysis = analyze_spf("v=spf1 ip4:203.0.113.5 -all");
    assert_eq!(analysis.lookup_mechanisms, 0);
}

fn records(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

#[test]
fn a_present_record_completes_without_the_mx_question() {
    let step = evaluate_spf_txt(
        "example.com",
        DnsOutcome::Records(records(&["v=spf1 include:_spf.google.com -all"])),
    );
    let SpfStep::Done(results) = step else {
        panic!("a present SPF record must not need the MX answer");
    };
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].description.contains("-all"));
}

#[test]
fn a_missing_record_asks_for_the_mx_answer_and_gates_severity_on_it() {
    let SpfStep::NeedsMx(pending) = evaluate_spf_txt("example.com", DnsOutcome::NoRecords) else {
        panic!("a missing SPF record needs the MX gate");
    };
    assert_eq!(pending.domain(), "example.com");
    let results = pending.evaluate(&DnsOutcome::Records(vec![MxRecord {
        preference: 10,
        exchange: "mail.example.com".into(),
    }]));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].severity, Severity::Medium);
    assert!(results[0].description.contains("natural spoofing target"));

    let SpfStep::NeedsMx(pending) = evaluate_spf_txt("example.com", DnsOutcome::NoRecords) else {
        panic!("a missing SPF record needs the MX gate");
    };
    let no_mx = pending.evaluate(&DnsOutcome::NoRecords);
    assert_eq!(no_mx[0].severity, Severity::Low);
    assert_eq!(no_mx[0].status, CheckStatus::Warn);
    assert!(
        no_mx[0]
            .description
            .contains("publishes no MX records, so it shows no inbound mail setup"),
        "{}",
        no_mx[0].description
    );
    assert!(no_mx[0].description.contains("v=spf1 -all"));

    let SpfStep::NeedsMx(pending) = evaluate_spf_txt("example.com", DnsOutcome::NoRecords) else {
        panic!("a missing SPF record needs the MX gate");
    };
    let unknown_mx = pending.evaluate(&DnsOutcome::Failed("timed out".into()));
    assert!(!unknown_mx[0].description.contains("MX records"));
}

#[test]
fn unrelated_txt_records_still_count_as_missing() {
    let step = evaluate_spf_txt(
        "example.com",
        DnsOutcome::Records(records(&["google-site-verification=abc"])),
    );
    assert!(matches!(step, SpfStep::NeedsMx(_)));
}

#[test]
fn a_failed_lookup_is_a_skip_not_a_finding() {
    let step = evaluate_spf_txt(
        "example.com",
        DnsOutcome::Failed("TXT query timed out".into()),
    );
    let SpfStep::Done(results) = step else {
        panic!("a failed lookup completes as Skipped");
    };
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["reason"],
        "dns_lookup_failed"
    );
}

#[test]
fn multiple_records_fail_without_the_mx_question() {
    let step = evaluate_spf_txt(
        "example.com",
        DnsOutcome::Records(records(&[
            "v=spf1 -all",
            "v=spf1 include:a.example.com ~all",
        ])),
    );
    let SpfStep::Done(results) = step else {
        panic!("duplicate SPF records complete immediately");
    };
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].description.contains("permerror"));
}
