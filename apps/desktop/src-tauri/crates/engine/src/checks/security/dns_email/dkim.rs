//! Grades common DKIM selector records after MX and SPF establish mail posture.
//!
//! Published keys do not prove message signing, validation, or DMARC alignment.

use super::{has_mx_from, skipped_dns_failure, spf, spf_posture_from, SpfPosture};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::dns::{DnsOutcome, MxRecord};

pub const CHECK_ID: &str = "security.dns.dkim";
pub const TITLE: &str = "DKIM selectors";

/// Common DKIM selector names, covering the default names used by major mail
/// providers (Google Workspace, Microsoft 365, Proton Mail's protonmail,
/// protonmail2, protonmail3 CNAMEs, Fastmail's fm1-fm3, iCloud's sig1,
/// Mailchimp/Mandrill, Postmark, Zoho, and generic k1/s1-style ESP defaults).
pub const COMMON_SELECTORS: [&str; 22] = [
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
    "fm2",
    "fm3",
    "protonmail",
    "protonmail2",
    "protonmail3",
    "sig1",
];

/// A mail provider a domain's SPF record can name, the `include:` hosts that
/// identify it, and the DKIM selector names it publishes by default. An SPF
/// include is the domain owner stating which provider sends their mail, so
/// that provider's documented selectors are worth probing even when they are
/// not general enough for [`COMMON_SELECTORS`].
pub struct SpfMailProvider {
    /// Provider name for the verdict copy.
    pub label: &'static str,
    /// `include:` hosts that identify the provider. Matched exactly or as a
    /// parent of the include host.
    pub includes: &'static [&'static str],
    /// Selector names the provider publishes by default.
    pub selectors: &'static [&'static str],
}

/// Providers whose default selector names can be derived from an SPF include.
pub const SPF_MAIL_PROVIDERS: &[SpfMailProvider] = &[
    SpfMailProvider {
        label: "Google Workspace",
        includes: &["_spf.google.com"],
        selectors: &["google"],
    },
    SpfMailProvider {
        label: "Microsoft 365",
        includes: &["spf.protection.outlook.com"],
        selectors: &["selector1", "selector2"],
    },
    SpfMailProvider {
        label: "Proton Mail",
        includes: &["_spf.protonmail.ch"],
        selectors: &["protonmail", "protonmail2", "protonmail3"],
    },
    SpfMailProvider {
        label: "Fastmail",
        includes: &["spf.messagingengine.com"],
        selectors: &["fm1", "fm2", "fm3"],
    },
    SpfMailProvider {
        label: "iCloud Mail",
        includes: &["_spf.icloud.com"],
        selectors: &["sig1"],
    },
    SpfMailProvider {
        label: "Zoho Mail",
        includes: &["zoho.com", "zoho.eu", "zohomail.com"],
        selectors: &["zoho"],
    },
    SpfMailProvider {
        label: "Mailchimp",
        includes: &["servers.mcsv.net"],
        selectors: &["k1", "k2", "k3"],
    },
    SpfMailProvider {
        label: "Mandrill",
        includes: &["spf.mandrillapp.com"],
        selectors: &["mandrill"],
    },
    SpfMailProvider {
        label: "Postmark",
        includes: &["spf.mtasv.net"],
        selectors: &["pm"],
    },
    SpfMailProvider {
        label: "SendGrid",
        includes: &["sendgrid.net"],
        selectors: &["s1", "s2"],
    },
    SpfMailProvider {
        label: "Mailjet",
        includes: &["spf.mailjet.com"],
        selectors: &["mailjet"],
    },
    SpfMailProvider {
        label: "Brevo",
        includes: &["spf.brevo.com", "spf.sendinblue.com"],
        selectors: &["mail"],
    },
    SpfMailProvider {
        label: "Titan",
        includes: &["spf.titan.email"],
        selectors: &["titan1", "titan2"],
    },
    SpfMailProvider {
        label: "Hostinger",
        includes: &["_spf.mail.hostinger.com"],
        selectors: &["hostingermail1", "hostingermail2", "hostingermail3"],
    },
];

/// The `include:` hosts named by a domain's SPF records, lowercased.
fn spf_include_hosts(records: &[String]) -> Vec<String> {
    let mut hosts = Vec::new();
    for record in records.iter().filter(|record| spf::is_spf_record(record)) {
        for term in record.split_whitespace() {
            let term = term.to_ascii_lowercase();
            // SPF qualifiers (+ - ~ ?) precede the mechanism name.
            let term = term.trim_start_matches(['+', '-', '~', '?']);
            let Some(host) = term.strip_prefix("include:") else {
                continue;
            };
            let host = host.trim_end_matches('.').to_string();
            if !host.is_empty() && !hosts.contains(&host) {
                hosts.push(host);
            }
        }
    }
    hosts
}

