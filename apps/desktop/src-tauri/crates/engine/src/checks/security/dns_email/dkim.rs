//! Grades common DKIM selector records after MX and SPF establish mail posture.
//!
//! Published keys do not prove message signing, validation, or DMARC alignment.

use super::{has_mx_from, skipped_dns_failure, spf_posture_from, SpfPosture};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::dns::{DnsOutcome, MxRecord};

pub const CHECK_ID: &str = "security.dns.dkim";
pub const TITLE: &str = "DKIM selectors";

/// Common DKIM selector names, covering the default names used by major mail
/// providers (Google Workspace, Microsoft 365, Mailchimp/Mandrill, Postmark,
/// Zoho, Fastmail, and generic k1/s1-style ESP defaults).
pub const COMMON_SELECTORS: [&str; 16] = [
    "default",
    "google",
    "k1",
    "k2",
    "k3",
    "s1",
    "s2",
    "selector1",
    "selector2",
    "mail",
    "smtp",
    "dkim",
    "mandrill",
    "pm",
    "zoho",
    "fm1",
];

/// The TXT name a selector's key record lives at.
pub fn selector_lookup_name(selector: &str, domain: &str) -> String {
    format!("{}._domainkey.{}", selector, domain)
}

/// True when a TXT record looks like a DKIM key record: a v=DKIM1 version tag
/// or a p= key tag (an empty p= is a revoked key, which still proves DKIM is
/// configured for the selector).
pub fn looks_like_dkim_record(record: &str) -> bool {
    record.split(';').map(str::trim).any(|tag| {
        let lower = tag.to_ascii_lowercase();
        lower.starts_with("v=dkim1") || lower.starts_with("p=")
    })
}

/// True when a DKIM key record is revoked: the p= tag is present but empty
/// (RFC 6376 section 3.6.1). Receivers can NOT verify signatures with a
/// revoked key - revocation is how a retired selector is decommissioned.
pub fn dkim_record_is_revoked(record: &str) -> bool {
    let mut has_key_tag = false;
    let mut key_is_empty = false;
    for tag in record.split(';') {
        if let Some((key, value)) = tag.split_once('=') {
            if key.trim().eq_ignore_ascii_case("p") {
                has_key_tag = true;
                key_is_empty = value.trim().is_empty();
            }
        }
    }
    has_key_tag && key_is_empty
}

/// A usable selector needs a non-empty public-key (`p=`) tag. A bare
/// `v=DKIM1` declaration is not active key material, and an empty `p=` is an
/// explicit revocation.
pub fn dkim_record_has_active_key(record: &str) -> bool {
    record.split(';').any(|tag| {
        tag.split_once('=').is_some_and(|(key, value)| {
            key.trim().eq_ignore_ascii_case("p") && !value.trim().is_empty()
        })
    })
}

/// Pass copy for found selectors, split because an empty `p=` key is revoked
/// and cannot verify signatures.
pub fn dkim_found_description(domain: &str, active: &[&str], revoked: &[&str]) -> String {
    let total = active.len() + revoked.len();
    let all: Vec<&str> = active.iter().chain(revoked.iter()).copied().collect();
    let mut description = format!(
        "Found {} for selector{} {} under _domainkey.{}.",
        if total == 1 {
            "a DKIM key record"
        } else {
            "DKIM key records"
        },
        if total == 1 { "" } else { "s" },
        all.join(", "),
        domain
    );
    if !active.is_empty() {
        description.push_str(&format!(
            " The {} selector{} {} key material through a non-empty p= value that receivers can use to verify a matching signature. This DNS observation does not confirm that outbound messages are signed, that signatures validate, or that the signing domain aligns with the visible From domain.",
            active.join(", "),
            if active.len() == 1 { "" } else { "s" },
            if active.len() == 1 { "publishes" } else { "publish" },
        ));
    }
    if !revoked.is_empty() {
        description.push_str(&format!(
            " The key{} for {} {} revoked (empty p=), so receivers cannot verify signatures made with {}; publishing an empty p= is the standard way to retire a selector.",
            if revoked.len() == 1 { "" } else { "s" },
            revoked.join(", "),
            if revoked.len() == 1 { "is" } else { "are" },
            if revoked.len() == 1 { "it" } else { "them" },
        ));
    }
    description
}

pub enum SweepDecision {
    /// Domain shows mail configuration (MX, or an SPF record that
    /// authorizes senders): probe the selectors.
    Sweep,
    /// Domain authoritatively has neither MX nor SPF: skip 16 pointless
    /// lookups and report not-applicable.
    NotApplicable,
    /// No real MX and the SPF null record (`v=spf1 -all`): the domain
    /// explicitly declares it sends no mail, so missing DKIM selectors are
    /// the expected posture, not a gap.
    DeclaredNoMail,
    /// Both gating lookups failed: DNS is unavailable, report Skipped.
    DnsUnavailable,
}

