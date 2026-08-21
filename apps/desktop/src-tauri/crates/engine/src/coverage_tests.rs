//! Pair-precise coverage tests.

use super::*;

fn outcome<'a>(route: &'a str, check_id: &'a str, status: CheckStatus) -> CheckOutcome<'a> {
    CheckOutcome {
        route: Some(route),
        check_id,
        status,
    }
}

fn page_run(outcomes: &[CheckOutcome<'_>]) -> ScanCoverageManifest {
    ScanCoverageManifest::derive(
        ScanCoverageKind::Page,
        vec!["https://example.com/pricing".into()],
        outcomes,
        ClaimBasis::PerRoute,
    )
}

#[test]
fn a_verdict_proves_the_pair() {
    let coverage = page_run(&[
        outcome(
            "https://example.com/pricing",
            "security.csp",
            CheckStatus::Fail,
        ),
        outcome(
            "https://example.com/pricing",
            "seo.title",
            CheckStatus::Pass,
        ),
        outcome(
            "https://example.com/pricing",
            "seo.viewport",
            CheckStatus::Warn,
        ),
    ]);

    for check in ["security.csp", "seo.title", "seo.viewport"] {
        assert!(
            coverage.covers(Some("https://example.com/pricing"), check),
            "{check} reached a verdict"
        );
    }
}

#[test]
fn a_skipped_check_is_excepted_rather_than_claimed() {
    let coverage = page_run(&[
        outcome(
            "https://example.com/pricing",
            "security.csp",
            CheckStatus::Skipped,
        ),
        outcome(
            "https://example.com/pricing",
            "seo.title",
            CheckStatus::Pass,
        ),
    ]);

    assert!(!coverage.covers(Some("https://example.com/pricing"), "security.csp"));
    assert!(coverage.covers(Some("https://example.com/pricing"), "seo.title"));
    assert_eq!(
        coverage.exceptions,
        vec![CoverageException {
            route: Some("https://example.com/pricing".into()),
            checks_not_run: vec!["security.csp".into()],
            reason: CoverageExceptionReason::CheckSkipped,
        }]
    );
}

#[test]
fn one_skipped_outcome_excepts_the_pair_its_siblings_passed() {
    let coverage = page_run(&[
        outcome(
            "https://example.com/pricing",
            "security.exposed_files.summary",
            CheckStatus::Pass,
        ),
        outcome(
            "https://example.com/pricing",
            "security.exposed_files.summary",
            CheckStatus::Skipped,
        ),
    ]);

    assert!(
        !coverage.covers(
            Some("https://example.com/pricing"),
            "security.exposed_files.summary"
        ),
        "a family that half ran has not proven the half that did not"
    );
}

#[test]
fn a_family_is_claimed_by_its_runner_and_covers_the_ids_a_fix_removed() {
    let coverage = page_run(&[outcome(
        "https://example.com/pricing",
        "accessibility.axe.label",
        CheckStatus::Pass,
    )]);

    assert_eq!(coverage.checks, vec!["accessibility.axe.".to_string()]);
    assert!(coverage.covers(
        Some("https://example.com/pricing"),
        "accessibility.axe.image-alt"
    ));
}

#[test]
fn a_family_member_that_could_not_conclude_is_still_excepted() {
    let coverage = page_run(&[
        outcome(
            "https://example.com/pricing",
            "accessibility.axe.label",
            CheckStatus::Pass,
        ),
        outcome(
            "https://example.com/pricing",
            "accessibility.axe.color-contrast",
            CheckStatus::Skipped,
        ),
    ]);

    assert!(
        !coverage.covers(
            Some("https://example.com/pricing"),
            "accessibility.axe.color-contrast"
        ),
        "the family claim must not override a stated ignorance about one member"
    );
    assert!(coverage.covers(
        Some("https://example.com/pricing"),
        "accessibility.axe.label"
    ));
}

#[test]
fn an_exception_naming_a_family_removes_every_member() {
    let coverage = ScanCoverageManifest::derive(
        ScanCoverageKind::PageSet,
        vec!["https://example.com/".into()],
        &[outcome(
            "https://example.com/",
            "accessibility.axe.label",
            CheckStatus::Pass,
        )],
        ClaimBasis::RouteSet { complete: false },
    );

    assert_eq!(
        coverage.exceptions[0].checks_not_run,
        vec!["accessibility.axe.".to_string()]
    );
    assert!(!coverage.covers(None, "accessibility.axe.image-alt"));
    assert!(!coverage.covers(Some("https://example.com/"), "accessibility.axe.label"));
}

#[test]
fn a_family_that_never_ran_claims_none_of_its_members() {
    let coverage = page_run(&[outcome(
        "https://example.com/pricing",
        "security.csp",
        CheckStatus::Pass,
    )]);

    assert!(!coverage.covers(
        Some("https://example.com/pricing"),
        "accessibility.axe.image-alt"
    ));
}

#[test]
fn a_route_the_run_never_visited_is_not_covered() {
    let coverage = page_run(&[outcome(
        "https://example.com/pricing",
        "security.csp",
        CheckStatus::Pass,
    )]);

    assert!(!coverage.covers(Some("https://example.com/docs"), "security.csp"));
}

#[test]
fn a_trailing_slash_is_a_distinct_route() {
    let coverage = page_run(&[outcome(
        "https://example.com/pricing",
        "security.csp",
        CheckStatus::Pass,
    )]);

    assert!(!coverage.covers(Some("https://example.com/pricing/"), "security.csp"));
}

#[test]
fn a_check_the_run_never_executed_is_not_covered() {
    let coverage = page_run(&[outcome(
        "https://example.com/pricing",
        "security.csp",
        CheckStatus::Pass,
    )]);

    assert!(
        !coverage.covers(Some("https://example.com/pricing"), "seo.title"),
        "a check with no outcome is not part of the claim"
    );
}

#[test]
fn an_unsuccessful_run_proves_nothing_it_claims() {
    let mut coverage = page_run(&[outcome(
        "https://example.com/pricing",
        "security.csp",
        CheckStatus::Pass,
    )]);
    coverage.successful = false;

    assert!(!coverage.covers(Some("https://example.com/pricing"), "security.csp"));
}

#[test]
fn an_incomplete_route_set_proves_none_of_its_cross_page_claims() {
    let coverage = ScanCoverageManifest::derive(
        ScanCoverageKind::PageSet,
        vec![
            "https://example.com/".into(),
            "https://example.com/a".into(),
        ],
        &[
            outcome(
                "https://example.com/",
                "seo.duplicate_h1",
                CheckStatus::Pass,
            ),
            outcome(
                "https://example.com/",
                "seo.orphan_pages",
                CheckStatus::Warn,
            ),
        ],
        ClaimBasis::RouteSet { complete: false },
    );

    assert!(!coverage.covers(None, "seo.duplicate_h1"));
    assert!(!coverage.covers(Some("https://example.com/"), "seo.orphan_pages"));
    assert_eq!(
        coverage.exceptions[0].reason,
        CoverageExceptionReason::SessionIncomplete
    );
    assert_eq!(coverage.exceptions[0].route, None);
}

#[test]
fn a_complete_route_set_proves_its_cross_page_claims() {
    let coverage = ScanCoverageManifest::derive(
        ScanCoverageKind::PageSet,
        vec![
            "https://example.com/".into(),
            "https://example.com/a".into(),
        ],
        &[outcome(
            "https://example.com/",
            "seo.duplicate_h1",
            CheckStatus::Pass,
        )],
        ClaimBasis::RouteSet { complete: true },
    );

    assert!(coverage.covers(None, "seo.duplicate_h1"));
    assert!(coverage.exceptions.is_empty());
}

#[test]
fn a_site_level_finding_needs_one_unexcepted_route() {
    let coverage = ScanCoverageManifest::derive(
        ScanCoverageKind::PageSet,
        vec![
            "https://example.com/".into(),
            "https://example.com/a".into(),
        ],
        &[
            outcome(
                "https://example.com/",
                "seo.canonical_loop",
                CheckStatus::Skipped,
            ),
            outcome(
                "https://example.com/a",
                "seo.canonical_loop",
                CheckStatus::Pass,
            ),
        ],
        ClaimBasis::RouteSet { complete: true },
    );

    assert!(
        coverage.covers(None, "seo.canonical_loop"),
        "one claimed route proved it, which is what a site-level finding needs"
    );
}

#[test]
fn a_site_level_finding_is_not_covered_when_every_route_is_excepted() {
    let coverage = ScanCoverageManifest::derive(
        ScanCoverageKind::PageSet,
        vec!["https://example.com/".into()],
        &[outcome(
            "https://example.com/",
            "seo.canonical_loop",
            CheckStatus::Skipped,
        )],
        ClaimBasis::RouteSet { complete: true },
    );

    assert!(!coverage.covers(None, "seo.canonical_loop"));
}

#[test]
fn a_code_claim_ignores_routes_and_covers_every_declared_rule() {
    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::Project,
        Vec::new(),
        vec!["code_scan.security".into(), "code_scan.quality".into()],
    );

    assert!(coverage.covers(None, "code_scan.security"));
    assert!(coverage.covers(Some("anything"), "code_scan.quality"));
    assert!(
        !coverage.covers(None, "code_scan.retired"),
        "a rule the build no longer registers is outside the claim"
    );
}

