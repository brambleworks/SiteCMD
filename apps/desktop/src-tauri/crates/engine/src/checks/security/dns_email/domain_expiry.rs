//! Domain-expiry grading from RDAP. Registry failures skip rather than imply
//! domain failure.

use chrono::{DateTime, Utc};

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::{ProbeOutcome, ProbeRequest};

pub const CHECK_ID: &str = "security.domain_expiry";
pub const TITLE: &str = "Domain registration expiry";

/// The bootstrap URL whose redirect chain reaches the authoritative
/// registry's RDAP record for the domain.
pub fn rdap_url(domain: &str) -> String {
    format!("https://rdap.org/domain/{}", domain)
}

/// The planned RDAP probe: follow the bootstrap redirect and read the 2xx
/// body as required evidence.
pub fn rdap_probe(domain: &str) -> ProbeRequest {
    ProbeRequest::get(rdap_url(domain)).header("Accept", "application/rdap+json")
}

/// Extract the expiration timestamp from an RDAP domain response
/// (events[] entry with eventAction == "expiration").
pub fn parse_rdap_expiration(body: &serde_json::Value) -> Option<DateTime<Utc>> {
    body.get("events")?.as_array()?.iter().find_map(|event| {
        if event.get("eventAction")?.as_str()? != "expiration" {
            return None;
        }
        let date = event.get("eventDate")?.as_str()?;
        DateTime::parse_from_rfc3339(date)
            .ok()
            .map(|parsed| parsed.with_timezone(&Utc))
    })
}

pub struct ExpiryVerdict {
    pub status: CheckStatus,
    pub severity: Severity,
}

/// Treat observed registration windows as warnings, not confirmed outages.
pub fn classify_expiry(days_until: i64) -> ExpiryVerdict {
    if days_until <= 7 {
        ExpiryVerdict {
            status: CheckStatus::Warn,
            severity: Severity::High,
        }
    } else if days_until <= 30 {
        ExpiryVerdict {
            status: CheckStatus::Warn,
            severity: Severity::Medium,
        }
    } else if days_until <= 90 {
        ExpiryVerdict {
            status: CheckStatus::Warn,
            severity: Severity::Low,
        }
    } else {
        ExpiryVerdict {
            status: CheckStatus::Pass,
            severity: Severity::Low,
        }
    }
}

fn expiry_title(days_until: i64) -> String {
    match days_until {
        0 => "Domain registration expires today".into(),
        1 => "Domain registration expires in 1 day".into(),
        days => format!("Domain registration expires in {} days", days),
    }
}

fn expiry_window_phrase(days_until: i64) -> String {
    match days_until {
        0 => "today".into(),
        1 => "in 1 day".into(),
        days => format!("in {} days", days),
    }
}

