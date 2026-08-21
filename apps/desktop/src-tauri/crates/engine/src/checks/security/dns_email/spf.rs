//! SPF grading against RFC 7208. Missing-record severity is MX-gated, and the
//! mechanism count is a lower-bound static approximation.

use super::{has_mx_from, missing_record_severity, skipped_dns_failure};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::dns::{DnsOutcome, MxRecord};

pub const CHECK_ID: &str = "security.dns.spf";
pub const TITLE: &str = "SPF record";

/// True when the TXT record is an SPF version-1 record (first term is v=spf1).
pub fn is_spf_record(record: &str) -> bool {
    record
        .split_whitespace()
        .next()
        .is_some_and(|first| first.eq_ignore_ascii_case("v=spf1"))
}

pub struct SpfAnalysis {
    /// Direct count of DNS-querying terms (include, a, mx, ptr, exists,
    /// redirect). Nested includes are not resolved, so this is "at least N".
    pub lookup_mechanisms: u32,
    /// The record contains +all (or a bare all, which defaults to +): every
    /// server on the internet is authorized to send as this domain.
    pub allows_any_sender: bool,
    /// Qualifier on the all term, if one is present: '+', '-', '~', or '?'.
    pub all_qualifier: Option<char>,
    /// A redirect= modifier delegates the policy to another domain.
    pub has_redirect: bool,
}

pub fn analyze_spf(record: &str) -> SpfAnalysis {
    let mut analysis = SpfAnalysis {
        lookup_mechanisms: 0,
        allows_any_sender: false,
        all_qualifier: None,
        has_redirect: false,
    };

    for term in record.split_whitespace().skip(1) {
        let lower = term.to_ascii_lowercase();

        // Modifiers are name=value. redirect= costs a DNS lookup at
        // evaluation time; exp= and unknown modifiers do not (RFC 7208 4.6.4).
        if lower.starts_with("redirect=") {
            analysis.lookup_mechanisms += 1;
            analysis.has_redirect = true;
            continue;
        }
        if lower.contains('=') {
            continue;
        }

        let (qualifier, body) = match lower.chars().next() {
            Some(q @ ('+' | '-' | '~' | '?')) => (q, &lower[1..]),
            _ => ('+', lower.as_str()),
        };
        let mechanism = body.split([':', '/']).next().unwrap_or("");
        match mechanism {
            "include" | "a" | "mx" | "ptr" | "exists" => analysis.lookup_mechanisms += 1,
            "all" => {
                analysis.all_qualifier = Some(qualifier);
                if qualifier == '+' {
                    analysis.allows_any_sender = true;
                }
            }
            _ => {}
        }
    }

    analysis
}

/// RFC 7208 caps SPF evaluation at 10 DNS-querying terms.
const SPF_LOOKUP_LIMIT: u32 = 10;
/// Direct counts at or above this get a "nearing the limit" note.
const SPF_LOOKUP_NEAR_LIMIT: u32 = 8;

fn all_qualifier_note(analysis: &SpfAnalysis) -> String {
    match analysis.all_qualifier {
        Some('-') => {
            " It ends with -all, so receivers are told to reject mail from unlisted servers."
                .to_string()
        }
        Some('~') => {
            " It ends with ~all (softfail), so receivers are told to treat mail from unlisted servers with suspicion."
                .to_string()
        }
        Some('?') => {
            " It ends with ?all (neutral), which gives receivers no enforcement signal for unlisted servers; consider ~all or -all once every legitimate sender is listed."
                .to_string()
        }
        Some(_) => String::new(),
        None if analysis.has_redirect => {
            " It delegates its policy via redirect=, so the target domain's record decides what happens to unlisted servers."
                .to_string()
        }
        None => {
            " It has no all term, so receivers get no explicit policy for unlisted servers."
                .to_string()
        }
    }
}

/// What the SPF verdict needs next after the apex TXT answer.
pub enum SpfStep {
    Done(Vec<CheckResult>),
    /// No SPF record was found; the missing-record verdict is MX-gated, so
    /// the runtime must answer one MX question at the apex.
    NeedsMx(MissingSpf),
}

/// The pending missing-record verdict, waiting on the apex MX answer.
pub struct MissingSpf {
    domain: String,
    txt_records_scanned: usize,
}

impl MissingSpf {
    /// The apex name whose MX records gate the severity.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn evaluate(self, mx: &DnsOutcome<Vec<MxRecord>>) -> Vec<CheckResult> {
        let has_mx = has_mx_from(mx);
        let mx_note = if has_mx == Some(true) {
            " This domain has MX records, so it handles mail and is a natural spoofing target."
        } else {
            ""
        };
        vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "No SPF record".into(),
            description: format!(
                "No TXT record starting with v=spf1 was found at {}. SPF lets receivers evaluate whether the connecting host is authorized to use the domain in the SMTP MAIL FROM or HELO identity. It does not by itself authenticate the user-visible From header; DMARC alignment addresses that separate identity. A domain that sends mail needs a record matching its current senders, while a domain that sends no mail can publish a null SPF policy.{}",
                self.domain, mx_note
            ),
            status: CheckStatus::Warn,
            severity: missing_record_severity(has_mx),
            fix_prompt: None,
            manual_fix: Some(format!(
                "Inventory every service that sends with this domain in MAIL FROM or HELO, then publish exactly one SPF TXT record at {} using each provider's current documented mechanism and the narrowest appropriate `all` policy. Keep the evaluation within SPF's DNS-lookup limits. If the domain sends no mail, publish `v=spf1 -all`; configure aligned DKIM and DMARC separately for protection of the visible From domain.",
                self.domain
            )),
            raw_data: Some(serde_json::json!({
                "domain": self.domain,
                "txt_records_scanned": self.txt_records_scanned,
                "has_mx": has_mx,
            })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("Without an SPF policy, receivers cannot use SPF to distinguish authorized from unauthorized hosts for this domain's SMTP envelope/HELO identity; visible From-domain protection also requires aligned DKIM or SPF under DMARC.".into()),
        }]
    }
}