#[test]
fn an_unproven_run_claims_nothing() {
    let coverage =
        ScanCoverageManifest::unproven(ScanCoverageKind::PageSet, vec!["https://a.test/".into()]);

    assert!(!coverage.successful);
    assert!(coverage.checks.is_empty());
    assert!(!coverage.covers(Some("https://a.test/"), "security.csp"));
}

#[test]
fn the_claim_and_its_exceptions_survive_a_round_trip() {
    let coverage = page_run(&[
        outcome(
            "https://example.com/pricing",
            "security.csp",
            CheckStatus::Skipped,
        ),
        outcome(
            "https://example.com/pricing",
            "seo.title",
            CheckStatus::Pass,
        ),
    ]);

    let json = serde_json::to_string(&coverage).expect("serialize");
    let restored: ScanCoverageManifest = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored, coverage);
    assert!(json.contains("\"checksNotRun\""));
    assert!(json.contains("\"check_skipped\""));
}

#[test]
fn a_manifest_stored_before_pairs_existed_reads_back_claiming_nothing() {
    let stored = r#"{"kind":"page","successful":true,
        "pageUrls":["https://example.com/"],"producerIds":[]}"#;

    let coverage: ScanCoverageManifest = serde_json::from_str(stored).expect("deserialize");

    assert!(coverage.checks.is_empty());
    assert!(!coverage.covers(Some("https://example.com/"), "security.csp"));
}

#[test]
fn validation_refuses_a_check_set_claim_with_no_checks() {
    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::CheckSet,
        vec!["https://example.com/".into()],
        Vec::new(),
    );

    assert!(coverage.validate().is_err());
}

#[test]
fn validation_refuses_a_page_claim_with_no_route() {
    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::Page,
        Vec::new(),
        vec!["security.csp".into()],
    );

    assert!(coverage.validate().is_err());
}

#[test]
fn every_kind_states_whether_it_observes_routes() {
    for kind in [
        ScanCoverageKind::Site,
        ScanCoverageKind::PageSet,
        ScanCoverageKind::Page,
        ScanCoverageKind::CheckSet,
    ] {
        assert!(kind.is_route_scoped(), "{} observes routes", kind.as_str());
    }
    for kind in [ScanCoverageKind::Project, ScanCoverageKind::RuleSet] {
        assert!(
            !kind.is_route_scoped(),
            "{} observes a project tree",
            kind.as_str()
        );
    }
}
