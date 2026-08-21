//! Plan and grade bounded open-redirect probes using a reserved canary domain.
//! Pass results state the probe set's limits.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::{BodyPolicy, ProbeOutcome, ProbeRequest, RedirectPolicy};

/// Common redirect parameters to test
pub const REDIRECT_PARAMS: &[&str] = &[
    "redirect",
    "url",
    "next",
    "return",
    "returnTo",
    "return_url",
    "redirect_url",
    "redirect_uri",
    "continue",
    "dest",
    "destination",
    "go",
    "target",
    "rurl",
];

/// Auth-flow paths where redirect parameters commonly appear.
pub const PROBE_PATHS: &[&str] = &[
    "/",
    "/auth/callback",
    "/login",
    "/signin",
    "/logout",
    "/oauth/callback",
    "/account/redirect",
];

/// The canary domain we redirect to for testing
pub const CANARY_DOMAIN: &str = "https://evil.example.com";

/// The canary host a redirect target must resolve to before it counts.
pub const CANARY_HOST: &str = "evil.example.com";

/// One planned probe: the URL to request, and the `path?param` label that
/// identifies it in evidence.
#[derive(Debug, Clone)]
pub struct OpenRedirectProbe {
    pub url: String,
    pub label: String,
}

impl OpenRedirectProbe {
    /// The probe request: no-follow (the Location header IS the evidence,
    /// following it would hide the redirect) and no body.
    pub fn request(&self) -> ProbeRequest {
        ProbeRequest::get(&self.url)
            .body(BodyPolicy::None)
            .redirects(RedirectPolicy::None)
    }
}

/// Every parameter/path probe for one scanned origin.
pub fn open_redirect_probes(origin: &str) -> Vec<OpenRedirectProbe> {
    let mut probes = Vec::with_capacity(PROBE_PATHS.len() * REDIRECT_PARAMS.len());
    for path in PROBE_PATHS {
        for param in REDIRECT_PARAMS {
            probes.push(OpenRedirectProbe {
                url: format!("{}{}?{}={}", origin, path, param, CANARY_DOMAIN),
                label: format!("{}?{}", path, param),
            });
        }
    }
    probes
}

/// The scanned URL's origin without userinfo, path, or query - the base
/// every probe URL is built from.
pub fn probe_origin(page_url: &url::Url) -> String {
    format!(
        "{}://{}{}",
        page_url.scheme(),
        page_url.host_str().unwrap_or("localhost"),
        page_url
            .port()
            .map(|port| format!(":{}", port))
            .unwrap_or_default()
    )
}

/// Classify one probe outcome: `Some(label)` only when the response is a
/// redirect whose resolved destination host is the canary. A failed request
/// is not evidence of anything.
pub fn observe_open_redirect_probe(
    probe: &OpenRedirectProbe,
    outcome: &ProbeOutcome,
) -> Option<String> {
    let ProbeOutcome::Response(response) = outcome else {
        return None;
    };
    if !(300..400).contains(&response.status) {
        return None;
    }
    let location = response.header("location")?;
    redirects_to_canary(location, &probe.url).then(|| probe.label.clone())
}

/// Return whether the resolved redirect target is the canary host.
/// Canary text in a same-origin query string does not count.
fn redirects_to_canary(location: &str, probe_url: &str) -> bool {
    let Ok(base) = url::Url::parse(probe_url) else {
        return false;
    };
    let Ok(target) = base.join(location.trim()) else {
        return false;
    };
    matches!(
        target.host_str(),
        Some(host) if host == CANARY_HOST || host.ends_with(&format!(".{CANARY_HOST}"))
    )
}

/// Tracks planned and answered probes separately from vulnerable parameters.
/// A clean verdict requires coverage, not only an empty finding list.
#[derive(Debug, Clone, Default)]
pub struct OpenRedirectSweep {
    planned: usize,
    answered: usize,
    vulnerable_labels: Vec<String>,
}

