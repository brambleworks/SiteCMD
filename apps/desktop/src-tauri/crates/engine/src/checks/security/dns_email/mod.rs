//! Portable DNS, email-authentication, and domain-registration verdicts.
//!
//! Resolver failures produce skipped results because they are not evidence of
//! domain misconfiguration.

pub mod dangling_cname;
pub mod dkim;
pub mod dmarc;
pub mod domain_expiry;
pub mod records;
pub mod spf;

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::dns::{DnsOutcome, MxRecord};

/// Where a check should direct its DNS questions, or why it cannot.
pub enum DomainTarget {
    /// The registrable domain (PSL eTLD+1): www.example.com -> example.com,
    /// sub.example.co.uk -> example.co.uk.
    Registrable(String),
    /// IP-literal,.local/.internal, or single-label host: there is no
    /// public DNS zone or registration to inspect.
    LocalOrIp,
}

pub fn registrable_domain_for_url(url: &url::Url) -> DomainTarget {
    let Some(url::Host::Domain(host)) = url.host() else {
        // IP literals (and URLs without a host) have no registrable domain.
        return DomainTarget::LocalOrIp;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.ends_with(".local") || host.ends_with(".internal") || !host.contains('.') {
        return DomainTarget::LocalOrIp;
    }
    match psl::domain_str(&host) {
        Some(domain) => DomainTarget::Registrable(domain.to_string()),
        None => DomainTarget::LocalOrIp,
    }
}

/// Skipped result for hosts with no public DNS zone (mirrors the localhost
/// skip pattern in checks/security/headers.rs).
pub fn skipped_local_result(check_id: &str, title: &str) -> CheckResult {
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Security,
        title: title.into(),
        description: "Skipped on local, IP-literal, or internal hosts. DNS and domain-registration records exist only for public registrable domains, so run this check against the deployed site.".into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({"reason": "local_or_ip_host"})),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Skipped result for resolver/transport problems. A lookup failure is not
/// evidence about the domain, so it must never surface as a Fail.
pub fn skipped_dns_failure(check_id: &str, title: &str, domain: &str, detail: &str) -> CheckResult {
    let detail = crate::log_sanitizer::bounded_issue_evidence(detail);
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Security,
        title: title.into(),
        description: format!(
            "DNS lookup failed for {}: {}. The resolver may be offline or the network may be blocking the query, so this check reports no finding rather than guessing.",
            domain, detail
        ),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(
            serde_json::json!({"reason": "dns_lookup_failed", "domain": domain, "detail": detail}),
        ),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Severity for a missing email-authentication record: domains that receive
/// mail (MX present) get Medium, others Low. An unknown MX state (lookup
/// failure) also gets the gentler Low.
pub fn missing_record_severity(has_mx: Option<bool>) -> Severity {
    if has_mx == Some(true) {
        Severity::Medium
    } else {
        Severity::Low
    }
}

/// Some(true) if the MX answer holds at least one real (non-null) exchange.
/// None when DNS could not answer, so callers fall back to gentler severity.
pub fn has_mx_from(outcome: &DnsOutcome<Vec<MxRecord>>) -> Option<bool> {
    match outcome {
        DnsOutcome::Records(records) => Some(records.iter().any(|record| record.exchange != ".")),
        DnsOutcome::NoRecords => Some(false),
        DnsOutcome::Failed(_) => None,
    }
}

/// Apex SPF posture at the resolution needed by the DKIM gate.
pub enum SpfPosture {
    /// At least one v=spf1 record that authorizes senders.
    Present,
    /// The only SPF content is the null record `v=spf1 -all`.
    NullRecord,
    /// No v=spf1 record.
    Absent,
}

/// True for the RFC 7208 null record: `v=spf1 -all` with no other terms.
fn spf_is_null_record(record: &str) -> bool {
    let mut terms = record.split_whitespace();
    let is_version = terms
        .next()
        .is_some_and(|first| first.eq_ignore_ascii_case("v=spf1"));
    let rest: Vec<&str> = terms.collect();
    is_version && rest.len() == 1 && rest[0].eq_ignore_ascii_case("-all")
}

pub fn classify_spf_posture(records: &[String]) -> SpfPosture {
    let spf_records: Vec<&String> = records
        .iter()
        .filter(|record| spf::is_spf_record(record))
        .collect();
    if spf_records.is_empty() {
        SpfPosture::Absent
    } else if spf_records.iter().all(|record| spf_is_null_record(record)) {
        SpfPosture::NullRecord
    } else {
        SpfPosture::Present
    }
}

/// The apex SPF posture from a TXT answer, or None when DNS could not answer.
pub fn spf_posture_from(outcome: &DnsOutcome<Vec<String>>) -> Option<SpfPosture> {
    match outcome {
        DnsOutcome::Records(records) => Some(classify_spf_posture(records)),
        DnsOutcome::NoRecords => Some(SpfPosture::Absent),
        DnsOutcome::Failed(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_spf_posture, has_mx_from, missing_record_severity, registrable_domain_for_url,
        DnsOutcome, DomainTarget, MxRecord, Severity, SpfPosture,
    };

    fn target_for(url: &str) -> DomainTarget {
        registrable_domain_for_url(&url::Url::parse(url).expect("url"))
    }

    #[test]
    fn registrable_domain_strips_subdomains() {
        match target_for("https://www.example.com/page") {
            DomainTarget::Registrable(domain) => assert_eq!(domain, "example.com"),
            DomainTarget::LocalOrIp => panic!("www.example.com should be registrable"),
        }
    }

    #[test]
    fn registrable_domain_honors_multi_label_public_suffixes() {
        match target_for("https://sub.example.co.uk/") {
            DomainTarget::Registrable(domain) => assert_eq!(domain, "example.co.uk"),
            DomainTarget::LocalOrIp => panic!("example.co.uk should be registrable"),
        }
    }

    #[test]
    fn ip_literal_hosts_have_no_registrable_domain() {
        assert!(matches!(
            target_for("http://192.168.1.10:3000/"),
            DomainTarget::LocalOrIp
        ));
        assert!(matches!(
            target_for("http://[::1]:8080/"),
            DomainTarget::LocalOrIp
        ));
    }

    #[test]
    fn local_and_internal_suffixes_are_skipped() {
        assert!(matches!(
            target_for("http://myserver.local/"),
            DomainTarget::LocalOrIp
        ));
        assert!(matches!(
            target_for("http://api.corp.internal/"),
            DomainTarget::LocalOrIp
        ));
        assert!(matches!(
            target_for("http://intranethost/"),
            DomainTarget::LocalOrIp
        ));
    }

    #[test]
    fn missing_record_severity_gates_on_mx() {
        assert_eq!(missing_record_severity(Some(true)), Severity::Medium);
        assert_eq!(missing_record_severity(Some(false)), Severity::Low);
        assert_eq!(missing_record_severity(None), Severity::Low);
    }

    fn mx_answer(exchanges: &[&str]) -> DnsOutcome<Vec<MxRecord>> {
        DnsOutcome::Records(
            exchanges
                .iter()
                .map(|exchange| MxRecord {
                    preference: 10,
                    exchange: exchange.to_string(),
                })
                .collect(),
        )
    }

    #[test]
    fn has_mx_requires_a_real_exchange() {
        assert_eq!(has_mx_from(&mx_answer(&["mail.example.com"])), Some(true));
        assert_eq!(has_mx_from(&mx_answer(&["."])), Some(false));
        assert_eq!(has_mx_from(&DnsOutcome::NoRecords), Some(false));
        assert_eq!(has_mx_from(&DnsOutcome::Failed("timed out".into())), None);
    }

    fn records(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn spf_posture_recognizes_the_null_record() {
        assert!(matches!(
            classify_spf_posture(&records(&["v=spf1 -all"])),
            SpfPosture::NullRecord
        ));
        assert!(matches!(
            classify_spf_posture(&records(&["V=SPF1 -ALL"])),
            SpfPosture::NullRecord
        ));
        // Unrelated TXT records alongside the null record don't change it.
        assert!(matches!(
            classify_spf_posture(&records(&["google-site-verification=abc", "v=spf1 -all"])),
            SpfPosture::NullRecord
        ));
    }

    #[test]
    fn spf_posture_with_sender_mechanisms_is_present() {
        assert!(matches!(
            classify_spf_posture(&records(&["v=spf1 include:_spf.google.com -all"])),
            SpfPosture::Present
        ));
        // Softfail-all is not the null record: it hedges instead of
        // declaring a no-mail posture.
        assert!(matches!(
            classify_spf_posture(&records(&["v=spf1 ~all"])),
            SpfPosture::Present
        ));
    }

    #[test]
    fn spf_posture_without_spf_records_is_absent() {
        assert!(matches!(
            classify_spf_posture(&records(&["google-site-verification=abc"])),
            SpfPosture::Absent
        ));
        assert!(matches!(classify_spf_posture(&[]), SpfPosture::Absent));
    }
}
