//! Cross-page session analysis tests.

use super::*;

fn page(url: &str) -> PageSignals {
    PageSignals {
        url: url.to_string(),
        title: None,
        meta_description: None,
        h1: None,
        canonical: None,
        noindex: false,
        hreflang: Vec::new(),
        internal_links: Vec::new(),
        internal_links_truncated: false,
    }
}

#[test]
fn single_page_sessions_emit_nothing() {
    let pages = vec![page("https://a.com")];
    assert!(analyze_session(&pages, None).is_empty());
}

#[test]
fn duplicate_titles_grouped_case_insensitively() {
    let mut a = page("https://a.com/1");
    a.title = Some("Welcome".into());
    let mut b = page("https://a.com/2");
    b.title = Some("welcome".into());
    let mut c = page("https://a.com/3");
    c.title = Some("Unique".into());
    let results = analyze_session(&[a, b, c], None);
    let dup = results
        .iter()
        .find(|r| r.check_id == "seo.duplicate_title_across_pages")
        .expect("duplicate title finding");
    assert_eq!(dup.status, CheckStatus::Warn);
    assert!(dup.description.contains("welcome"));
    assert_eq!(dup.confidence, IssueConfidence::NeedsReview);
    assert!(!dup.description.contains("compete with each other"));
    assert_eq!(
        outcome(&results, "seo.duplicate_h1").status,
        CheckStatus::Pass
    );
}

#[test]
fn duplicate_descriptions_use_a_cross_page_identity() {
    let mut a = page("https://a.com/1");
    a.meta_description = Some("Shared summary".into());
    let mut b = page("https://a.com/2");
    b.meta_description = Some("shared summary".into());

    let results = analyze_session(&[a, b], None);

    assert!(results
        .iter()
        .any(|result| result.check_id == "seo.duplicate_description_across_pages"));
    assert!(!results
        .iter()
        .any(|result| result.check_id == "seo.duplicate_description"));
}

#[test]
fn duplicate_h1_copy_does_not_claim_the_heading_is_a_search_snippet() {
    let mut a = page("https://a.com/1");
    a.h1 = Some("Documentation".into());
    let mut b = page("https://a.com/2");
    b.h1 = Some("documentation".into());
    let finding = analyze_session(&[a, b], None)
        .into_iter()
        .find(|result| result.check_id == "seo.duplicate_h1")
        .expect("duplicate H1 finding");
    assert!(finding.description.contains("initial HTML"));
    assert!(!finding.description.contains("search results"));
}

#[test]
fn orphans_need_minimum_pages_and_skip_entry_page() {
    let mut home = page("https://a.com/");
    home.internal_links = vec![
        "https://a.com/x".into(),
        "https://a.com/y".into(),
        "https://a.com/z".into(),
    ];
    let x = page("https://a.com/x");
    let y = page("https://a.com/y");
    let z = page("https://a.com/z");
    let orphan = page("https://a.com/lost");
    let results = analyze_session(&[home, x, y, z, orphan], None);
    let finding = results
        .iter()
        .find(|r| r.check_id == "seo.orphan_pages")
        .expect("orphan finding");
    assert!(finding.description.contains("https://a.com/lost"));
    assert_eq!(finding.confidence, IssueConfidence::NeedsReview);
    assert!(!finding
        .description
        .contains("only reachable through the sitemap"));
    assert!(!finding
        .why_it_matters
        .as_deref()
        .unwrap_or("")
        .contains("entirely"));
}

#[test]
fn noindex_in_sitemap_contradiction_detected() {
    let mut a = page("https://a.com/private");
    a.noindex = true;
    let b = page("https://a.com/public");
    let sitemap = vec![
        "https://a.com/private/".to_string(),
        "https://a.com/public".to_string(),
    ];
    let results = analyze_session(&[a, b], Some(&sitemap));
    let finding = results
        .iter()
        .find(|r| r.check_id == "seo.noindex_in_sitemap")
        .expect("noindex finding");
    assert!(finding.description.contains("/private"));
    assert!(finding.description.contains("intent needs review"));
    assert!(!finding.description.contains("wastes crawl attention"));
}

