//! Detects a `www` CNAME whose target no longer resolves.
//!
//! DNS failure establishes an outage signal, not provider-side takeover ability.

use super::skipped_dns_failure;
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::dns::DnsOutcome;

pub const CHECK_ID: &str = "security.dns.dangling_cname";
pub const TITLE: &str = "Dangling CNAME (www)";

/// The www sibling whose alias this check inspects.
pub fn www_lookup_name(domain: &str) -> String {
    format!("www.{}", domain)
}

/// What the www alias lookup concluded.
pub enum WwwAliasPosture {
    /// No CNAME record at www: it either resolves directly (A/AAAA) or is not
    /// configured at all. Neither is a takeover risk.
    NoCname,
    /// www is an alias and the target resolves to at least one address.
    Resolves { target: String },
    /// www is an alias but the target authoritatively does not resolve.
    Dangling { target: String },
}

pub fn posture_result(check_id: &str, www_host: &str, posture: &WwwAliasPosture) -> CheckResult {
    let (title, description, status, severity, manual_fix, why, raw) = match posture {
        WwwAliasPosture::NoCname => (
            TITLE.to_string(),
            format!(
                "{} publishes no CNAME record, so there is no alias target that could dangle. It either resolves directly or is not configured - neither is a takeover risk.",
                www_host
            ),
            CheckStatus::Pass,
            Severity::Low,
            None,
            None,
            serde_json::json!({"www_host": www_host, "cname_target": null}),
        ),
        WwwAliasPosture::Resolves { target } => (
            TITLE.to_string(),
            format!(
                "{} is a CNAME alias for {}, and the target resolves normally.",
                www_host, target
            ),
            CheckStatus::Pass,
            Severity::Low,
            None,
            None,
            serde_json::json!({"www_host": www_host, "cname_target": target, "target_resolves": true}),
        ),
        WwwAliasPosture::Dangling { target } => (
            format!("www CNAME target does not resolve: {}", www_host),
            format!(
                "{} is a CNAME alias for {}, but the target returned no A or AAAA address. Requests to {} cannot reach an origin through this alias. A subdomain-takeover risk exists only if the target belongs to a shared service that lets another account claim this exact identifier; this DNS probe does not test provider-side claimability.",
                www_host, target, www_host
            ),
            CheckStatus::Fail,
            Severity::Medium,
            Some(format!(
                "Either delete the {} CNAME record at your DNS host if www is no longer used, or point it back at a live target. If the target was a hosting-service name (a retired *.netlify.app, *.github.io, or storage-bucket hostname), remove the DNS record first and only restore it after the upstream resource exists again.",
                www_host
            )),
            Some(
                "The unresolved target is direct evidence that the www alias is broken. It does not establish that another party can claim the target; provider ownership and claim behavior must be verified before calling this a takeover exposure."
                    .to_string(),
            ),
            serde_json::json!({"www_host": www_host, "cname_target": target, "target_resolves": false}),
        ),
    };

    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Security,
        title,
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix,
        raw_data: Some(raw),
        confidence: IssueConfidence::High,
        confidence_reason: matches!(posture, WwwAliasPosture::Dangling { .. }).then(|| {
            "The DNS lookup directly observed a CNAME whose target returned no address records. Upstream service claimability was not tested.".into()
        }),
        why_it_matters: why,
    }
}

/// What the verdict needs next after the www CNAME answer.
pub enum WwwAliasStep {
    Done(Vec<CheckResult>),
    /// www is an alias: the runtime must resolve the target's addresses.
    LookupTarget(WwwTargetProbe),
}

/// The pending target resolution, waiting on the A/AAAA answer.
pub struct WwwTargetProbe {
    www_host: String,
    target: String,
}

impl WwwTargetProbe {
    /// The alias target whose addresses decide the verdict.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Grade the target's address answer, delivered as display-form
    /// addresses (only presence matters to the verdict).
    pub fn evaluate(self, addresses: DnsOutcome<Vec<String>>) -> Vec<CheckResult> {
        let posture = match addresses {
            DnsOutcome::Failed(detail) => {
                return vec![skipped_dns_failure(CHECK_ID, TITLE, &self.target, &detail)]
            }
            DnsOutcome::NoRecords => WwwAliasPosture::Dangling {
                target: self.target,
            },
            DnsOutcome::Records(_) => WwwAliasPosture::Resolves {
                target: self.target,
            },
        };
        vec![posture_result(CHECK_ID, &self.www_host, &posture)]
    }
}