/// Grade the apex TXT answer. Completes for every present-record posture;
/// asks for the MX answer when no SPF record exists.
pub fn evaluate_spf_txt(domain: &str, txt: DnsOutcome<Vec<String>>) -> SpfStep {
    let records = match txt {
        DnsOutcome::Failed(detail) => {
            return SpfStep::Done(vec![skipped_dns_failure(CHECK_ID, TITLE, domain, &detail)])
        }
        DnsOutcome::NoRecords => Vec::new(),
        DnsOutcome::Records(records) => records,
    };
    let spf_records: Vec<&String> = records.iter().filter(|r| is_spf_record(r)).collect();

    if spf_records.is_empty() {
        return SpfStep::NeedsMx(MissingSpf {
            domain: domain.to_string(),
            txt_records_scanned: records.len(),
        });
    }

    if spf_records.len() > 1 {
        return SpfStep::Done(vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "Multiple SPF records".into(),
            description: format!(
                "{} TXT records at {} start with v=spf1. RFC 7208 requires exactly one; receivers treat multiple SPF records as a permanent error (permerror), which can make your legitimate mail fail SPF entirely.",
                spf_records.len(),
                domain
            ),
            status: CheckStatus::Fail,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: Some("Merge them into a single record: keep one v=spf1, combine every include and mechanism from the others, and end with one all term, for example `v=spf1 include:_spf.google.com include:sendgrid.net -all`. Then delete the extra records.".into()),
            raw_data: Some(serde_json::json!({
                "domain": domain,
                "spf_records": spf_records,
            })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("Two SPF records evaluate as a permanent error, so receivers may treat all your mail as failing SPF.".into()),
        }]);
    }

    let record = spf_records[0].clone();
    let analysis = analyze_spf(&record);
    let raw_data = serde_json::json!({
        "domain": domain,
        "spf_record": record,
        "lookup_mechanisms": analysis.lookup_mechanisms,
        "all_qualifier": analysis.all_qualifier.map(String::from),
        "has_redirect": analysis.has_redirect,
    });

    if analysis.allows_any_sender {
        return SpfStep::Done(vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "SPF record allows any sender (+all)".into(),
            description: format!(
                "The SPF record at {} ends with +all (a bare all counts as +all), which authorizes every server on the internet to send mail as your domain. SPF then passes for spoofed mail too, defeating the record's purpose.",
                domain
            ),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: Some("Replace +all with -all (or ~all while you confirm every legitimate sender is listed), for example `v=spf1 include:_spf.google.com -all`.".into()),
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("An SPF record ending in +all validates spoofed mail as if it were yours.".into()),
        }]);
    }

    if analysis.lookup_mechanisms > SPF_LOOKUP_LIMIT {
        return SpfStep::Done(vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "SPF record exceeds the 10 DNS lookup limit".into(),
            description: format!(
                "The SPF record at {} contains at least {} DNS-querying terms (include, a, mx, ptr, exists, redirect). RFC 7208 caps SPF evaluation at 10 DNS lookups; over the limit, receivers return a permanent error and the record stops working. The true count can only be higher, because each include performs its own lookups when receivers evaluate it.",
                domain, analysis.lookup_mechanisms
            ),
            status: CheckStatus::Fail,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: Some("Trim the record: remove includes for providers you no longer send through, replace a/mx terms with ip4:/ip6: blocks where the addresses are stable, or use an SPF flattening service. Aim to stay comfortably under 10 lookups.".into()),
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("An over-limit SPF record evaluates as a permanent error, so receivers may treat legitimate mail as failing SPF.".into()),
        }]);
    }

    let near_limit_note = if analysis.lookup_mechanisms >= SPF_LOOKUP_NEAR_LIMIT {
        format!(
            " It already uses at least {} of the 10 allowed DNS lookups (nested includes add more), so adding another sender may push it over the limit.",
            analysis.lookup_mechanisms
        )
    } else {
        String::new()
    };

    SpfStep::Done(vec![CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Security,
        title: TITLE.into(),
        description: format!(
            "SPF record found at {}: `{}`.{}{}",
            domain,
            record,
            all_qualifier_note(&analysis),
            near_limit_note
        ),
        status: CheckStatus::Pass,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(raw_data),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }])
}

#[cfg(test)]
#[path = "spf_tests.rs"]
mod tests;
