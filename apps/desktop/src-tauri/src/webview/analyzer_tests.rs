use super::{
    browser_build_from_user_agent, classify_admission_error, follow_deferred_hop, poll_webview,
    HopOutcome, NavigationGate, READY_PROBE_SCRIPT,
};
use std::time::Duration;
use url::Url;

fn parse(url: &str) -> Url {
    Url::parse(url).expect("test url")
}

#[tokio::test(start_paused = true)]
async fn poll_webview_returns_immediately_when_probe_is_ready() {
    let start = tokio::time::Instant::now();
    let result = poll_webview(Duration::from_millis(100), Duration::from_secs(8), || {
        Some(42)
    })
    .await;
    assert_eq!(result, Some(42));
    assert_eq!(start.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn poll_webview_returns_as_soon_as_probe_succeeds() {
    let start = tokio::time::Instant::now();
    let mut calls = 0;
    let result = poll_webview(Duration::from_millis(100), Duration::from_secs(8), || {
        calls += 1;
        (calls >= 5).then_some(())
    })
    .await;
    assert_eq!(result, Some(()));
    // 5th probe fires after 4 sleeps: 400ms, nowhere near the 8s cap.
    assert_eq!(start.elapsed(), Duration::from_millis(400));
}

#[tokio::test(start_paused = true)]
async fn poll_webview_gives_up_at_the_cap() {
    let start = tokio::time::Instant::now();
    let result = poll_webview(Duration::from_millis(100), Duration::from_secs(1), || {
        None::<()>
    })
    .await;
    assert_eq!(result, None);
    assert!(start.elapsed() <= Duration::from_secs(1));
    assert!(start.elapsed() >= Duration::from_millis(900));
}

#[test]
fn gate_refuses_private_literals_inline_without_deferring() {
    let (gate, mut deferred) = NavigationGate::new(&parse("https://example.com/"), false);
    for url in [
        "http://169.254.169.254/latest/meta-data/",
        "http://192.168.1.1/",
        "http://10.0.0.5/",
        "http://172.16.0.1/",
        "http://127.0.0.1:3000/",
        "http://[::1]:3000/",
        "http://localhost:3000/",
        "http://metadata.google.internal/",
    ] {
        assert!(!gate.decide(&parse(url)), "{url}");
    }
    assert!(
        deferred.try_recv().is_err(),
        "literals and local names are decided inline, never deferred to DNS"
    );
}

#[test]
fn gate_allows_the_origin_and_defers_unknown_hosts_until_dns_admits_them() {
    let (gate, mut deferred) = NavigationGate::new(&parse("https://example.com/"), false);
    assert!(gate.decide(&parse("https://example.com/page")));
    assert!(gate.decide(&parse("https://EXAMPLE.com./other")));
    assert!(gate.decide(&parse("about:blank")));

    assert!(!gate.decide(&parse("https://cdn.example.net/")));
    assert_eq!(
        deferred.try_recv().expect("deferred hop").as_str(),
        "https://cdn.example.net/"
    );
    gate.allow_host("cdn.example.net");
    assert!(gate.decide(&parse("https://cdn.example.net/")));
}

#[tokio::test]
async fn dns_admission_validates_before_allowing() {
    let (gate, _deferred) = NavigationGate::new(&parse("https://example.com/"), false);
    assert!(gate
        .admit_after_dns(&parse("http://10.0.0.5/"))
        .await
        .is_err());
    assert!(gate
        .admit_after_dns(&parse("http://localhost/"))
        .await
        .is_err());
    assert!(!gate.decide(&parse("http://localhost/")));
}

#[tokio::test]
async fn deferred_hops_are_validated_navigated_and_capped() {
    let (gate, mut deferred) = NavigationGate::new(&parse("http://localhost:3000/"), true);
    assert!(!gate.decide(&parse("http://app.localhost:4000/")));
    let hop = deferred.try_recv().expect("deferred hop");

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    assert_eq!(
        follow_deferred_hop(&gate, hop, &mut hops, &mut |target| navigated.push(target)).await,
        HopOutcome::Followed
    );
    assert_eq!(navigated.len(), 1);
    assert!(gate.decide(&parse("http://app.localhost:4000/")));

    let refusal = follow_deferred_hop(
        &gate,
        parse("http://192.168.1.1/"),
        &mut hops,
        &mut |target| navigated.push(target),
    )
    .await;
    assert_eq!(refusal, HopOutcome::RefusedByPolicy);
    assert_eq!(navigated.len(), 1, "a refused hop is never navigated");
    assert!(
        refusal.scan_failure().is_some(),
        "a refused hop fails the analysis instead of reporting a completed run"
    );

    hops = crate::constants::MAX_REDIRECT_HOPS;
    assert_eq!(
        follow_deferred_hop(
            &gate,
            parse("http://other.localhost:5000/"),
            &mut hops,
            &mut |target| navigated.push(target)
        )
        .await,
        HopOutcome::HopLimitReached
    );
    assert_eq!(navigated.len(), 1, "the hop budget stops the chain");
}

#[tokio::test]
async fn a_burst_of_deferred_hops_stops_at_the_cap() {
    let (gate, mut deferred) = NavigationGate::new(&parse("http://localhost:3000/"), true);
    let cap = crate::constants::MAX_REDIRECT_HOPS;
    let burst = cap + 3;
    for index in 0..burst {
        assert!(!gate.decide(&parse(&format!("http://hop{index}.localhost/"))));
    }

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    let mut outcomes = Vec::new();
    while let Ok(hop) = deferred.try_recv() {
        outcomes.push(
            follow_deferred_hop(&gate, hop, &mut hops, &mut |target| navigated.push(target)).await,
        );
    }

    assert_eq!(outcomes.len(), burst);
    assert_eq!(
        navigated.len(),
        cap,
        "the navigate sink runs at most once per allowed hop"
    );
    assert!(outcomes[..cap]
        .iter()
        .all(|outcome| *outcome == HopOutcome::Followed));
    assert!(outcomes[cap..]
        .iter()
        .all(|outcome| *outcome == HopOutcome::HopLimitReached));
    assert!(
        outcomes[cap].scan_failure().is_some(),
        "the hop past the cap ends the page-load wait instead of spinning"
    );
}

#[test]
fn every_unfollowed_hop_fails_the_analysis_with_its_own_message() {
    assert_eq!(HopOutcome::Followed.scan_failure(), None);
    let failures: Vec<String> = [
        HopOutcome::RefusedByPolicy,
        HopOutcome::Unresolvable,
        HopOutcome::HopLimitReached,
    ]
    .iter()
    .map(|outcome| outcome.scan_failure().expect("failure message"))
    .collect();
    let distinct: std::collections::HashSet<&String> = failures.iter().collect();
    assert_eq!(
        distinct.len(),
        failures.len(),
        "policy refusal, resolution failure, and the hop cap report distinctly"
    );
}

#[test]
fn admission_errors_separate_resolution_failure_from_policy_refusal() {
    assert_eq!(
        classify_admission_error("Could not resolve URL host 'app.example.test': no address"),
        HopOutcome::Unresolvable
    );
    assert_eq!(
        classify_admission_error("Cannot access private/internal IP address '10.0.0.5'."),
        HopOutcome::RefusedByPolicy
    );
}

#[test]
fn readiness_probe_never_fires_on_the_blank_start_page() {
    assert!(READY_PROBE_SCRIPT.contains("location.href !== 'about:blank'"));
    assert!(READY_PROBE_SCRIPT.contains("document.readyState === 'complete'"));
}

#[test]
fn explicit_local_scan_keeps_loopback_navigation() {
    let (gate, _deferred) = NavigationGate::new(&parse("http://localhost:3000/"), true);
    assert!(gate.decide(&parse("http://127.0.0.1:3000/")));
    assert!(gate.decide(&parse("http://localhost:3000/")));
    assert!(!gate.decide(&parse("http://192.168.1.1/")));
}

#[test]
fn browser_build_is_derived_from_the_runtime_user_agent() {
    assert_eq!(
        browser_build_from_user_agent(
            "webkit",
            "Mozilla/5.0 AppleWebKit/621.1.15 (KHTML, like Gecko) Version/18.5 Safari/621.1.15",
        )
        .as_deref(),
        Some("621.1.15")
    );
    assert_eq!(
        browser_build_from_user_agent(
            "webview2",
            "Mozilla/5.0 Chrome/136.0.7103.49 Safari/537.36 Edg/136.0.3240.50",
        )
        .as_deref(),
        Some("136.0.3240.50")
    );
}