pub fn sweep_decision(has_mx: Option<bool>, spf: Option<SpfPosture>) -> SweepDecision {
    match (has_mx, spf) {
        (Some(true), _) | (_, Some(SpfPosture::Present)) => SweepDecision::Sweep,
        (_, Some(SpfPosture::NullRecord)) => SweepDecision::DeclaredNoMail,
        (None, None) => SweepDecision::DnsUnavailable,
        _ => SweepDecision::NotApplicable,
    }
}

/// What the DKIM verdict needs next after the two gating answers.
pub enum DkimStep {
    Done(Vec<CheckResult>),
    /// The domain shows mail configuration: the runtime must answer one TXT
    /// question per name in [`DkimSweep::probe_names`].
    Sweep(DkimSweep),
}

/// The pending selector sweep, waiting on the per-selector TXT answers.
pub struct DkimSweep {
    domain: String,
    spf_is_null: bool,
    spf_posture_label: Option<&'static str>,
}

fn spf_posture_label(posture: &Option<SpfPosture>) -> Option<&'static str> {
    match posture {
        Some(SpfPosture::Present) => Some("present"),
        Some(SpfPosture::NullRecord) => Some("null_record"),
        Some(SpfPosture::Absent) => Some("absent"),
        None => None,
    }
}

/// Grade the two gating answers: MX at the apex and TXT at the apex.
pub fn evaluate_dkim_gate(
    domain: &str,
    mx: &DnsOutcome<Vec<MxRecord>>,
    apex_txt: &DnsOutcome<Vec<String>>,
) -> DkimStep {
    let has_mx = has_mx_from(mx);
    let spf_posture = spf_posture_from(apex_txt);
    let spf_is_null = matches!(spf_posture, Some(SpfPosture::NullRecord));
    let label = spf_posture_label(&spf_posture);
    match sweep_decision(has_mx, spf_posture) {
        SweepDecision::DnsUnavailable => DkimStep::Done(vec![skipped_dns_failure(
            CHECK_ID,
            TITLE,
            domain,
            "the MX and TXT lookups that gate the selector sweep both failed",
        )]),
        SweepDecision::NotApplicable => DkimStep::Done(vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "DKIM probe not applicable".into(),
            description: format!(
                "{} has no MX records and no SPF record, so it does not look like a mail-handling domain. Skipped probing {} common DKIM selector names.",
                domain,
                COMMON_SELECTORS.len()
            ),
            status: CheckStatus::Pass,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "domain": domain,
                "probed": false,
                "has_mx": has_mx,
                "spf_posture": label,
            })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }]),
        SweepDecision::DeclaredNoMail => DkimStep::Done(vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "DKIM not expected: domain declares it sends no mail".into(),
            description: format!(
                "{} publishes the SPF null record (v=spf1 -all), explicitly declaring that it sends no mail, and has no mail-receiving MX records. DKIM signs outbound mail, so missing DKIM selectors are the correct posture here. Skipped probing {} common DKIM selector names.",
                domain,
                COMMON_SELECTORS.len()
            ),
            status: CheckStatus::Pass,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "domain": domain,
                "probed": false,
                "has_mx": has_mx,
                "spf_posture": label,
            })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }]),
        SweepDecision::Sweep => DkimStep::Sweep(DkimSweep {
            domain: domain.to_string(),
            spf_is_null,
            spf_posture_label: label,
        }),
    }
}

impl DkimSweep {
    /// The (selector, lookup name) pairs the runtime must answer, in the
    /// vendored order.
    pub fn probe_names(&self) -> Vec<(&'static str, String)> {
        COMMON_SELECTORS
            .iter()
            .map(|selector| (*selector, selector_lookup_name(selector, &self.domain)))
            .collect()
    }

