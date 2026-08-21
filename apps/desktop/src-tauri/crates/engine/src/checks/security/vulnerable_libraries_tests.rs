use super::*;

fn lib(name: &str, version: &str) -> DetectedLibrary {
    DetectedLibrary {
        name: name.to_string(),
        version: version.to_string(),
        source_url: format!("/js/{}-{}.min.js", name, version),
    }
}

fn advisory(name: &str, version: &str, id: &str, severity: &str) -> LibraryAdvisory {
    LibraryAdvisory {
        package_name: name.to_string(),
        current_version: version.to_string(),
        advisory_id: id.to_string(),
        severity: severity.to_string(),
        advisory_url: Some(format!("https://osv.dev/vulnerability/{id}")),
        fixed_version: Some("9.9.9".to_string()),
    }
}

#[test]
fn clean_pass_copy_pluralizes_properly() {
    let one = clean_pass_result(&[lib("jquery", "3.7.1")]);
    assert_eq!(one.status, CheckStatus::Pass);
    assert!(
        one.description
            .starts_with("1 recognizable library with a pinned version detected"),
        "singular copy: {}",
        one.description
    );

    let two = clean_pass_result(&[lib("jquery", "3.7.1"), lib("vue", "3.4.0")]);
    assert!(
        two.description
            .starts_with("2 recognizable libraries with pinned versions detected"),
        "plural copy: {}",
        two.description
    );
}

#[test]
fn osv_unreachable_is_skipped_and_makes_no_verification_claim() {
    let results =
        evaluate_vulnerable_libraries(&[lib("jquery", "1.12.4")], AdvisoryLookup::Unavailable);
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert!(
        results[0].description.contains("could not be reached"),
        "copy must say OSV was unreachable: {}",
        results[0].description
    );
    assert!(
        !results[0].description.contains("no known advisories"),
        "must not claim OSV verified anything: {}",
        results[0].description
    );
    assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
}

#[test]
fn an_empty_answer_is_a_verified_pass_but_no_answer_never_is() {
    let answered = evaluate_vulnerable_libraries(
        &[lib("jquery", "3.7.1")],
        AdvisoryLookup::Answered(Vec::new()),
    );
    assert_eq!(answered[0].status, CheckStatus::Pass);
    assert!(answered[0].description.contains("no known advisories"));

    let unavailable =
        evaluate_vulnerable_libraries(&[lib("jquery", "3.7.1")], AdvisoryLookup::Unavailable);
    assert_eq!(unavailable[0].status, CheckStatus::Skipped);
}

#[test]
fn nothing_detectable_emits_no_rows_in_either_lookup_state() {
    assert!(evaluate_vulnerable_libraries(&[], AdvisoryLookup::Answered(Vec::new())).is_empty());
    assert!(evaluate_vulnerable_libraries(&[], AdvisoryLookup::Unavailable).is_empty());
}

#[test]
fn a_matching_advisory_fails_at_the_worst_reported_severity() {
    let medium = evaluate_vulnerable_libraries(
        &[lib("jquery", "1.12.4")],
        AdvisoryLookup::Answered(vec![advisory(
            "jquery",
            "1.12.4",
            "GHSA-test-medium",
            "moderate",
        )]),
    );
    assert_eq!(medium[0].status, CheckStatus::Fail);
    assert_eq!(medium[0].severity, Severity::Medium);
    assert!(medium[0].description.contains("GHSA-test-medium"));
    assert!(medium[0].description.contains("does not establish"));

    let high = evaluate_vulnerable_libraries(
        &[lib("jquery", "1.12.4")],
        AdvisoryLookup::Answered(vec![
            advisory("jquery", "1.12.4", "GHSA-test-medium", "moderate"),
            advisory("jquery", "1.12.4", "GHSA-test-crit", "critical"),
        ]),
    );
    assert_eq!(high[0].severity, Severity::High);
}

