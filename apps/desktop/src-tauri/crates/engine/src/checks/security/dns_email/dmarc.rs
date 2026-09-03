//! DMARC policy grading against RFC 7489. Graded postures are MX-gated, while
//! failed TXT lookups remain skipped.

use super::{has_mx_from, missing_record_severity, skipped_dns_failure};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::dns::{DnsOutcome, MxRecord};

pub const CHECK_ID: &str = "security.dns.dmarc";
pub const TITLE: &str = "DMARC record";

/// The name whose TXT records carry the domain's DMARC policy.
pub fn dmarc_lookup_name(domain: &str) -> String {
    format!("_dmarc.{}", domain)
}

/// True when the TXT record is a DMARC record (first tag is v=DMARC1).
pub fn is_dmarc_record(record: &str) -> bool {
    record
        .split(';')
        .next()
        .map(str::trim)
        .is_some_and(|first| first.eq_ignore_ascii_case("v=dmarc1"))
}

/// Extract a DMARC tag value (lowercased) from a record, e.g. tag "p" from
/// "v=DMARC1; p=quarantine; rua=mailto:reports@example.com".
pub fn dmarc_tag(record: &str, key: &str) -> Option<String> {
    record
        .split(';')
        .filter_map(|part| part.split_once('='))
        .find_map(|(tag, value)| {
            if tag.trim().eq_ignore_ascii_case(key) {
                Some(value.trim().to_ascii_lowercase())
            } else {
                None
            }
        })
}

pub enum DmarcEvaluation {
    /// Monitoring only: receivers report but deliver failing mail normally.
    PolicyNone,
    /// p=quarantine or p=reject: receivers act on failing mail.
    PolicyEnforced { policy: String },
    /// A DMARC record exists but is unusable (no p=, duplicate records,
    /// or an unknown policy value).
    Malformed { reason: String },
    /// Only unrelated TXT records (e.g. a stray verification token) live at
    /// the _dmarc name. Receivers ignore them, so the domain effectively
    /// has no DMARC record - it must not be called "malformed DMARC"
    ///.
    NoDmarcRecord,
}

/// Evaluate the TXT records found at the _dmarc name. The caller handles the
/// no-records case (missing) separately.
pub fn evaluate_dmarc(records: &[String]) -> DmarcEvaluation {
    let dmarc_records: Vec<&String> = records.iter().filter(|r| is_dmarc_record(r)).collect();

    if dmarc_records.is_empty() {
        return DmarcEvaluation::NoDmarcRecord;
    }
    if dmarc_records.len() > 1 {
        return DmarcEvaluation::Malformed {
            reason: "more than one v=DMARC1 record is published; receivers treat duplicates as if no record existed".into(),
        };
    }

    match dmarc_tag(dmarc_records[0], "p").as_deref() {
        None => DmarcEvaluation::Malformed {
            reason: "the record is missing the required p= policy tag".into(),
        },
        Some("none") => DmarcEvaluation::PolicyNone,
        Some(policy @ ("quarantine" | "reject")) => DmarcEvaluation::PolicyEnforced {
            policy: policy.to_string(),
        },
        Some(other) => DmarcEvaluation::Malformed {
            reason: format!("unknown policy value p={}", other),
        },
    }
}

/// What the DMARC verdict needs next after the _dmarc TXT answer.
pub enum DmarcStep {
    Done(Vec<CheckResult>),
    /// Every graded posture is MX-gated, so the runtime must answer one MX
    /// question at the apex.
    NeedsMx(PendingDmarc),
}

/// The pending DMARC verdict, waiting on the apex MX answer.
pub struct PendingDmarc {
    domain: String,
    records: Vec<String>,
}

impl PendingDmarc {
    /// The apex name whose MX records gate the severity.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn evaluate(self, mx: &DnsOutcome<Vec<MxRecord>>) -> Vec<CheckResult> {
        let dmarc_name = dmarc_lookup_name(&self.domain);
        vec![dmarc_result(
            CHECK_ID,
            &self.domain,
            &dmarc_name,
            &self.records,
            has_mx_from(mx),
        )]
    }
}

/// Grade the _dmarc TXT answer: a failed lookup completes as Skipped, every
/// other posture waits on the MX gate.
pub fn evaluate_dmarc_txt(domain: &str, txt: DnsOutcome<Vec<String>>) -> DmarcStep {
    let records = match txt {
        DnsOutcome::Failed(detail) => {
            return DmarcStep::Done(vec![skipped_dns_failure(
                CHECK_ID,
                TITLE,
                &dmarc_lookup_name(domain),
                &detail,
            )])
        }
        DnsOutcome::NoRecords => Vec::new(),
        DnsOutcome::Records(records) => records,
    };
    DmarcStep::NeedsMx(PendingDmarc {
        domain: domain.to_string(),
        records,
    })
}