/// The providers a domain's SPF records name through `include:` mechanisms.
pub fn spf_named_providers(records: &[String]) -> Vec<&'static SpfMailProvider> {
    let hosts = spf_include_hosts(records);
    SPF_MAIL_PROVIDERS
        .iter()
        .filter(|provider| {
            provider.includes.iter().any(|include| {
                hosts
                    .iter()
                    .any(|host| host == include || host.ends_with(&format!(".{include}")))
            })
        })
        .collect()
}

/// Every selector any entry in [`SPF_MAIL_PROVIDERS`] can contribute beyond
/// [`COMMON_SELECTORS`], in table order.
///
/// A runtime that has to author its DNS questions before it can read the
/// domain's SPF record gathers this union, so that whichever subset
/// [`selectors_from_spf`] later asks for has an answer waiting. The desktop
/// resolver reads the apex TXT first and asks for the derived subset directly;
/// both runtimes then grade the same names, which is the portable-engine
/// contract. The union is small because most providers publish selectors that
/// are already common names.
pub fn all_provider_selectors() -> Vec<&'static str> {
    let mut selectors: Vec<&'static str> = Vec::new();
    for provider in SPF_MAIL_PROVIDERS {
        for selector in provider.selectors {
            if !COMMON_SELECTORS.contains(selector) && !selectors.contains(selector) {
                selectors.push(selector);
            }
        }
    }
    selectors
}

/// What a domain's SPF record adds to the selector sweep.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SpfDerivedSweep {
    /// Selector names beyond [`COMMON_SELECTORS`], in provider-table order.
    /// Always a subset of [`all_provider_selectors`].
    pub selectors: Vec<&'static str>,
    /// Labels of the providers that contributed at least one of those names.
    /// A provider whose defaults are already common names is named by the SPF
    /// record but adds nothing to the probe list, so it must not appear in
    /// copy about what the sweep covered.
    pub providers: Vec<&'static str>,
}

