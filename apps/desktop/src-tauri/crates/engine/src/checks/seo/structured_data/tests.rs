use super::StructuredDataCheck;
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, Severity};
use http::header::HeaderMap;

fn ctx(body: &str) -> PageContext {
    PageContext {
        evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        url: url::Url::parse("https://example.com").unwrap(),
        response_headers: HeaderMap::new(),
        status_code: 200,
        body: body.to_string(),
        is_localhost: false,
        is_strict_localhost: false,
        http_version: Some("HTTP/2.0".to_string()),
        body_lower_cache: std::sync::OnceLock::new(),
    }
}

fn run(body: &str) -> Vec<CheckResult> {
    StructuredDataCheck.run(&ctx(body))
}

fn run_json_ld(block: &str) -> Vec<CheckResult> {
    run(&page_with_json_ld(block))
}

fn page_with_json_ld(block: &str) -> String {
    format!(
        "<html><head><script type=\"application/ld+json\">{}</script></head><body></body></html>",
        block
    )
}

fn find<'a>(results: &'a [CheckResult], id: &str) -> Option<&'a CheckResult> {
    results.iter().find(|r| r.check_id == id)
}

const INVALID_ID: &str = "seo.structured_data.invalid";
const INCOMPLETE_ID: &str = "seo.structured_data.incomplete";