#[test]
fn canonical_two_page_loop_reported_once() {
    let mut a = page("https://a.com/a");
    a.canonical = Some("https://a.com/b".into());
    let mut b = page("https://a.com/b");
    b.canonical = Some("https://a.com/a".into());
    let results = analyze_session(&[a, b], None);
    let finding = results
        .iter()
        .find(|r| r.check_id == "seo.canonical_loop")
        .expect("canonical loop finding");
    assert_eq!(finding.status, CheckStatus::Fail);
    assert!(finding.description.contains("initial-HTML canonical"));
    assert!(!finding.description.contains("unpredictably"));
    assert_eq!(
        finding.raw_data.as_ref().unwrap()["loops"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn indirect_canonical_chain_is_a_review_warning_not_a_loop_failure() {
    let mut a = page("https://a.com/a");
    a.canonical = Some("https://a.com/b".into());
    let mut b = page("https://a.com/b");
    b.canonical = Some("https://a.com/c".into());
    let c = page("https://a.com/c");
    let finding = analyze_session(&[a, b, c], None)
        .into_iter()
        .find(|result| result.check_id == "seo.canonical_loop")
        .expect("canonical chain finding");
    assert_eq!(finding.status, CheckStatus::Warn);
    assert!(finding.title.contains("chain"));
    assert!(finding.description.contains("can still settle"));
}

#[test]
fn three_page_canonical_cycle_is_classified_as_a_cycle() {
    let mut a = page("https://a.com/a");
    a.canonical = Some("https://a.com/b".into());
    let mut b = page("https://a.com/b");
    b.canonical = Some("https://a.com/c".into());
    let mut c = page("https://a.com/c");
    c.canonical = Some("https://a.com/a".into());
    let finding = analyze_session(&[a, b, c], None)
        .into_iter()
        .find(|result| result.check_id == "seo.canonical_loop")
        .expect("canonical cycle finding");
    assert_eq!(finding.status, CheckStatus::Fail);
    assert!(finding.title.contains("cycle"));
    assert_eq!(
        finding.raw_data.as_ref().unwrap()["loops"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn session_evidence_removes_query_values_but_keeps_useful_page_paths() {
    let mut home = page("https://a.com/");
    home.internal_links = vec![
        "https://a.com/x".into(),
        "https://a.com/y".into(),
        "https://a.com/z".into(),
    ];
    let x = page("https://a.com/x");
    let y = page("https://a.com/y");
    let z = page("https://a.com/z");
    let orphan = page("https://a.com/lost?token=secret#fragment");
    let finding = analyze_session(&[home, x, y, z, orphan], None)
        .into_iter()
        .find(|result| result.check_id == "seo.orphan_pages")
        .expect("orphan finding");
    let serialized = serde_json::to_string(&finding).unwrap();
    assert!(serialized.contains("/lost"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("fragment"));
}

#[test]
fn hreflang_missing_return_link_flagged_only_for_scanned_targets() {
    let mut en = page("https://a.com/en");
    en.hreflang = vec![
        ("de".into(), "https://a.com/de".into()),
        ("fr".into(), "https://a.com/fr".into()),
    ];
    let de = page("https://a.com/de"); // scanned, no return link
    let results = analyze_session(&[en, de], None);
    let finding = results
        .iter()
        .find(|r| r.check_id == "seo.hreflang_reciprocity")
        .expect("hreflang finding");
    // The fr target was not scanned, so only the de pair is reported.
    let missing = finding.raw_data.as_ref().unwrap()["missing_return_links"]
        .as_array()
        .unwrap();
    assert_eq!(missing.len(), 1);
    assert!(missing[0].as_str().unwrap().contains("/de"));
    assert_eq!(finding.confidence, IssueConfidence::NeedsReview);
    assert!(finding.description.contains("initial-HTML"));
    assert!(!finding.description.contains("silently does nothing"));
}

#[test]
fn self_referencing_canonicals_and_reciprocal_hreflang_pass() {
    let mut a = page("https://a.com/a");
    a.canonical = Some("https://a.com/a".into());
    a.hreflang = vec![
        ("en".into(), "https://a.com/a".into()),
        ("de".into(), "https://a.com/b".into()),
    ];
    let mut b = page("https://a.com/b");
    b.hreflang = vec![
        ("de".into(), "https://a.com/b".into()),
        ("en".into(), "https://a.com/a".into()),
    ];
    let results = analyze_session(&[a, b], None);
    assert_eq!(
        outcome(&results, "seo.canonical_loop").status,
        CheckStatus::Pass
    );
    assert_eq!(
        outcome(&results, "seo.hreflang_reciprocity").status,
        CheckStatus::Pass
    );
}

// The one row for a check. Every session check reports exactly one
// outcome per run, which is what makes coverage derivable from them.
fn outcome<'a>(results: &'a [CheckResult], check_id: &str) -> &'a CheckResult {
    let mut matching = results.iter().filter(|r| r.check_id == check_id);
    let found = matching
        .next()
        .unwrap_or_else(|| panic!("{check_id} reported no outcome"));
    assert!(
        matching.next().is_none(),
        "{check_id} reported more than one outcome"
    );
    found
}

#[test]
fn an_outcome_row_names_what_it_checked_not_its_check_id() {
    let mut a = page("https://a.com/1");
    a.title = Some("One".into());
    let mut b = page("https://a.com/2");
    b.title = Some("Two".into());

    let results = analyze_session(&[a, b], None);
    let clean = outcome(&results, "seo.duplicate_h1");

    assert_eq!(
        clean.title,
        "No shared H1 headings across the scanned pages"
    );
    assert!(!clean.description.contains("seo."));
}

#[test]
fn every_session_check_reports_an_outcome() {
    let mut a = page("https://a.com/1");
    a.title = Some("One".into());
    let mut b = page("https://a.com/2");
    b.title = Some("Two".into());

    let results = analyze_session(&[a, b], None);

    for check_id in SESSION_CHECK_IDS {
        let result = outcome(&results, check_id);
        assert!(
            matches!(result.status, CheckStatus::Pass | CheckStatus::Skipped),
            "{check_id} reported {:?} on a clean two-page set",
            result.status
        );
    }
}

#[test]
fn a_check_whose_inputs_were_missing_reports_skipped_not_pass() {
    let mut a = page("https://a.com/1");
    a.title = Some("One".into());
    let mut b = page("https://a.com/2");
    b.title = Some("Two".into());

    let results = analyze_session(&[a, b], None);

    assert_eq!(
        outcome(&results, "seo.orphan_pages").status,
        CheckStatus::Skipped
    );
    assert_eq!(
        outcome(&results, "seo.noindex_in_sitemap").status,
        CheckStatus::Skipped
    );
}

#[test]
fn a_supplied_sitemap_with_no_contradiction_reports_pass() {
    let mut a = page("https://a.com/1");
    a.title = Some("One".into());
    let b = page("https://a.com/2");

    let results = analyze_session(&[a, b], Some(&["https://a.com/1".to_string()]));

    assert_eq!(
        outcome(&results, "seo.noindex_in_sitemap").status,
        CheckStatus::Pass
    );
}