/// Grade the www CNAME answer, delivered as the chain's target names in
/// answer order (the first one is the alias target).
pub fn evaluate_www_cname(domain: &str, cname: DnsOutcome<Vec<String>>) -> WwwAliasStep {
    let www_host = www_lookup_name(domain);
    match cname {
        DnsOutcome::Failed(detail) => WwwAliasStep::Done(vec![skipped_dns_failure(
            CHECK_ID, TITLE, &www_host, &detail,
        )]),
        DnsOutcome::NoRecords => WwwAliasStep::Done(vec![posture_result(
            CHECK_ID,
            &www_host,
            &WwwAliasPosture::NoCname,
        )]),
        DnsOutcome::Records(targets) => match targets.into_iter().next() {
            Some(target) => WwwAliasStep::LookupTarget(WwwTargetProbe { www_host, target }),
            None => WwwAliasStep::Done(vec![posture_result(
                CHECK_ID,
                &www_host,
                &WwwAliasPosture::NoCname,
            )]),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "security.dns.dangling_cname";

    #[test]
    fn no_cname_passes_without_fix_guidance() {
        let result = posture_result(ID, "www.example.com", &WwwAliasPosture::NoCname);
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.severity, Severity::Low);
        assert!(result.manual_fix.is_none());
        assert!(result.description.contains("no CNAME record"));
    }

    #[test]
    fn resolving_alias_passes_and_names_the_target() {
        let posture = WwwAliasPosture::Resolves {
            target: "sites.example-host.net".into(),
        };
        let result = posture_result(ID, "www.example.com", &posture);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("sites.example-host.net"));
    }

    #[test]
    fn dangling_alias_reports_availability_without_asserting_takeover() {
        let posture = WwwAliasPosture::Dangling {
            target: "retired-site.example-host.net".into(),
        };
        let result = posture_result(ID, "www.example.com", &posture);
        assert_eq!(result.status, CheckStatus::Fail);
        assert_eq!(result.severity, Severity::Medium);
        assert!(result.manual_fix.is_some());
        assert!(result
            .why_it_matters
            .as_deref()
            .is_some_and(|why| why.contains("does not establish")));
        assert!(!result.title.to_lowercase().contains("takeover"));
        assert!(!result
            .why_it_matters
            .as_deref()
            .unwrap_or_default()
            .contains("means whoever registers"));
        assert!(result.description.contains("retired-site.example-host.net"));
    }

    #[test]
    fn no_cname_answer_completes_without_the_target_question() {
        let step = evaluate_www_cname("example.com", DnsOutcome::NoRecords);
        let WwwAliasStep::Done(results) = step else {
            panic!("no CNAME must not resolve a target");
        };
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn an_alias_asks_for_its_targets_addresses() {
        let step = evaluate_www_cname(
            "example.com",
            DnsOutcome::Records(vec!["sites.example-host.net".into()]),
        );
        let WwwAliasStep::LookupTarget(probe) = step else {
            panic!("an alias needs the target resolved");
        };
        assert_eq!(probe.target(), "sites.example-host.net");

        let resolved = probe.evaluate(DnsOutcome::Records(vec!["192.0.2.10".into()]));
        assert_eq!(resolved[0].status, CheckStatus::Pass);
    }

    #[test]
    fn an_unresolving_target_is_the_dangling_failure() {
        let step = evaluate_www_cname(
            "example.com",
            DnsOutcome::Records(vec!["retired.example-host.net".into()]),
        );
        let WwwAliasStep::LookupTarget(probe) = step else {
            panic!("an alias needs the target resolved");
        };
        let results = probe.evaluate(DnsOutcome::NoRecords);
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert!(results[0].description.contains("retired.example-host.net"));
    }

    #[test]
    fn failed_lookups_skip_at_both_steps() {
        let first = evaluate_www_cname("example.com", DnsOutcome::Failed("timed out".into()));
        let WwwAliasStep::Done(results) = first else {
            panic!("a failed CNAME lookup completes as Skipped");
        };
        assert_eq!(results[0].status, CheckStatus::Skipped);

        let step = evaluate_www_cname(
            "example.com",
            DnsOutcome::Records(vec!["sites.example-host.net".into()]),
        );
        let WwwAliasStep::LookupTarget(probe) = step else {
            panic!("an alias needs the target resolved");
        };
        let results = probe.evaluate(DnsOutcome::Failed("timed out".into()));
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }
}