    /// Grade the per-selector TXT answers, one per entry of
    /// [`DkimSweep::probe_names`] in the same order.
    pub fn evaluate(self, outcomes: &[(String, DnsOutcome<Vec<String>>)]) -> Vec<CheckResult> {
        let domain = &self.domain;
        let mut active: Vec<&str> = Vec::new();
        let mut revoked: Vec<&str> = Vec::new();
        let mut malformed: Vec<&str> = Vec::new();
        let mut failed_probes = 0usize;
        for (selector, outcome) in outcomes {
            match outcome {
                DnsOutcome::Records(records) => {
                    let dkim_records: Vec<&String> = records
                        .iter()
                        .filter(|record| looks_like_dkim_record(record))
                        .collect();
                    if dkim_records.is_empty() {
                        continue;
                    }
                    if dkim_records
                        .iter()
                        .any(|record| dkim_record_has_active_key(record))
                    {
                        active.push(selector);
                    } else if dkim_records
                        .iter()
                        .any(|record| dkim_record_is_revoked(record))
                    {
                        revoked.push(selector);
                    } else {
                        malformed.push(selector);
                    }
                }
                DnsOutcome::NoRecords => {}
                DnsOutcome::Failed(_) => failed_probes += 1,
            }
        }
        if active.is_empty()
            && revoked.is_empty()
            && malformed.is_empty()
            && failed_probes == outcomes.len()
        {
            return vec![skipped_dns_failure(
                CHECK_ID,
                TITLE,
                domain,
                "every selector probe failed",
            )];
        }

        if !active.is_empty() {
            return vec![CheckResult {
                check_id: CHECK_ID.into(),
                category: ScanCategory::Security,
                title: TITLE.into(),
                description: dkim_found_description(domain, &active, &revoked),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "domain": domain,
                    "probed": true,
                    "selectors_found": active,
                    "selectors_revoked": revoked,
                    "selectors_malformed": malformed,
                    "selectors_probed": COMMON_SELECTORS.len(),
                    "failed_probes": failed_probes,
                })),
                confidence: IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        if !revoked.is_empty() || !malformed.is_empty() {
            let mut details = Vec::new();
            if !revoked.is_empty() {
                details.push(format!(
                    "{} {} revoked with an empty p= value",
                    revoked.join(", "),
                    if revoked.len() == 1 { "is" } else { "are" }
                ));
            }
            if !malformed.is_empty() {
                details.push(format!(
                    "{} {} a DKIM-like record but no non-empty p= public key",
                    malformed.join(", "),
                    if malformed.len() == 1 { "has" } else { "have" }
                ));
            }
            return vec![CheckResult {
                check_id: CHECK_ID.into(),
                category: ScanCategory::Security,
                title: "No active DKIM key found among common selectors".into(),
                description: format!(
                    "The common-selector probe found no active public key under _domainkey.{}: {}. A different custom selector may still be active, so confirm against a recently delivered message or the sending provider before changing DNS.",
                    domain,
                    details.join("; ")
                ),
                status: CheckStatus::Warn,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: Some(format!(
                    "Inspect a recent message's DKIM-Signature header to identify the actual selector (`s=`) and signing domain (`d=`), then query `<selector>._domainkey.{}`. If the sending provider reports DKIM disabled or the referenced selector has no non-empty key, enable/rotate DKIM using the exact DNS record that provider supplies.",
                    domain
                )),
                raw_data: Some(serde_json::json!({
                    "domain": domain,
                    "probed": true,
                    "selectors_found": [],
                    "selectors_revoked": revoked,
                    "selectors_malformed": malformed,
                    "selectors_probed": COMMON_SELECTORS.len(),
                    "failed_probes": failed_probes,
                })),
                confidence: IssueConfidence::NeedsReview,
                confidence_reason: Some("The observed common selectors have no active public key, but a custom selector outside the probe list may be active.".into()),
                why_it_matters: Some("If the domain sends mail without a valid, aligned DKIM signature, forwarding can break SPF-based authentication and leave DMARC with fewer aligned authentication paths.".into()),
            }];
        }

        if self.spf_is_null {
            // The sweep ran (MX present) but the domain's SPF is the null
            // record: it declares it sends no mail, so absent DKIM
            // selectors are consistent, not a warning.
            return vec![CheckResult {
                check_id: CHECK_ID.into(),
                category: ScanCategory::Security,
                title: "No DKIM selectors; domain declares it sends no mail".into(),
                description: format!(
                    "None of {} common DKIM selector names has a key record under _domainkey.{}. This domain's SPF record is the null record (v=spf1 -all), declaring it sends no mail, and DKIM signs outbound mail - so missing selectors are consistent with that posture.",
                    COMMON_SELECTORS.len(),
                    domain
                ),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "domain": domain,
                    "probed": true,
                    "selectors_found": [],
                    "selectors_probed": COMMON_SELECTORS.len(),
                    "failed_probes": failed_probes,
                    "spf_posture": self.spf_posture_label,
                })),
                confidence: IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "No DKIM selectors found among common names".into(),
            description: format!(
                "None of {} common DKIM selector names (default, google, selector1, k1, ...) has a key record under _domainkey.{}. If this domain sends mail, DKIM signing may not be set up - or it may use a custom selector this probe cannot see.",
                COMMON_SELECTORS.len(),
                domain
            ),
            status: CheckStatus::Warn,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: Some(format!(
                "Enable DKIM signing wherever this domain's mail is sent from (Google Workspace: Admin console > Apps > Gmail > Authenticate email; Microsoft 365: Defender portal > Email authentication settings; transactional senders like Postmark or SendGrid show the exact DNS records in their dashboard). Publish the CNAME or TXT record the provider gives you at its selector name under _domainkey.{}.",
                domain
            )),
            raw_data: Some(serde_json::json!({
                "domain": domain,
                "probed": true,
                "selectors_found": [],
                "selectors_probed": COMMON_SELECTORS.len(),
                "failed_probes": failed_probes,
            })),
            confidence: IssueConfidence::NeedsReview,
            confidence_reason: Some(format!(
                "Selector names are chosen by the mail provider and this probe only guesses {} common ones; a custom selector is invisible to it, so absence here is not proof that DKIM is missing.",
                COMMON_SELECTORS.len()
            )),
            why_it_matters: Some("Without a verifiable DKIM signature, receivers lean on SPF alone, which breaks on forwarding; DKIM plus SPF gives DMARC something to align on.".into()),
        }]
    }
}

#[cfg(test)]
#[path = "dkim_tests.rs"]
mod tests;