/// Build the missing-DMARC result, preserving any unrelated TXT evidence.
fn no_dmarc_result(
    check_id: &str,
    domain: &str,
    dmarc_name: &str,
    unrelated_records: &[String],
    has_mx: Option<bool>,
) -> CheckResult {
    let mx_note = match has_mx {
        Some(true) => {
            " This domain publishes MX records, which confirms that it receives mail; this scan does not establish whether it also sends mail."
        }
        Some(false) => {
            " This domain publishes no MX records, so it shows no inbound mail setup; a domain that sends no mail can publish `v=DMARC1; p=reject` directly so receivers reject anyone using it as a visible From domain."
        }
        None => "",
    };
    let lead = if unrelated_records.is_empty() {
        format!("No TXT record at {}.", dmarc_name)
    } else {
        format!(
            "The TXT record{} at {} {} unrelated to DMARC (none starts with v=DMARC1), so receivers treat this domain as having no DMARC record.",
            if unrelated_records.len() == 1 { "" } else { "s" },
            dmarc_name,
            if unrelated_records.len() == 1 { "is" } else { "are" },
        )
    };
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Security,
        title: "No DMARC record".into(),
        description: format!(
            "{} Without DMARC, the domain publishes no alignment-based request for receivers to quarantine or reject messages that use its visible From domain. Receivers can still apply their own filtering and authentication policy.{}",
            lead, mx_note
        ),
        status: CheckStatus::Warn,
        severity: missing_record_severity(has_mx),
        fix_prompt: None,
        manual_fix: Some(format!(
            "Publish a TXT record at {} starting in monitoring mode: `v=DMARC1; p=none; rua=mailto:dmarc-reports@{}`. Review the aggregate reports to confirm your real senders pass, then graduate the policy to p=quarantine and finally p=reject.",
            dmarc_name, domain
        )),
        raw_data: Some(serde_json::json!({
            "domain": domain,
            "lookup_name": dmarc_name,
            "records": unrelated_records,
            "has_mx": has_mx,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: Some("DMARC connects SPF or DKIM alignment to the visible From domain and publishes the domain owner's requested disposition. Without it, receivers have no domain-published DMARC disposition to apply, although their own filtering still governs delivery.".into()),
    }
}

/// Grade DMARC TXT records with MX-aware severity.
pub fn dmarc_result(
    check_id: &str,
    domain: &str,
    dmarc_name: &str,
    records: &[String],
    has_mx: Option<bool>,
) -> CheckResult {
    if records.is_empty() {
        return no_dmarc_result(check_id, domain, dmarc_name, records, has_mx);
    }

    let raw_data = serde_json::json!({
        "domain": domain,
        "lookup_name": dmarc_name,
        "records": records,
        "has_mx": has_mx,
    });

    match evaluate_dmarc(records) {
        DmarcEvaluation::NoDmarcRecord => {
            no_dmarc_result(check_id, domain, dmarc_name, records, has_mx)
        }
        DmarcEvaluation::PolicyNone => CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: "DMARC policy is monitoring only (p=none)".into(),
            description: format!(
                "The DMARC record at {} sets p=none. The requested DMARC disposition is monitoring only: the domain requests no quarantine or reject treatment for messages that fail DMARC alignment, while receivers can still apply their own local filtering. Monitoring is a valid rollout or diagnostic posture. If the domain's objective is active anti-spoofing enforcement, review alignment data before moving deliberately toward quarantine or reject.",
                dmarc_name
            ),
            status: CheckStatus::Warn,
            severity: missing_record_severity(has_mx),
            fix_prompt: None,
            manual_fix: Some("Review the aggregate reports (the rua= address) for a few weeks to confirm your real senders pass SPF or DKIM alignment. Then tighten in steps: move to `p=quarantine` (optionally starting with pct=25 and ramping up), and finally `p=reject` once quarantine shows no legitimate mail failing.".into()),
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("A p=none policy provides visibility but requests no DMARC quarantine or reject treatment for alignment failures, so it should be evaluated against the domain's intended enforcement posture.".into()),
        },
        DmarcEvaluation::PolicyEnforced { policy } => CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: TITLE.into(),
            description: format!(
                "The DMARC record at {} publishes p={}, requesting receivers to {} messages that fail DMARC alignment. A receiver can still apply its own local policy.",
                dmarc_name,
                policy,
                if policy == "reject" { "reject" } else { "quarantine" }
            ),
            status: CheckStatus::Pass,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        },
        DmarcEvaluation::Malformed { reason } => CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: "Malformed DMARC record".into(),
            description: format!(
                "The DMARC record at {} is not usable: {}. Receivers that cannot parse a valid policy fall back to treating the domain as having no DMARC at all.",
                dmarc_name, reason
            ),
            // MX-gated like the missing branch: on a domain that receives
            // no mail, a broken record must not outscore having no record.
            status: if has_mx == Some(true) {
                CheckStatus::Fail
            } else {
                CheckStatus::Warn
            },
            severity: missing_record_severity(has_mx),
            fix_prompt: None,
            manual_fix: Some(format!(
                "Publish exactly one TXT record at {} that starts with v=DMARC1 and includes a policy tag, for example `v=DMARC1; p=quarantine; rua=mailto:dmarc-reports@{}`.",
                dmarc_name, domain
            )),
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("A DMARC record that receivers cannot parse does not publish a usable alignment policy, even though the DNS record exists.".into()),
        },
    }
}

#[cfg(test)]
#[path = "dmarc_tests.rs"]
mod tests;
