use super::{admit_deferred_target, classify_admission_error, HopOutcome, MainFrame};
use crate::webview::analyzer::NavigationGate;
use sitecmd_engine::browser::AdmittedDocuments;
use tauri::webview::PageLoadEvent;
use url::Url;

fn parse(url: &str) -> Url {
    Url::parse(url).expect("test url")
}

/// A local-dev gate: local names validate without touching DNS, so the state
/// machine runs offline.
fn local_gate() -> (
    std::sync::Arc<NavigationGate>,
    tokio::sync::mpsc::UnboundedReceiver<Url>,
) {
    NavigationGate::new(&parse("http://localhost:3000/"), true)
}

#[test]
fn the_main_frame_records_commits_but_never_the_blank_start_page() {
    let main_frame = MainFrame::default();
    assert_eq!(main_frame.committed(), None);

    main_frame.record(PageLoadEvent::Started, &parse("about:blank"));
    main_frame.record(PageLoadEvent::Finished, &parse("about:blank"));
    assert_eq!(main_frame.committed(), None);

    // Finished never precedes Started for the same document; only the
    // commit says which document the main frame holds.
    main_frame.record(PageLoadEvent::Finished, &parse("http://localhost:3000/"));
    assert_eq!(main_frame.committed(), None);

    main_frame.record(PageLoadEvent::Started, &parse("http://localhost:3000/"));
    assert_eq!(
        main_frame.committed(),
        Some(parse("http://localhost:3000/"))
    );
}

#[tokio::test]
async fn a_deferred_target_before_commit_is_a_redirect_hop_and_renavigates() {
    let (gate, mut deferred) = local_gate();
    let main_frame = MainFrame::default();
    assert!(!gate.decide(&parse("http://app.localhost:4000/")));
    let hop = deferred.try_recv().expect("deferred hop");

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    let mut admitted = AdmittedDocuments::new(&parse("http://localhost:3000/"));
    let outcome = admit_deferred_target(
        &gate,
        &main_frame,
        hop,
        &mut hops,
        &mut admitted,
        &mut |target| navigated.push(target),
    )
    .await;

    assert_eq!(outcome, HopOutcome::Followed);
    assert_eq!(navigated, vec![parse("http://app.localhost:4000/")]);
    assert_eq!(hops, 1);
    assert!(gate.decide(&parse("http://app.localhost:4000/")));
    assert_eq!(
        admitted.verify(Some("http://app.localhost:4000/welcome")),
        Ok(()),
        "a followed hop is a document the analyzer may grade"
    );
}

#[tokio::test]
async fn a_deferred_target_after_commit_is_a_subframe_and_never_renavigates() {
    // The visityourteam.com failure: an ad iframe's URL reached the gate
    // after the page committed and the analyzer followed it as a redirect.
    let (gate, mut deferred) = local_gate();
    let main_frame = MainFrame::default();
    main_frame.record(PageLoadEvent::Started, &parse("http://localhost:3000/"));
    assert!(!gate.decide(&parse("http://ads.localhost:5000/frame")));
    let subframe = deferred.try_recv().expect("deferred subframe");

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    let mut admitted = AdmittedDocuments::new(&parse("http://localhost:3000/"));
    let outcome = admit_deferred_target(
        &gate,
        &main_frame,
        subframe,
        &mut hops,
        &mut admitted,
        &mut |target| navigated.push(target),
    )
    .await;

    assert_eq!(
        outcome,
        HopOutcome::StayedOnDocument {
            host_admitted: true
        }
    );
    assert!(navigated.is_empty(), "the main frame keeps its document");
    assert_eq!(hops, 0, "a subframe never spends the redirect budget");
    assert_eq!(outcome.scan_failure(), None);
    assert!(
        gate.decide(&parse("http://ads.localhost:5000/other-frame")),
        "later loads of the validated host go through"
    );
    assert!(
        admitted
            .verify(Some("http://ads.localhost:5000/frame"))
            .is_err(),
        "an admitted subframe host is still not a document the analyzer may grade"
    );
}