fn skipped_rdap(domain: &str, detail: &str) -> CheckResult {
    let detail = crate::log_sanitizer::bounded_issue_evidence(detail);
    CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Security,
        title: TITLE.into(),
        description: format!(
            "RDAP data for {} was unavailable ({}). Some registries and country-code TLDs do not publish RDAP records; this check never fails on registry infrastructure problems.",
            domain, detail
        ),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(
            serde_json::json!({"reason": "rdap_unavailable", "domain": domain, "detail": detail}),
        ),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Grade the RDAP probe outcome under the injected evaluation time.
pub fn evaluate_rdap(
    domain: &str,
    outcome: &ProbeOutcome,
    evaluation_time: DateTime<Utc>,
) -> Vec<CheckResult> {
    let response = match outcome {
        ProbeOutcome::Failure(failure) => {
            return vec![skipped_rdap(
                domain,
                &format!("request failed: {}", failure.detail),
            )]
        }
        ProbeOutcome::Response(response) => response,
    };

    if !(200..300).contains(&response.status) {
        return vec![skipped_rdap(
            domain,
            &format!("registry answered HTTP {}", response.status),
        )];
    }

    let Some(body) = response.body.as_ref() else {
        return vec![skipped_rdap(domain, "response body unavailable")];
    };

    let json: serde_json::Value = match serde_json::from_str(&body.text) {
        Ok(json) => json,
        Err(_) => return vec![skipped_rdap(domain, "response was not valid JSON")],
    };

    let Some(expiry) = parse_rdap_expiration(&json) else {
        return vec![skipped_rdap(
            domain,
            "the registry did not publish an expiration event",
        )];
    };

    let days_until = (expiry - evaluation_time).num_days();
    let verdict = classify_expiry(days_until);
    let expiry_date = expiry.format("%Y-%m-%d").to_string();
    let raw_data = serde_json::json!({
        "domain": domain,
        "rdap_url": rdap_url(domain),
        "expiration_date": expiry.to_rfc3339(),
        "days_until_expiry": days_until,
    });

    let (title, description, manual_fix, why_it_matters) = if days_until < 0 {
        (
            "RDAP domain expiration date is in the past".to_string(),
            format!(
                "The registry RDAP record reports an expiration event for {} on {}, which is in the past. The site resolved for this scan, so this does not by itself prove that the registration is currently inactive: the record may be awaiting a renewal update or the domain may be in a registrar/registry grace state. Confirm the authoritative registrar status immediately.",
                domain, expiry_date
            ),
            Some("Open the authoritative registrar account now and confirm the domain's current registration and renewal status. If renewal is still available and intended, complete it through the registrar; then verify the RDAP record, nameserver delegation, website, and domain email after the registry updates. If the date is stale after a completed renewal, contact the registrar rather than repeatedly purchasing or transferring the domain.".into()),
            Some("A genuinely lapsed registration can lead to suspended delegation or service disruption and can eventually make the name available to someone else. The exact grace, redemption, and release lifecycle varies by registry and registrar; a past RDAP event alone does not establish the current lifecycle state.".into()),
        )
    } else if days_until <= 7 {
        (
            expiry_title(days_until),
            format!(
                "The registry RDAP record reports that {} expires on {}, {}. SiteCMD cannot see registrar auto-renew, account ownership, payment state, registry grace rules, or a renewal already in progress. Confirm the authoritative registrar status now and renew if needed.",
                domain, expiry_date, expiry_window_phrase(days_until)
            ),
            Some("Confirm the domain in the authoritative registrar account now. Verify auto-renew is enabled if intended, the account and payment path are usable, renewal notices reach an independently accessible address, and any pending renewal has completed. Renew through the registrar if the registration is not already secured, then confirm RDAP and DNS after propagation.".into()),
            Some("A registration that is not renewed can enter a registrar/registry lifecycle that may suspend delegation or disrupt the website and domain email and can eventually release the name. Timing and service behavior vary by TLD and registrar.".into()),
        )
    } else if days_until <= 30 {
        (
            expiry_title(days_until),
            format!(
                "The registry RDAP record reports that {} expires on {}, {}. This is a planning warning, not evidence that auto-renew is broken; SiteCMD cannot observe registrar settings, payment state, or a pending renewal.",
                domain, expiry_date, expiry_window_phrase(days_until)
            ),
            Some("Review the domain in the authoritative registrar account. Confirm ownership, renewal intent, auto-renew and payment state where applicable, and an independently accessible renewal contact. Renew according to the organization's policy before the reported date, then verify that RDAP reflects the updated term.".into()),
            Some("An unrenewed registration can eventually disrupt DNS-backed services and release the name, but this scan does not know whether a normal automatic or manual renewal is already scheduled.".into()),
        )
    } else if days_until <= 90 {
        (
            expiry_title(days_until),
            format!(
                "The registry RDAP record reports that {} expires on {}, {}. This early reminder provides time to confirm renewal ownership and policy; it does not indicate a current registration failure.",
                domain, expiry_date, expiry_window_phrase(days_until)
            ),
            Some("Confirm which team or owner is responsible for renewal, whether the authoritative registrar is configured as intended, and that renewal notices reach an independently accessible contact. Schedule renewal or a later verification before the date enters the short recovery window.".into()),
            Some("Registration ownership is a single operational dependency for the site's domain. An early reminder reduces the chance that an expired account, lost owner, or missed notice is discovered only near the deadline.".into()),
        )
    } else {
        (
            TITLE.to_string(),
            format!(
                "The registration for {} runs until {} ({} days from now), according to registry RDAP data.",
                domain, expiry_date, days_until
            ),
            None,
            None,
        )
    };

    vec![CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Security,
        title,
        description,
        status: verdict.status,
        severity: verdict.severity,
        fix_prompt: None,
        manual_fix,
        raw_data: Some(raw_data),
        confidence: if days_until < 0 {
            IssueConfidence::NeedsReview
        } else {
            IssueConfidence::High
        },
        confidence_reason: (days_until < 0).then(|| "The RDAP expiration event is directly observed, but the site still resolved and SiteCMD cannot see registrar renewal completion, publication lag, or the domain's current grace/redemption state.".into()),
        why_it_matters,
    }]
}

#[cfg(test)]
#[path = "domain_expiry_tests.rs"]
mod tests;