impl OpenRedirectSweep {
    /// Fold one planned probe's outcome in. Every planned probe is observed,
    /// the failed ones included: a probe left out here is a probe the
    /// coverage claim quietly forgets it asked for.
    pub fn observe(&mut self, probe: &OpenRedirectProbe, outcome: &ProbeOutcome) {
        self.planned += 1;
        // Any response is an answer, a 404 included. What a coverage claim
        // needs to know is whether the server spoke, not whether it approved
        // of the request.
        if matches!(outcome, ProbeOutcome::Response(_)) {
            self.answered += 1;
        }
        if let Some(label) = observe_open_redirect_probe(probe, outcome) {
            self.vulnerable_labels.push(label);
        }
    }
}

/// Require a majority of probes for a clean verdict; one observed redirect is
/// sufficient evidence for a failing verdict.
fn sweep_is_representative(sweep: &OpenRedirectSweep) -> bool {
    sweep.answered * 2 > sweep.planned
}

/// Grade the collected probe observations.
pub fn evaluate_open_redirect(sweep: OpenRedirectSweep) -> Vec<CheckResult> {
    if sweep.vulnerable_labels.is_empty() {
        return match sweep_is_representative(&sweep) {
            true => vec![clean_sweep_result(&sweep)],
            false => vec![unswept_result(&sweep)],
        };
    }

    let vulnerable_params = sweep.vulnerable_labels;
    vec![CheckResult {
        check_id: "security.open_redirect".into(),
        category: ScanCategory::Security,
        title: "External canary redirect observed".into(),
        description: format!(
            "The server returned a redirect whose resolved destination host was the reserved external canary for the following tested path/parameter label{}: {}. This is direct evidence that those unauthenticated probes accepted an off-site destination; authenticated and application-specific paths were not evaluated. If the behavior is reachable in a trusted user flow, it can be abused for phishing or redirect-chain confusion.",
            if vulnerable_params.len() == 1 { "" } else { "s" },
            vulnerable_params.join(", ")
        ),
        status: CheckStatus::Fail,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: Some(
            "Prefer a server-owned route key mapped to an internal relative path. If full URLs are required, parse and normalize once with a standards-compliant URL library and allow only the exact intended scheme, host, port, and path policy; reject credentials, protocol-relative forms, control characters, encoded parser-confusion cases, and non-web schemes. Revalidate every redirect hop generated from untrusted state.".into(),
        ),
        raw_data: Some(serde_json::json!({
            "vulnerable_params": vulnerable_params,
            "canary_host": CANARY_HOST,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: Some("A trusted application URL that accepts an attacker-selected external destination can lend credibility to phishing and confuse login, account-linking, or payment return flows.".into()),
    }]
}

/// Build a clean result from observed rather than planned probe counts.
fn clean_sweep_result(sweep: &OpenRedirectSweep) -> CheckResult {
    let unanswered = sweep.planned - sweep.answered;
    let tested = if unanswered == 0 {
        format!(
            "Tested {} common redirect parameter{} against {} common path{} with a reserved external canary host. None of those probes returned a redirect to the canary.",
            REDIRECT_PARAMS.len(),
            if REDIRECT_PARAMS.len() == 1 { "" } else { "s" },
            PROBE_PATHS.len(),
            if PROBE_PATHS.len() == 1 { "" } else { "s" },
        )
    } else {
        format!(
            "Tested {} of the {} planned probes ({} common redirect parameters against {} common paths) with a reserved external canary host; the remaining {} did not complete and are not covered by this result. None of the probes that answered returned a redirect to the canary.",
            sweep.answered, sweep.planned, REDIRECT_PARAMS.len(), PROBE_PATHS.len(), unanswered,
        )
    };
    CheckResult {
        check_id: "security.open_redirect".into(),
        category: ScanCategory::Security,
        title: "No tested open redirect observed".into(),
        description: format!("{tested} This limited pass does not cover application-specific parameter names, request bodies, authenticated flows, multi-step redirects, client-side navigation, or routes outside the tested set."),
        status: CheckStatus::Pass,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "parameter_count": REDIRECT_PARAMS.len(),
            "path_count": PROBE_PATHS.len(),
            "probes_planned": sweep.planned,
            "probes_answered": sweep.answered,
            "canary_host": CANARY_HOST,
        })),
        confidence: if unanswered == 0 {
            IssueConfidence::High
        } else {
            IssueConfidence::NeedsReview
        },
        confidence_reason: (unanswered > 0).then(|| format!(
            "{unanswered} of the {} planned probes did not complete, so the parameter sweep behind this result is partial and a redirect on one of those path/parameter pairs would not have been seen.",
            sweep.planned,
        )),
        why_it_matters: None,
    }
}

