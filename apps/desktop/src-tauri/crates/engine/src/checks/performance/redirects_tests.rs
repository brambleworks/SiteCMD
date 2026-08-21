use super::*;
use crate::probe::{ProbeFailure, ProbeFailureClass, ProbeResponse};

fn hop(from: &str, to: &str) -> RedirectHop {
    RedirectHop {
        from: from.into(),
        to: to.into(),
        status: 301,
    }
}

fn resolved(hops: Vec<RedirectHop>, final_url: &str) -> RedirectWalk {
    RedirectWalk {
        hops,
        termination: RedirectWalkTermination::FinalResponse {
            url: final_url.into(),
            status: 200,
        },
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

fn final_response(status: u16) -> ProbeOutcome {
    ProbeOutcome::Response(ProbeResponse {
        status,
        final_url: String::new(),
        content_type: None,
        content_length: None,
        headers: Vec::new(),
        body: None,
    })
}

// Drive a walker through recorded outcomes until it completes.
fn walk_through(start: &str, outcomes: &[ProbeOutcome]) -> RedirectWalk {
    let mut walker = RedirectWalker::new(&url::Url::parse(start).expect("static test url"));
    for outcome in outcomes {
        match walker.observe(outcome) {
            RedirectWalkStep::Continue(next) => walker = next,
            RedirectWalkStep::Done(walk) => return walk,
        }
    }
    panic!("walker did not terminate within the recorded outcomes");
}

#[test]
fn walker_records_hops_and_ends_on_the_final_response() {
    let walk = walk_through(
        "https://a.com/",
        &[
            redirect_to(Some("https://b.com/"), 301),
            redirect_to(Some("https://c.com/"), 302),
            final_response(200),
        ],
    );
    assert_eq!(walk.hops.len(), 2);
    assert_eq!(walk.hops[1].status, 302);
    assert!(matches!(
        walk.termination,
        RedirectWalkTermination::FinalResponse { status: 200, .. }
    ));
}

#[test]
fn walker_detects_a_revisited_url_as_a_loop() {
    let walk = walk_through(
        "https://a.com/",
        &[
            redirect_to(Some("https://b.com/"), 301),
            redirect_to(Some("https://a.com/"), 301),
        ],
    );
    assert_eq!(walk.hops.len(), 2);
    assert!(matches!(
        walk.termination,
        RedirectWalkTermination::Loop { ref url } if url == "https://a.com/"
    ));
}

#[test]
fn walker_stops_at_the_hop_limit_without_probing_further() {
    let outcomes: Vec<ProbeOutcome> = (0..REDIRECT_HOP_LIMIT)
        .map(|index| redirect_to(Some(&format!("https://example.com/{index}")), 301))
        .collect();
    let walk = walk_through("https://example.com/", &outcomes);
    assert_eq!(walk.hops.len(), REDIRECT_HOP_LIMIT);
    assert!(matches!(
        walk.termination,
        RedirectWalkTermination::HopLimitReached {
            limit: REDIRECT_HOP_LIMIT,
            ..
        }
    ));
}

#[test]
fn walker_classifies_transport_failure_missing_and_invalid_locations() {
    let failed = walk_through(
        "https://a.com/",
        &[ProbeOutcome::Failure(ProbeFailure {
            class: ProbeFailureClass::Transport,
            detail: "connection refused".into(),
        })],
    );
    assert!(matches!(
        failed.termination,
        RedirectWalkTermination::NetworkError { .. }
    ));

    let missing = walk_through("https://a.com/", &[redirect_to(None, 302)]);
    assert!(matches!(
        missing.termination,
        RedirectWalkTermination::MissingLocation { status: 302, .. }
    ));

    let invalid = walk_through("https://a.com/", &[redirect_to(Some("https://["), 301)]);
    assert!(matches!(
        invalid.termination,
        RedirectWalkTermination::InvalidLocation { status: 301, .. }
    ));
}

#[test]
fn walker_resolves_relative_locations_against_the_current_position() {
    let walk = walk_through(
        "http://localhost:3000/a",
        &[redirect_to(Some("/b"), 302), final_response(200)],
    );
    assert_eq!(walk.hops[0].to, "http://localhost:3000/b");
}

#[test]
fn a_loop_is_reported_as_a_loop_not_a_hop_count() {
    let walk = RedirectWalk {
        hops: vec![
            hop("https://a.com/", "https://b.com/"),
            hop("https://b.com/", "https://a.com/"),
        ],
        termination: RedirectWalkTermination::Loop {
            url: "https://a.com/".into(),
        },
    };
    let result = evaluate_redirect_chain("https://a.com/", &walk);
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.severity, Severity::Medium);
    assert_eq!(result.title, "Redirect loop detected");
    assert!(
        result.description.contains("never resolves"),
        "{}",
        result.description
    );
    assert_eq!(
        result.raw_data.as_ref().unwrap()["loop_url"],
        "https://a.com/"
    );
}

#[test]
fn a_plain_chain_is_still_reported_by_hop_count() {
    let walk = resolved(
        vec![
            hop("https://a.com/", "https://b.com/"),
            hop("https://b.com/", "https://c.com/"),
        ],
        "https://c.com/",
    );
    let result = evaluate_redirect_chain("https://a.com/", &walk);
    assert_eq!(result.status, CheckStatus::Warn);
    assert_eq!(result.title, "2 redirects before final response");
    assert_eq!(
        result.raw_data.as_ref().unwrap()["loop_url"],
        serde_json::Value::Null
    );
    assert!(
        !result.description.contains("100-300ms")
            && result.description.contains("request/response round trip"),
        "{}",
        result.description
    );
}

#[test]
fn no_redirects_pass() {
    let result = evaluate_redirect_chain("https://a.com/", &resolved(Vec::new(), "https://a.com/"));
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn network_failure_is_skipped_instead_of_reported_as_no_redirects() {
    let walk = RedirectWalk {
        hops: Vec::new(),
        termination: RedirectWalkTermination::NetworkError {
            url: "https://a.com/private/reset/short-token?token=secret".into(),
        },
    };
    let result = evaluate_redirect_chain("https://a.com/", &walk);
    assert_eq!(result.status, CheckStatus::Skipped);
    assert!(result.description.contains("inconclusive"));
    assert!(!result.description.contains("No redirects detected"));
    let serialized = serde_json::to_string(&result).expect("serialize result");
    assert!(
        serialized.contains("/private/reset/[redacted]"),
        "{serialized}"
    );
    assert!(!serialized.contains("short-token"), "{serialized}");
    assert!(!serialized.contains("secret"), "{serialized}");
}

#[test]
fn missing_location_and_hop_limit_are_not_described_as_final_urls() {
    let missing = RedirectWalk {
        hops: Vec::new(),
        termination: RedirectWalkTermination::MissingLocation {
            url: "https://a.com/".into(),
            status: 302,
        },
    };
    let result = evaluate_redirect_chain("https://a.com/", &missing);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.title.contains("no Location"));

    let limited = RedirectWalk {
        hops: vec![hop("https://a.com/", "https://b.com/")],
        termination: RedirectWalkTermination::HopLimitReached {
            url: "https://b.com/".into(),
            limit: 10,
        },
    };
    let result = evaluate_redirect_chain("https://a.com/", &limited);
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result
        .description
        .contains("did not reach a final response"));
    assert!(!result.title.contains("before final URL"));
}

#[test]
fn relative_location_keeps_the_port() {
    assert_eq!(
        super::resolve_location("http://localhost:3000/a", "/b").as_deref(),
        Some("http://localhost:3000/b")
    );
}

#[test]
fn scheme_relative_location_resolves_to_the_new_host() {
    assert_eq!(
        super::resolve_location("https://a.example.com/x", "//b.example.com/y").as_deref(),
        Some("https://b.example.com/y")
    );
}

#[test]
fn bare_relative_location_resolves_against_the_current_path() {
    assert_eq!(
        super::resolve_location("https://a.example.com/dir/page", "other").as_deref(),
        Some("https://a.example.com/dir/other")
    );
}