#[tokio::test]
async fn a_page_initiated_navigation_after_commit_is_not_followed() {
    // Once the page has committed, a cross-host top-level navigation it
    // starts looks exactly like a subframe to the gate and gets the same
    // answer: the analyzer stays on the document it was asked about.
    let (gate, mut deferred) = local_gate();
    let main_frame = MainFrame::default();
    main_frame.record(PageLoadEvent::Started, &parse("http://localhost:3000/"));
    assert!(!gate.decide(&parse("http://other.localhost:5000/")));
    let navigation = deferred.try_recv().expect("deferred navigation");

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    let mut admitted = AdmittedDocuments::new(&parse("http://localhost:3000/"));
    let outcome = admit_deferred_target(
        &gate,
        &main_frame,
        navigation,
        &mut hops,
        &mut admitted,
        &mut |target| navigated.push(target),
    )
    .await;

    assert!(matches!(outcome, HopOutcome::StayedOnDocument { .. }));
    assert!(navigated.is_empty());
    assert_eq!(
        admitted.verify(Some("http://localhost:3000/pricing")),
        Ok(()),
        "the committed document is the only one the analyzer grades"
    );
}

#[tokio::test]
async fn a_refused_subframe_host_is_not_a_scan_failure() {
    // An iframe pointing into a private range stays blocked, and the page
    // still gets graded: only an unreachable target fails the browser layer.
    let (gate, _deferred) = local_gate();
    let main_frame = MainFrame::default();
    main_frame.record(PageLoadEvent::Started, &parse("http://localhost:3000/"));

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    let mut admitted = AdmittedDocuments::new(&parse("http://localhost:3000/"));
    let outcome = admit_deferred_target(
        &gate,
        &main_frame,
        parse("http://192.168.1.1/"),
        &mut hops,
        &mut admitted,
        &mut |target| navigated.push(target),
    )
    .await;

    assert_eq!(
        outcome,
        HopOutcome::StayedOnDocument {
            host_admitted: false
        }
    );
    assert_eq!(outcome.scan_failure(), None);
    assert!(navigated.is_empty());
    assert!(!gate.decide(&parse("http://192.168.1.1/")));
}

#[tokio::test]
async fn deferred_hops_are_validated_navigated_and_capped() {
    let (gate, mut deferred) = local_gate();
    let main_frame = MainFrame::default();
    assert!(!gate.decide(&parse("http://app.localhost:4000/")));
    let hop = deferred.try_recv().expect("deferred hop");

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    let mut admitted = AdmittedDocuments::new(&parse("http://localhost:3000/"));
    assert_eq!(
        admit_deferred_target(
            &gate,
            &main_frame,
            hop,
            &mut hops,
            &mut admitted,
            &mut |target| navigated.push(target)
        )
        .await,
        HopOutcome::Followed
    );
    assert_eq!(navigated.len(), 1);
    assert!(gate.decide(&parse("http://app.localhost:4000/")));

    let refusal = admit_deferred_target(
        &gate,
        &main_frame,
        parse("http://192.168.1.1/"),
        &mut hops,
        &mut admitted,
        &mut |target| navigated.push(target),
    )
    .await;
    assert_eq!(refusal, HopOutcome::RefusedByPolicy);
    assert_eq!(navigated.len(), 1, "a refused hop is never navigated");
    assert!(
        refusal.scan_failure().is_some(),
        "a refused hop fails the analysis instead of reporting a completed run"
    );
    assert!(
        admitted.verify(Some("http://192.168.1.1/")).is_err(),
        "a refused hop is never an admitted document"
    );

    hops = crate::constants::MAX_REDIRECT_HOPS;
    assert_eq!(
        admit_deferred_target(
            &gate,
            &main_frame,
            parse("http://other.localhost:5000/"),
            &mut hops,
            &mut admitted,
            &mut |target| navigated.push(target)
        )
        .await,
        HopOutcome::HopLimitReached
    );
    assert_eq!(navigated.len(), 1, "the hop budget stops the chain");
}

#[tokio::test]
async fn a_burst_of_deferred_hops_stops_at_the_cap() {
    let (gate, mut deferred) = local_gate();
    let main_frame = MainFrame::default();
    let cap = crate::constants::MAX_REDIRECT_HOPS;
    let burst = cap + 3;
    for index in 0..burst {
        assert!(!gate.decide(&parse(&format!("http://hop{index}.localhost/"))));
    }

    let mut navigated: Vec<Url> = Vec::new();
    let mut hops = 0usize;
    let mut admitted = AdmittedDocuments::new(&parse("http://localhost:3000/"));
    let mut outcomes = Vec::new();
    while let Ok(hop) = deferred.try_recv() {
        outcomes.push(
            admit_deferred_target(
                &gate,
                &main_frame,
                hop,
                &mut hops,
                &mut admitted,
                &mut |target| navigated.push(target),
            )
            .await,
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
    assert_eq!(
        HopOutcome::StayedOnDocument {
            host_admitted: false
        }
        .scan_failure(),
        None
    );
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