/// Return Skipped when too little of the sweep answered to support a verdict.
fn unswept_result(sweep: &OpenRedirectSweep) -> CheckResult {
    let description = if sweep.answered == 0 {
        format!(
            "None of the {} planned open-redirect probes returned a response, so the parameter sweep was not performed and this scan produced no open-redirect verdict.",
            sweep.planned,
        )
    } else {
        format!(
            "Only {} of the {} planned open-redirect probes returned a response. That is too little of the parameter sweep to report a result in either direction, so this scan produced no open-redirect verdict.",
            sweep.answered, sweep.planned,
        )
    };
    CheckResult {
        check_id: "security.open_redirect".into(),
        category: ScanCategory::Security,
        title: "Open redirect probes did not complete".into(),
        description,
        status: CheckStatus::Skipped,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "probes_planned": sweep.planned,
            "probes_answered": sweep.answered,
            "canary_host": CANARY_HOST,
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "The probes did not reach the site, so this result establishes nothing about open-redirect behavior in either direction. Re-scan once the origin is reachable.".into(),
        ),
        why_it_matters: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ProbeFailure, ProbeFailureClass, ProbeResponse};

    const PROBE: &str = "https://site.example/login?next=https://evil.example.com";

    fn probe() -> OpenRedirectProbe {
        OpenRedirectProbe {
            url: PROBE.to_string(),
            label: "/login?next".to_string(),
        }
    }

    fn redirect_to(location: Option<&str>, status: u16) -> ProbeOutcome {
        ProbeOutcome::Response(ProbeResponse {
            status,
            final_url: String::new(),
            content_type: None,
            content_length: None,
            headers: location
                .map(|value| vec![("location".to_string(), value.to_string())])
                .unwrap_or_default(),
            body: None,
        })
    }

    #[test]
    fn absolute_canary_target_is_flagged() {
        assert!(redirects_to_canary("https://evil.example.com/", PROBE));
        assert!(redirects_to_canary("https://sub.evil.example.com/x", PROBE));
    }

    #[test]
    fn protocol_relative_canary_target_is_flagged() {
        assert!(redirects_to_canary("//evil.example.com/phish", PROBE));
    }

    #[test]
    fn same_origin_location_echoing_canary_in_query_is_safe() {
        assert!(!redirects_to_canary(
            "/login?next=https://evil.example.com",
            PROBE
        ));
        assert!(!redirects_to_canary(
            "https://site.example/verify?goto=https%3A%2F%2Fevil.example.com",
            PROBE
        ));
    }

    #[test]
    fn ordinary_same_origin_redirect_is_safe() {
        assert!(!redirects_to_canary("/dashboard", PROBE));
        assert!(!redirects_to_canary("https://site.example/home", PROBE));
    }

    #[test]
    fn lookalike_host_is_not_the_canary() {
        assert!(!redirects_to_canary(
            "https://evil.example.com.attacker.example/",
            PROBE
        ));
    }

    #[test]
    fn only_a_redirect_status_with_a_canary_location_counts() {
        assert_eq!(
            observe_open_redirect_probe(&probe(), &redirect_to(Some(CANARY_DOMAIN), 302)),
            Some("/login?next".to_string())
        );
        // A 200 that merely echoes the canary in its body/headers is not a
        // redirect; a redirect without a Location resolves to nothing.
        assert!(
            observe_open_redirect_probe(&probe(), &redirect_to(Some(CANARY_DOMAIN), 200)).is_none()
        );
        assert!(observe_open_redirect_probe(&probe(), &redirect_to(None, 302)).is_none());
        // A failed request is not evidence either way.
        assert!(observe_open_redirect_probe(
            &probe(),
            &ProbeOutcome::Failure(ProbeFailure {
                class: ProbeFailureClass::Transport,
                detail: "connection refused".into(),
            })
        )
        .is_none());
    }

    #[test]
    fn the_probe_plan_covers_every_path_and_parameter_pair() {
        let probes = open_redirect_probes("https://site.example");
        assert_eq!(probes.len(), PROBE_PATHS.len() * REDIRECT_PARAMS.len());
        assert!(probes
            .iter()
            .any(|probe| probe.label == "/oauth/callback?redirect_uri"));
        let first = &probes[0];
        assert!(first.url.starts_with("https://site.example/?redirect="));
        assert_eq!(first.request().redirects, RedirectPolicy::None);
        assert_eq!(first.request().body, BodyPolicy::None);
    }

    #[test]
    fn probe_origin_drops_credentials_path_and_query_but_keeps_the_port() {
        let url = url::Url::parse("https://user:pw@site.example:8443/deep/page?token=secret")
            .expect("test URL");
        assert_eq!(probe_origin(&url), "https://site.example:8443");
    }

    /// Fold the whole plan, answering `failures` of the probes with a
    /// transport failure and the rest with a benign same-origin redirect.
    fn sweep_with_failures(failures: usize) -> OpenRedirectSweep {
        let benign = redirect_to(Some("/dashboard"), 302);
        let failed = ProbeOutcome::Failure(ProbeFailure {
            class: ProbeFailureClass::Transport,
            detail: "connection refused".into(),
        });
        let mut sweep = OpenRedirectSweep::default();
        for (index, probe) in open_redirect_probes("https://site.example")
            .iter()
            .enumerate()
        {
            sweep.observe(probe, if index < failures { &failed } else { &benign });
        }
        sweep
    }

    #[test]
    fn a_clean_sweep_passes_with_bounded_coverage_copy() {
        let results = evaluate_open_redirect(sweep_with_failures(0));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].confidence, IssueConfidence::High);
        assert!(results[0].description.contains("does not cover"));
        // The full sweep may state the table sizes, because it ran them all.
        assert!(results[0].description.contains("Tested 14 common redirect"));
    }

    #[test]
    fn an_observed_canary_redirect_fails_naming_its_labels() {
        let mut sweep = sweep_with_failures(0);
        sweep.vulnerable_labels = vec!["/login?next".into()];
        let results = evaluate_open_redirect(sweep);
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::Medium);
        assert!(results[0].description.contains("/login?next"));
    }

    #[test]
    fn a_sweep_nothing_answered_declines_to_grade_rather_than_passing() {
        let planned = PROBE_PATHS.len() * REDIRECT_PARAMS.len();
        let results = evaluate_open_redirect(sweep_with_failures(planned));
        assert_eq!(results.len(), 1);
        assert_ne!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].title.contains("did not complete"));
        // And it never asserts a count it did not attempt.
        assert!(!results[0].description.contains("Tested"));
        assert_eq!(
            results[0].raw_data.as_ref().expect("raw data")["probes_answered"],
            0
        );
    }

    #[test]
    fn a_minority_sweep_declines_to_grade() {
        let planned = PROBE_PATHS.len() * REDIRECT_PARAMS.len();
        // One answer out of 98 is a sample, not a sweep: the finding would
        // most likely live in the 97 that never left the machine.
        let results = evaluate_open_redirect(sweep_with_failures(planned - 1));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("Only 1 of the 98"));
    }

    #[test]
    fn a_majority_sweep_still_grades_and_states_what_it_tested() {
        let planned = PROBE_PATHS.len() * REDIRECT_PARAMS.len();
        let results = evaluate_open_redirect(sweep_with_failures(8));
        assert_eq!(results[0].status, CheckStatus::Pass);
        // 90 real answers are real evidence, and the copy says 90 rather
        // than claiming the full grid.
        assert!(results[0]
            .description
            .contains("Tested 90 of the 98 planned probes"));
        assert!(!results[0].description.contains("Tested 14 common redirect"));
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        let raw = results[0].raw_data.as_ref().expect("raw data");
        assert_eq!(raw["probes_answered"], 90);
        assert_eq!(raw["probes_planned"], planned);
    }

    #[test]
    fn a_canary_redirect_fails_even_when_most_of_the_sweep_did_not_answer() {
        // Presence needs only valid execution: one observed off-site
        // redirect is direct evidence whatever else failed to answer.
        let mut sweep = sweep_with_failures(PROBE_PATHS.len() * REDIRECT_PARAMS.len() - 1);
        sweep.vulnerable_labels = vec!["/oauth/callback?redirect_uri".into()];
        assert_eq!(evaluate_open_redirect(sweep)[0].status, CheckStatus::Fail);
    }
}