#[test]
fn advisories_for_versions_this_page_does_not_carry_do_not_fail_it() {
    // A database answer keyed to a different version must not be attributed
    // to the version actually on the page.
    let results = evaluate_vulnerable_libraries(
        &[lib("jquery", "3.7.1")],
        AdvisoryLookup::Answered(vec![advisory(
            "jquery",
            "1.12.4",
            "GHSA-old-version",
            "high",
        )]),
    );
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn the_advisory_id_sample_is_bounded_and_discloses_truncation() {
    let advisories: Vec<LibraryAdvisory> = (0..6)
        .map(|index| advisory("jquery", "1.12.4", &format!("GHSA-{index}"), "moderate"))
        .collect();
    let results = evaluate_vulnerable_libraries(
        &[lib("jquery", "1.12.4")],
        AdvisoryLookup::Answered(advisories),
    );
    assert!(results[0].description.contains("6 advisories"));
    assert!(
        results[0].description.contains("GHSA-3, ..."),
        "the id sample must stop at four and say so: {}",
        results[0].description
    );
    assert!(!results[0].description.contains("GHSA-4"));
}

#[test]
fn detects_cdnjs_and_npm_cdn_paths() {
    let body = r#"
        <script src="https://cdnjs.cloudflare.com/ajax/libs/jquery/1.12.4/jquery.min.js"></script>
        <script src="https://cdn.jsdelivr.net/npm/vue@2.6.14/dist/vue.js"></script>
        <script src="https://unpkg.com/react@16.8.0/umd/react.production.min.js"></script>
    "#;
    let libs = detect_libraries(body);
    let names: Vec<(&str, &str)> = libs
        .iter()
        .map(|l| (l.name.as_str(), l.version.as_str()))
        .collect();
    assert!(names.contains(&("jquery", "1.12.4")));
    assert!(names.contains(&("vue", "2.6.14")));
    assert!(names.contains(&("react", "16.8.0")));
}

#[test]
fn versioned_filenames_only_match_known_libraries() {
    let body = r#"
        <script src="/js/jquery-1.11.0.min.js"></script>
        <script src="/js/app-2.1.3.min.js"></script>
        <script src="/js/bootstrap-3.3.7.min.js"></script>
    "#;
    let libs = detect_libraries(body);
    let names: Vec<&str> = libs.iter().map(|l| l.name.as_str()).collect();
    assert!(names.contains(&"jquery"));
    assert!(names.contains(&"bootstrap"));
    assert!(
        !names.contains(&"app"),
        "site-internal bundle names must not be treated as npm packages"
    );
}

#[test]
fn scoped_packages_and_dedup() {
    let body = r#"
        <script src="https://unpkg.com/@popperjs/core@2.11.8/dist/umd/popper.min.js"></script>
        <script src="https://unpkg.com/@popperjs/core@2.11.8/dist/umd/popper.min.js"></script>
    "#;
    let libs = detect_libraries(body);
    assert_eq!(libs.len(), 1);
    assert_eq!(libs[0].name, "@popperjs/core");
    assert_eq!(libs[0].version, "2.11.8");
}

#[test]
fn unversioned_scripts_are_ignored() {
    let libs = detect_libraries(r#"<script src="/js/jquery.min.js"></script>"#);
    assert!(libs.is_empty());
}

#[test]
fn detection_reads_real_script_tags_and_ignores_inert_examples() {
    let body = r#"<script>
        const docs = '<script src="https://cdnjs.cloudflare.com/ajax/libs/jquery/1.12.4/jquery.min.js">';
    </script>
    <!-- <script src="https://cdnjs.cloudflare.com/ajax/libs/vue/2.6.0/vue.min.js"></script> -->
    <script src=https://cdnjs.cloudflare.com/ajax/libs/bootstrap/4.0.0/bootstrap.min.js></script>"#;
    let found = detect_libraries(body);

    assert_eq!(found.len(), 1, "unexpected detections: {found:?}");
    assert_eq!(found[0].name, "bootstrap");
    assert_eq!(found[0].version, "4.0.0");
}
