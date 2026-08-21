//! MX posture, DNSSEC, and CAA grading at the domain apex.

use super::skipped_dns_failure;
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::dns::{CaaRecord, DnsOutcome, MxRecord};

// security.dns.mx

pub const MX_CHECK_ID: &str = "security.dns.mx";
pub const MX_TITLE: &str = "MX records";

pub enum MxPosture {
    /// Real mail exchangers are published.
    Receiving(usize),
    /// A single root exchange ("0.") - RFC 7505 null MX, an explicit
    /// declaration that the domain receives no mail.
    NullMx,
    /// No MX records at all.
    NoRecords,
}

pub fn classify_mx(records: &[MxRecord]) -> MxPosture {
    if records.is_empty() {
        return MxPosture::NoRecords;
    }
    let real = records
        .iter()
        .filter(|record| record.exchange != ".")
        .count();
    if real == 0 {
        MxPosture::NullMx
    } else {
        MxPosture::Receiving(real)
    }
}

pub fn evaluate_mx(domain: &str, mx: DnsOutcome<Vec<MxRecord>>) -> Vec<CheckResult> {
    let records = match mx {
        DnsOutcome::Failed(detail) => {
            return vec![skipped_dns_failure(MX_CHECK_ID, MX_TITLE, domain, &detail)]
        }
        DnsOutcome::NoRecords => Vec::new(),
        DnsOutcome::Records(records) => records,
    };

    let posture = classify_mx(&records);
    let has_mx = matches!(posture, MxPosture::Receiving(_));
    let (description, hosts): (String, Vec<String>) = match posture {
        MxPosture::Receiving(count) => {
            let hosts: Vec<String> = records
                .iter()
                .filter(|record| record.exchange != ".")
                .map(|record| format!("{} (priority {})", record.exchange, record.preference))
                .collect();
            (
                format!(
                    "{} publishes {} MX record{}: {}. The email authentication checks treat it as a mail-receiving domain.",
                    domain,
                    count,
                    if count == 1 { "" } else { "s" },
                    hosts.join(", ")
                ),
                hosts,
            )
        }
        MxPosture::NullMx => (
            format!(
                "{} publishes a null MX record (RFC 7505), explicitly declaring that it receives no mail. That is a deliberate, healthy posture for a non-mail domain.",
                domain
            ),
            vec![".".to_string()],
        ),
        MxPosture::NoRecords => (
            format!(
                "No MX records at {}. That is not a defect: the domain simply does not advertise inbound mail servers. It could still send mail, so the SPF and DMARC checks stay relevant at reduced severity.",
                domain
            ),
            Vec::new(),
        ),
    };

    vec![CheckResult {
        check_id: MX_CHECK_ID.into(),
        category: ScanCategory::Security,
        title: MX_TITLE.into(),
        description,
        status: CheckStatus::Pass,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "domain": domain,
            "has_mx": has_mx,
            "mx_hosts": hosts,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }]
}

// security.dns.dnssec

pub const DNSSEC_CHECK_ID: &str = "security.dns.dnssec";
pub const DNSSEC_TITLE: &str = "DNSSEC";

pub fn dnssec_records_result(check_id: &str, domain: &str, dnskey_count: usize) -> CheckResult {
    if dnskey_count > 0 {
        CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: "DNSSEC keys published".into(),
            description: format!(
                "{} publishes {} DNSKEY record{}. This confirms key publication only; it does not verify RRSIG coverage, algorithm or rollover health, or a valid DS chain from the parent zone.",
                domain,
                dnskey_count,
                if dnskey_count == 1 { "" } else { "s" }
            ),
            status: CheckStatus::Pass,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "domain": domain,
                "dnskey_count": dnskey_count,
            })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    } else {
        CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: "DNSSEC not enabled".into(),
            description: format!(
                "{} publishes no DNSKEY records, so validating resolvers cannot authenticate this zone through DNSSEC. DNSSEC is optional hardening, and whether it can be enabled depends on coordinated support from the authoritative DNS host and parent-zone registrar.",
                domain
            ),
            status: CheckStatus::Warn,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: Some("Confirm that both the authoritative DNS provider and registrar support DNSSEC. Enable zone signing at the DNS provider first, then publish exactly the DS material it supplies through the registrar and validate the full chain. For a later DNS-provider migration, follow both providers' documented multi-signer or double-sign rollover when supported. If a signed rollover is unavailable, use their documented transition-to-unsigned sequence, including DS removal and cache/TTL waiting, before changing nameservers; verify resolution and the chain at every stage.".into()),
            raw_data: Some(serde_json::json!({
                "domain": domain,
                "dnskey_count": 0,
            })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("DNSSEC lets validating resolvers detect DNS data that does not authenticate to the zone's signed chain. It reduces some spoofing and cache-poisoning risk but does not prevent registrar, authoritative-provider, or correctly signed account compromise.".into()),
        }
    }
}