#[test]
fn missing_structured_data_warns() {
    let results = run("<html><body><p>plain page</p></body></html>");
    assert_eq!(results.len(), 1);
    let presence = &results[0];
    assert_eq!(presence.check_id, "seo.structured_data");
    assert_eq!(presence.status, CheckStatus::Warn);
    assert_eq!(presence.severity, Severity::Low);
    assert_eq!(
        presence.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(presence.manual_fix.is_some());
}

#[test]
fn source_code_strings_do_not_count_as_structured_data() {
    let results = run(r#"<html><body>
        <script>const examples = ['itemscope', 'typeof="FAQPage"', 'property="name"', 'application/ld+json'];</script>
        <pre>&lt;div itemscope&gt;</pre>
        </body></html>"#);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].raw_data.as_ref().unwrap()["json_ld"], false);
    assert_eq!(results[0].raw_data.as_ref().unwrap()["microdata"], false);
    assert_eq!(results[0].raw_data.as_ref().unwrap()["rdfa"], false);
}

#[test]
fn script_prefix_inside_json_string_is_not_treated_as_a_closing_tag() {
    let results = run(r#"<html><head>
        <script type="application/ld+json">{
          "@context": "https://schema.org",
          "@type": "WebSite",
          "name": "Example </scripture reference>",
          "url": "https://example.com"
        }</script>
        </head><body></body></html>"#);

    assert!(find(&results, INVALID_ID).is_none());
    assert_eq!(
        find(&results, "seo.structured_data").unwrap().status,
        CheckStatus::Pass
    );
}

#[test]
fn real_unquoted_microdata_attributes_count() {
    let results = run(
        r#"<html><body><div itemscope itemtype=https://schema.org/Person></div></body></html>"#,
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn json_ld_text_in_an_unrelated_script_attribute_does_not_count() {
    let results = run(
        r#"<script type="text/plain" data-note="application/ld+json">{"@type":"WebSite"}</script>"#,
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, CheckStatus::Warn);
}

#[test]
fn microdata_only_passes_without_fanout_results() {
    let results = run(
        "<html><body><div itemscope itemtype=\"https://schema.org/Person\"></div></body></html>",
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].description.contains("Microdata"));
}

#[test]
fn fully_valid_article_mentions_validated_types() {
    let results = run_json_ld(
        r#"{"@context":"https://schema.org","@type":"Article","headline":"Title","author":{"@type":"Person","name":"A"},"datePublished":"2026-01-01","image":"https://example.com/a.jpg"}"#,
    );
    assert_eq!(results.len(), 1);
    let presence = &results[0];
    assert_eq!(presence.status, CheckStatus::Pass);
    assert!(presence.description.contains("recognized types: Article"));
    let raw = presence.raw_data.as_ref().unwrap();
    assert_eq!(raw["validated_types"][0], "Article");
}

#[test]
fn invalid_json_ld_block_reports_parse_position() {
    let results = run_json_ld(r#"{"@type": "Article",}"#);
    let invalid = find(&results, INVALID_ID).expect("invalid result");
    assert_eq!(invalid.status, CheckStatus::Warn);
    assert_eq!(invalid.severity, Severity::Medium);
    assert!(invalid.description.contains("Block 1"));
    assert!(invalid.description.contains("line 1"));
    let raw = invalid.raw_data.as_ref().unwrap();
    assert_eq!(raw["invalid_blocks"][0]["index"], 0);
    assert_eq!(raw["invalid_blocks"][0]["line"], 1);
    // Presence still passes: the format is present, just broken.
    assert_eq!(
        find(&results, "seo.structured_data").unwrap().status,
        CheckStatus::Pass
    );
}

#[test]
fn empty_json_ld_block_is_invalid() {
    let results = run_json_ld("");
    assert!(find(&results, INVALID_ID).is_some());
}

#[test]
fn mixed_valid_and_invalid_blocks_flag_only_the_broken_block() {
    let body = format!(
        "<html><head>{}{}</head></html>",
        "<script type=\"application/ld+json\">{\"@type\":\"WebSite\",\"name\":\"Site\",\"url\":\"https://example.com\"}</script>",
        "<script type=\"application/ld+json\">{not json}</script>",
    );
    let results = run(&body);
    let invalid = find(&results, INVALID_ID).expect("invalid result");
    assert!(invalid.description.contains("1 of 2"));
    assert!(invalid.description.contains("Block 2"));
    // The valid WebSite block still validates fully.
    assert!(find(&results, INCOMPLETE_ID).is_none());
}

#[test]
fn top_level_array_nodes_are_each_validated() {
    let results = run_json_ld(
        r#"[{"@type":"WebSite","name":"Site","url":"https://example.com"},{"@type":"Product"}]"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("Product profile is missing name"));
}

#[test]
fn graph_nodes_are_each_validated() {
    let results = run_json_ld(
        r#"{"@context":"https://schema.org","@graph":[{"@type":"Article"},{"@type":"WebSite","name":"Site","url":"https://example.com"}]}"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(
        incomplete
            .description
            .contains("Article profile recommends author")
            && incomplete.description.contains("headline"),
        "{}",
        incomplete.description
    );
    assert!(!incomplete
        .description
        .contains("Article profile is missing"));
    assert_eq!(incomplete.severity, Severity::Low);
    assert_eq!(
        incomplete.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
}

#[test]
fn type_array_matches_any_recognized_type() {
    let results = run_json_ld(r#"{"@type":["Thing","Product"],"image":"x.jpg"}"#);
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("Product profile is missing name"));
}

#[test]
fn product_offer_currency_is_recommended_for_product_snippets() {
    let results = run_json_ld(
        r#"{"@type":"Product","name":"Widget","image":"x.jpg","aggregateRating":{"ratingValue":4.5},"offers":[{"@type":"Offer","price":"10.00"}]}"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert_eq!(incomplete.severity, Severity::Low);
    assert!(incomplete
        .description
        .contains("Product profile recommends offers[0].priceCurrency"));
    assert!(!incomplete.description.contains("offers[0].price,"));
    let raw = incomplete.raw_data.as_ref().unwrap();
    assert_eq!(
        raw["types"]["Product"]["missing_recommended_for_profile"][0],
        "offers[0].priceCurrency"
    );
}

#[test]
fn product_requires_a_review_rating_or_offer_for_the_snippet_profile() {
    let results = run_json_ld(
        r#"{"@type":"Product","name":"Widget","image":"https://example.com/widget.jpg"}"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("review, aggregateRating, or offers"));
}

#[test]
fn aggregate_offer_requires_currency_for_the_product_snippet_profile() {
    let results = run_json_ld(
        r#"{"@type":"Product","name":"Widget","image":"x.jpg","offers":{"@type":"AggregateOffer","lowPrice":"10.00"}}"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    let raw = incomplete.raw_data.as_ref().unwrap();
    assert!(raw["types"]["Product"]["missing_required_for_profile"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "offers.priceCurrency"));
}

#[test]
fn product_offer_object_with_price_specification_is_complete() {
    let results = run_json_ld(
        r#"{"@type":"Product","name":"Widget","image":"x.jpg","aggregateRating":{"ratingValue":4.5},"offers":{"@type":"Offer","priceSpecification":{"price":"10.00","priceCurrency":"USD"}}}"#,
    );
    assert!(find(&results, INCOMPLETE_ID).is_none());
    assert!(find(&results, INVALID_ID).is_none());
}

#[test]
fn product_without_offers_is_not_flagged_for_offer_fields() {
    let results = run_json_ld(
        r#"{"@type":"Product","name":"Widget","image":"x.jpg","aggregateRating":{"ratingValue":4.5}}"#,
    );
    assert!(find(&results, INCOMPLETE_ID).is_none());
}

#[test]
fn article_missing_recommended_only_is_low_severity() {
    let results = run_json_ld(r#"{"@type":"BlogPosting","headline":"Title"}"#);
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert_eq!(incomplete.severity, Severity::Low);
    assert!(incomplete.description.contains("recommended"));
    assert!(incomplete.description.contains("author"));
    assert!(incomplete.description.contains("datePublished"));
    // Recommended-only gaps do not use required-gap phrasing.
    assert!(!incomplete
        .description
        .contains("BlogPosting profile is missing"));
}

#[test]
fn required_gaps_across_types_aggregate_into_one_result() {
    let results =
        run_json_ld(r#"[{"@type":"Product","image":"x.jpg"},{"@type":"Article","headline":"T"}]"#);
    let matches: Vec<_> = results
        .iter()
        .filter(|r| r.check_id == INCOMPLETE_ID)
        .collect();
    assert_eq!(matches.len(), 1);
    let incomplete = matches[0];
    assert_eq!(incomplete.severity, Severity::Low);
    assert!(incomplete
        .description
        .contains("Product profile is missing"));
    assert!(incomplete
        .description
        .contains("Additional profile recommendations"));
    assert!(incomplete
        .description
        .contains("Article profile recommends author"));
}

#[test]
fn breadcrumb_requires_item_list_element() {
    let results = run_json_ld(r#"{"@type":"BreadcrumbList"}"#);
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("BreadcrumbList profile is missing itemListElement"));
}

#[test]
fn breadcrumb_elements_need_position_name_and_nonfinal_item() {
    let results = run_json_ld(
        r#"{"@type":"BreadcrumbList","itemListElement":[{"@type":"ListItem","item":"https://example.com"},{"@type":"ListItem","position":2}]}"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("itemListElement[0].position"));
    assert!(incomplete.description.contains("itemListElement[0].name"));
    assert!(incomplete.description.contains("itemListElement[1].name"));
}

#[test]
fn breadcrumb_last_item_may_omit_item_but_not_name() {
    let results = run_json_ld(
        r#"{"@type":"BreadcrumbList","itemListElement":[{"@type":"ListItem","position":1,"name":"Home","item":"https://example.com/"},{"@type":"ListItem","position":2,"name":"Current"}]}"#,
    );
    assert!(find(&results, INCOMPLETE_ID).is_none());
}

#[test]
fn breadcrumb_profile_requires_at_least_two_items() {
    let results = run_json_ld(
        r#"{"@type":"BreadcrumbList","itemListElement":[{"@type":"ListItem","position":1,"name":"Current"}]}"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete.description.contains("at least two entries"));
}

#[test]
fn local_business_subtype_requires_name_and_address() {
    let results = run_json_ld(r#"{"@type":"MedicalBusiness","name":"Clinic"}"#);
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("MedicalBusiness profile is missing address"));
}

#[test]
fn restaurant_uses_the_local_business_profile() {
    let results = run_json_ld(r#"{"@type":"Restaurant","name":"Cafe"}"#);
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("Restaurant profile is missing address"));
}

#[test]
fn unknown_type_ending_in_business_is_not_misclassified() {
    let results = run_json_ld(r#"{"@type":"ImaginaryBusiness","name":"Example"}"#);
    assert!(find(&results, INCOMPLETE_ID).is_none());
}

#[test]
fn website_requires_name_and_url() {
    let results = run_json_ld(r#"{"@type":"WebSite","name":"Site"}"#);
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("WebSite profile is missing url"));
}

#[test]
fn unrecognized_types_are_ignored_and_counted() {
    let results = run_json_ld(r#"{"@type":"VideoObject","name":"Clip"}"#);
    assert!(find(&results, INCOMPLETE_ID).is_none());
    assert!(find(&results, INVALID_ID).is_none());
    let presence = find(&results, "seo.structured_data").unwrap();
    let raw = presence.raw_data.as_ref().unwrap();
    assert_eq!(raw["unvalidated_types"][0], "VideoObject");
}

#[test]
fn organization_properties_are_recommendations_not_requirements() {
    let results = run_json_ld(
        r#"{"@type":"Organization","url":"https://example.com","logo":"https://example.com/logo.png"}"#,
    );
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("Organization profile recommends name (or alternateName)"));
    assert!(!incomplete.description.contains("profile is missing"));
    for result in &results {
        assert!(!result.description.contains("sameAs"));
    }
}

#[test]
fn complete_organization_is_not_flagged_despite_missing_same_as() {
    let results = run_json_ld(
        r#"{"@type":"Organization","name":"Acme","url":"https://example.com","logo":"https://example.com/logo.png"}"#,
    );
    assert!(find(&results, INCOMPLETE_ID).is_none());
}

#[test]
fn faq_page_is_not_checked_against_a_removed_google_feature_profile() {
    let results = run_json_ld(
        r#"{"@type":"FAQPage","mainEntity":[{"@type":"Question","name":"What is it?"}]}"#,
    );
    assert!(find(&results, INCOMPLETE_ID).is_none());
    let presence = find(&results, "seo.structured_data").unwrap();
    assert_eq!(
        presence.raw_data.as_ref().unwrap()["unvalidated_types"][0],
        "FAQPage"
    );
}

#[test]
fn empty_string_properties_count_as_missing() {
    let results = run_json_ld(r#"{"@type":"WebSite","name":"","url":"https://example.com"}"#);
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    assert!(incomplete
        .description
        .contains("WebSite profile is missing name"));
}

#[test]
fn failing_results_carry_guidance_fields() {
    let body = format!(
        "<html><head>{}{}</head></html>",
        "<script type=\"application/ld+json\">{broken</script>",
        "<script type=\"application/ld+json\">{\"@type\":\"Product\"}</script>",
    );
    let results = run(&body);
    let invalid = find(&results, INVALID_ID).expect("invalid result");
    let incomplete = find(&results, INCOMPLETE_ID).expect("incomplete result");
    for result in [invalid, incomplete] {
        assert!(!result.description.is_empty());
        assert!(result.manual_fix.is_some());
        assert!(result.why_it_matters.is_some());
        assert!(result.raw_data.is_some());
    }
    assert_eq!(invalid.confidence, crate::checks::IssueConfidence::High);
    assert_eq!(
        incomplete.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
}