/// The selectors and providers the domain's own SPF record adds to the sweep.
pub fn selectors_from_spf(records: &[String]) -> SpfDerivedSweep {
    let mut sweep = SpfDerivedSweep::default();
    for provider in spf_named_providers(records) {
        let before = sweep.selectors.len();
        for selector in provider.selectors {
            if !COMMON_SELECTORS.contains(selector) && !sweep.selectors.contains(selector) {
                sweep.selectors.push(selector);
            }
        }
        if sweep.selectors.len() > before {
            sweep.providers.push(provider.label);
        }
    }
    sweep
}

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
    /// Domain authoritatively has neither MX nor SPF: skip the whole
    /// selector sweep as pointless and report not-applicable. The copy derives
    /// the number of skipped lookups from `COMMON_SELECTORS`.
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
    /// Selectors beyond `COMMON_SELECTORS` that the domain's own SPF record
    /// points at.
    derived_selectors: Vec<&'static str>,
    /// The providers those selectors came from: a subset of `spf_providers`,
    /// because a provider whose defaults are already common names adds none.
    derived_from_providers: Vec<&'static str>,
    /// Every provider the SPF record names, contributing or not.
    spf_providers: Vec<&'static str>,
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
        // Nothing to grade: DKIM signs outbound mail, and a domain that shows
        // no mail setup gives the selector sweep no verdict to establish.
        SweepDecision::NotApplicable => DkimStep::Done(vec![CheckResult {
            check_id: CHECK_ID.into(),
            category: ScanCategory::Security,
            title: "DKIM probe not applicable".into(),
            description: format!(
                "{} {} and {}, so it shows no mail setup. DKIM signs outbound mail, so probing {} common selector names would establish nothing; this check was skipped rather than graded.",
                domain,
                if has_mx == Some(false) {
                    "publishes no MX records"
                } else {
                    "did not answer the MX lookup"
                },
                if label == Some("absent") {
                    "has no SPF record"
                } else {
                    "did not answer the TXT lookup for SPF"
                },
                COMMON_SELECTORS.len()
            ),
            status: CheckStatus::Skipped,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "reason": "no_mail_setup",
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
        SweepDecision::Sweep => {
            let apex_records: &[String] = match apex_txt {
                DnsOutcome::Records(records) => records,
                _ => &[],
            };
            let derived = selectors_from_spf(apex_records);
            DkimStep::Sweep(DkimSweep {
                domain: domain.to_string(),
                spf_is_null,
                spf_posture_label: label,
                derived_selectors: derived.selectors,
                derived_from_providers: derived.providers,
                spf_providers: spf_named_providers(apex_records)
                    .into_iter()
                    .map(|provider| provider.label)
                    .collect(),
            })
        }
    }
}

impl DkimSweep {
    /// The (selector, lookup name) pairs the runtime must answer: the common
    /// defaults first, in the vendored order, then any selector the domain's
    /// own SPF record points at that the common list does not already cover.
    pub fn probe_names(&self) -> Vec<(&'static str, String)> {
        COMMON_SELECTORS
            .iter()
            .copied()
            .chain(self.derived_selectors.iter().copied())
            .map(|selector| (selector, selector_lookup_name(selector, &self.domain)))
            .collect()
    }

    /// The providers this domain's SPF record names, for the verdict copy.
    pub fn spf_providers(&self) -> &[&'static str] {
        &self.spf_providers
    }

    /// Grade the per-selector TXT answers, one per entry of
    /// [`DkimSweep::probe_names`] in the same order.
    pub fn evaluate(self, outcomes: &[(String, DnsOutcome<Vec<String>>)]) -> Vec<CheckResult> {
        let domain = &self.domain;
        // Only say the SPF record changed the probe list when it did: a
        // Google Workspace include names a provider whose selectors are
        // already common names, so nothing was added.
        let provider_note = if self.derived_selectors.is_empty() {
            String::new()
        } else {
            format!(
                " The sweep also probed the {} default selector name{} published by {}, which this domain's SPF record names through include: mechanisms.",
                self.derived_selectors.len(),
                if self.derived_selectors.len() == 1 {
                    ""
                } else {
                    "s"
                },
                self.derived_from_providers.join(", ")
            )
        };
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
                    "selectors_probed": outcomes.len(),
                    "selectors_answered": outcomes.len() - failed_probes,
                    "spf_derived_selectors": self.derived_selectors,
                    "spf_providers": self.spf_providers,
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
                    "selectors_probed": outcomes.len(),
                    "selectors_answered": outcomes.len() - failed_probes,
                    "spf_derived_selectors": self.derived_selectors,
                    "spf_providers": self.spf_providers,
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
                    "None of the {} DKIM selector names answered under _domainkey.{} has a key record. This domain's SPF record is the null record (v=spf1 -all), declaring it sends no mail, and DKIM signs outbound mail - so missing selectors are consistent with that posture.",
                    outcomes.len() - failed_probes,
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
                    "selectors_probed": outcomes.len(),
                    "selectors_answered": outcomes.len() - failed_probes,
                    "spf_derived_selectors": self.derived_selectors,
                    "spf_providers": self.spf_providers,
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
            title: "No DKIM selectors found among the probed names".into(),
            description: format!(
                "None of the {} DKIM selector names answered under _domainkey.{} has a key record.{}{} If this domain sends mail, DKIM signing may not be set up - or it may use a custom selector this probe cannot see.",
                outcomes.len() - failed_probes,
                domain,
                provider_note,
                if failed_probes > 0 {
                    format!(
                        " {} further selector lookup{} did not answer and {} not evaluated.",
                        failed_probes,
                        if failed_probes == 1 { "" } else { "s" },
                        if failed_probes == 1 { "was" } else { "were" }
                    )
                } else {
                    String::new()
                }
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
                "selectors_probed": outcomes.len(),
                "selectors_answered": outcomes.len() - failed_probes,
                "spf_derived_selectors": self.derived_selectors,
                "spf_providers": self.spf_providers,
                "failed_probes": failed_probes,
            })),
            confidence: IssueConfidence::NeedsReview,
            confidence_reason: Some(format!(
                "Selector names are chosen by the mail provider and this probe only tries {} known names; a custom selector is invisible to it, so absence here is not proof that DKIM is missing.",
                outcomes.len()
            )),
            why_it_matters: Some("Without a verifiable DKIM signature, receivers lean on SPF alone, which breaks on forwarding; DKIM plus SPF gives DMARC something to align on.".into()),
        }]
    }
}

#[cfg(test)]
#[path = "dkim_tests.rs"]
mod tests;