/// Grade the apex DNSKEY answer, delivered as a record count because key
/// material itself never rides into the verdict.
pub fn evaluate_dnssec(domain: &str, dnskey: DnsOutcome<usize>) -> Vec<CheckResult> {
    match dnskey {
        DnsOutcome::Failed(detail) => {
            vec![skipped_dns_failure(
                DNSSEC_CHECK_ID,
                DNSSEC_TITLE,
                domain,
                &detail,
            )]
        }
        DnsOutcome::Records(count) => {
            vec![dnssec_records_result(DNSSEC_CHECK_ID, domain, count)]
        }
        DnsOutcome::NoRecords => vec![dnssec_records_result(DNSSEC_CHECK_ID, domain, 0)],
    }
}

// security.dns.caa

pub const CAA_CHECK_ID: &str = "security.dns.caa";
pub const CAA_TITLE: &str = "CAA records";

/// The CA names allowed to issue for the domain (issue + issuewild tags).
pub fn caa_issuers(records: &[CaaRecord]) -> Vec<String> {
    records
        .iter()
        .filter(|record| record.tag == "issue" || record.tag == "issuewild")
        .map(|record| record.value.clone())
        .collect()
}

/// Grade CAA records by whether `issue` or `issuewild` restricts issuance.
pub fn caa_records_result(check_id: &str, domain: &str, records: &[CaaRecord]) -> CheckResult {
    let issuers = caa_issuers(records);
    let restricts_issuance = !issuers.is_empty();
    let raw_data = serde_json::json!({
        "domain": domain,
        "caa_records": records.iter().map(|record| format!("{} {}", record.tag, record.value)).collect::<Vec<_>>(),
        "issuers": issuers,
    });
    if restricts_issuance {
        CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: CAA_TITLE.into(),
            description: format!(
                "{} publishes CAA issue/issuewild authorizations naming: {}. These are issuance-time requests to compliant public certificate authorities, not a browser certificate-validation control.",
                domain,
                issuers.join(", ")
            ),
            status: CheckStatus::Pass,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    } else {
        CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: "CAA records do not restrict issuance".into(),
            description: format!(
                "{} publishes CAA records, but none carries an issue or issuewild tag. Under RFC 8659, an iodef-only or otherwise non-restricting CAA set places no CAA authorization limit on which compliant public certificate authority may issue.",
                domain
            ),
            status: CheckStatus::Warn,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: Some(format!(
                "Keep the existing iodef contact, and add one CAA issue record per CA you actually use, for example `{} CAA 0 issue \"letsencrypt.org\"`. Check each hosting/CDN provider's current issuer identifiers and include every required CA before rollout; an omitted authorized issuer can cause later issuance or renewal to fail.",
                domain
            )),
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("CAA gives compliant public CAs an issuance-time authorization signal that can reduce unintended mis-issuance. It does not constrain an authorized CA's mistakes, prevent noncompliance, or change how browsers validate an already issued certificate; iodef only supplies a reporting destination.".into()),
        }
    }
}

pub fn evaluate_caa(domain: &str, caa: DnsOutcome<Vec<CaaRecord>>) -> Vec<CheckResult> {
    match caa {
        DnsOutcome::Failed(detail) => {
            vec![skipped_dns_failure(CAA_CHECK_ID, CAA_TITLE, domain, &detail)]
        }
        DnsOutcome::Records(records) => {
            vec![caa_records_result(CAA_CHECK_ID, domain, &records)]
        }
        DnsOutcome::NoRecords => vec![CheckResult {
            check_id: CAA_CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "No CAA records".into(),
            description: format!(
                "{} has no CAA records, so it publishes no CAA authorization constraint on certificate issuance. A CAA issue/issuewild policy lets the domain name holder name authorized issuers for compliant public CAs to check before issuance.",
                domain
            ),
            status: CheckStatus::Warn,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: Some(format!(
                "Inventory every CA your host, CDN, and certificate automation may use from their current documentation and the active certificate. Then publish one CAA issue record per authorized issuer, for example `{} CAA 0 issue \"letsencrypt.org\"`. Stage and verify the policy before rollout; omitting a required issuer can cause later issuance or renewal to fail.",
                domain
            )),
            raw_data: Some(serde_json::json!({
                "domain": domain,
                "caa_records": [],
            })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("CAA supplies compliant public CAs with an issuance-time authorization signal that can reduce unintended mis-issuance. It is not enforced by browsers when validating existing certificates and does not eliminate authorized-CA or noncompliance risk.".into()),
        }],
    }
}

#[cfg(test)]
#[path = "records_tests.rs"]
mod tests;
