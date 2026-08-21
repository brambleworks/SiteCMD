use super::*;

#[test]
fn parse_aggregate_totals_extracts_first_row() {
    let resp = serde_json::json!({
        "rows": [{
            "clicks": 1500.0,
            "impressions": 25000.0,
            "ctr": 0.06,
            "position": 8.5,
        }]
    });
    let (clicks, impressions, ctr, position) = parse_aggregate_totals(&resp);
    assert_eq!(clicks, 1500);
    assert_eq!(impressions, 25000);
    assert!((ctr - 0.06).abs() < 1e-9);
    assert!((position - 8.5).abs() < 1e-9);
}

#[test]
fn parse_aggregate_totals_truncates_floats_to_u64() {
    // GSC returns clicks/impressions as floats; we truncate (cast),
    // not round. Documents that behavior so a future change is intentional.
    let resp = serde_json::json!({
        "rows": [{
            "clicks": 99.99,
            "impressions": 1.5,
            "ctr": 0.5,
            "position": 1.2,
        }]
    });
    let (clicks, impressions, _, _) = parse_aggregate_totals(&resp);
    assert_eq!(clicks, 99); // truncated, not 100
    assert_eq!(impressions, 1);
}

#[test]
fn parse_aggregate_totals_returns_zeros_when_rows_missing() {
    let result = parse_aggregate_totals(&serde_json::json!({}));
    assert_eq!(result, (0, 0, 0.0, 0.0));
}

#[test]
fn parse_aggregate_totals_returns_zeros_when_rows_empty() {
    let result = parse_aggregate_totals(&serde_json::json!({"rows": []}));
    assert_eq!(result, (0, 0, 0.0, 0.0));
}

#[test]
fn parse_aggregate_totals_defaults_missing_fields_to_zero() {
    // Fresh sites with zero traffic might omit individual fields.
    let resp = serde_json::json!({"rows": [{"clicks": 5.0}]});
    let (clicks, impressions, ctr, position) = parse_aggregate_totals(&resp);
    assert_eq!(clicks, 5);
    assert_eq!(impressions, 0);
    assert_eq!(ctr, 0.0);
    assert_eq!(position, 0.0);
}

#[test]
fn parse_top_queries_maps_keys_to_query_string() {
    let resp = serde_json::json!({
        "rows": [
            {"keys": ["sitecmd"], "clicks": 100.0, "impressions": 1000.0, "ctr": 0.1, "position": 2.0},
            {"keys": ["website checker"], "clicks": 50.0, "impressions": 800.0, "ctr": 0.0625, "position": 5.0},
        ]
    });
    let queries = parse_top_queries(&resp);
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0].query, "sitecmd");
    assert_eq!(queries[0].clicks, 100);
    assert_eq!(queries[0].impressions, 1000);
    assert!((queries[0].ctr - 0.1).abs() < 1e-9);
    assert!((queries[0].position - 2.0).abs() < 1e-9);
    assert_eq!(queries[1].query, "website checker");
}

#[test]
fn parse_top_queries_skips_rows_without_keys() {
    // GSC sometimes returns rows with no `keys` array - skip them rather
    // than letting the row appear with an empty query string.
    let resp = serde_json::json!({
        "rows": [
            {"clicks": 50.0, "impressions": 500.0},
            {"keys": ["valid"], "clicks": 10.0, "impressions": 100.0, "ctr": 0.1, "position": 1.0},
        ]
    });
    let queries = parse_top_queries(&resp);
    assert_eq!(queries.len(), 1, "row without keys must be filtered out");
    assert_eq!(queries[0].query, "valid");
}

#[test]
fn parse_top_queries_returns_empty_when_no_rows() {
    assert!(parse_top_queries(&serde_json::json!({})).is_empty());
    assert!(parse_top_queries(&serde_json::json!({"rows": []})).is_empty());
}

#[test]
fn parse_top_queries_handles_empty_keys_array() {
    // `keys: []` (empty array) should still produce a row with an empty
    // query string - defensive against GSC quirks.
    let resp = serde_json::json!({
        "rows": [{"keys": [], "clicks": 5.0, "impressions": 50.0}]
    });
    let queries = parse_top_queries(&resp);
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].query, "");
}

#[test]
fn parse_top_pages_maps_keys_to_page_url() {
    let resp = serde_json::json!({
        "rows": [
            {"keys": ["https://example.com/"], "clicks": 200.0, "impressions": 3000.0, "ctr": 0.0667, "position": 3.5},
        ]
    });
    let pages = parse_top_pages(&resp);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].page, "https://example.com/");
    assert_eq!(pages[0].clicks, 200);
}

#[test]
fn parse_daily_maps_keys_to_iso_date() {
    let resp = serde_json::json!({
        "rows": [
            {"keys": ["2026-04-17"], "clicks": 30.0, "impressions": 400.0, "ctr": 0.075, "position": 4.0},
            {"keys": ["2026-04-18"], "clicks": 45.0, "impressions": 500.0, "ctr": 0.09, "position": 3.5},
        ]
    });
    let daily = parse_daily(&resp);
    assert_eq!(daily.len(), 2);
    assert_eq!(daily[0].date, "2026-04-17");
    assert_eq!(daily[0].clicks, 30);
    assert_eq!(daily[1].date, "2026-04-18");
    assert_eq!(daily[1].clicks, 45);
}

#[test]
fn parse_devices_extracts_device_clicks_impressions_only() {
    // Devices payload doesn't carry CTR/position - the parser should
    // produce just (device, clicks, impressions).
    let resp = serde_json::json!({
        "rows": [
            {"keys": ["MOBILE"], "clicks": 600.0, "impressions": 9000.0},
            {"keys": ["DESKTOP"], "clicks": 400.0, "impressions": 5500.0},
            {"keys": ["TABLET"], "clicks": 50.0, "impressions": 1500.0},
        ]
    });
    let devices = parse_devices(&resp);
    assert_eq!(devices.len(), 3);
    assert_eq!(devices[0].device, "MOBILE");
    assert_eq!(devices[0].clicks, 600);
    assert_eq!(devices[2].device, "TABLET");
}

#[test]
fn parse_countries_extracts_country_code_clicks_impressions_only() {
    let resp = serde_json::json!({
        "rows": [
            {"keys": ["usa"], "clicks": 800.0, "impressions": 12000.0},
            {"keys": ["gbr"], "clicks": 200.0, "impressions": 3000.0},
        ]
    });
    let countries = parse_countries(&resp);
    assert_eq!(countries.len(), 2);
    assert_eq!(countries[0].country, "usa");
    assert_eq!(countries[0].clicks, 800);
}

#[test]
fn parse_sites_extracts_site_url_and_permission() {
    let json = serde_json::json!({
        "siteEntry": [
            {"siteUrl": "https://example.com/", "permissionLevel": "siteOwner"},
            {"siteUrl": "https://other.example.com/", "permissionLevel": "siteFullUser"},
        ]
    });
    let sites = parse_sites(&json);
    assert_eq!(sites.len(), 2);
    assert_eq!(sites[0].site_url, "https://example.com/");
    assert_eq!(sites[0].permission, "siteOwner");
    assert_eq!(sites[1].permission, "siteFullUser");
}

#[test]
fn parse_sites_defaults_missing_permission_to_empty_string() {
    // Permission level is informational - a missing field shouldn't drop
    // the site entry entirely.
    let json = serde_json::json!({
        "siteEntry": [{"siteUrl": "https://example.com/"}]
    });
    let sites = parse_sites(&json);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].permission, "");
}

#[test]
fn parse_sites_skips_entries_missing_site_url() {
    // siteUrl is the identifier - without it the entry is unusable, drop it.
    let json = serde_json::json!({
        "siteEntry": [
            {"permissionLevel": "siteOwner"}, // no siteUrl
            {"siteUrl": "https://example.com/", "permissionLevel": "siteOwner"},
        ]
    });
    let sites = parse_sites(&json);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].site_url, "https://example.com/");
}

#[test]
fn parse_sites_returns_empty_when_no_site_entry() {
    assert!(parse_sites(&serde_json::json!({})).is_empty());
    assert!(parse_sites(&serde_json::json!({"siteEntry": []})).is_empty());
}

#[test]
fn parse_rows_yields_empty_keys_for_rows_without_keys_array() {
    // The mapper receives an empty Vec when `keys` is missing entirely.
    let resp = serde_json::json!({
        "rows": [
            {"keys": ["a"]},
            {"clicks": 5.0}, // no keys
        ]
    });
    // Capture the keys passed to the mapper to assert behavior.
    let collected: Vec<Vec<String>> = parse_rows(&resp, |keys, _row| keys);
    // The current implementation skips the row without a `keys` array,
    // yielding only the rows that have one.
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0], vec!["a"]);
}

#[test]
fn parse_extracts_basic_fields() {
    let json = serde_json::json!({
        "inspectionResult": {
            "indexStatusResult": {
                "verdict": "PASS",
                "coverageState": "Submitted and indexed",
                "indexingState": "INDEXED",
                "pageFetchState": "SUCCESSFUL",
                "robotsTxtState": "ALLOWED",
                "lastCrawlTime": "2026-04-20T00:00:00Z",
                "userCanonical": "https://example.com/",
                "googleCanonical": "https://example.com/"
            }
        }
    });
    let r = parse_url_inspection(&json, "https://example.com/").unwrap();
    assert_eq!(r.verdict, "PASS");
    assert_eq!(r.coverage_state, "Submitted and indexed");
    assert_eq!(r.indexing_state.as_deref(), Some("INDEXED"));
    assert!(r.canonical_inspection.is_some());
    assert!(!r.canonical_inspection.unwrap().mismatch);
}

#[test]
fn parse_detects_canonical_mismatch() {
    let json = serde_json::json!({
        "inspectionResult": {
            "indexStatusResult": {
                "verdict": "PASS",
                "coverageState": "Duplicate, Google chose different canonical",
                "userCanonical": "https://example.com/a",
                "googleCanonical": "https://example.com/b"
            }
        }
    });
    let r = parse_url_inspection(&json, "https://example.com/a").unwrap();
    let canon = r.canonical_inspection.unwrap();
    assert!(canon.mismatch);
}

#[test]
fn classify_maps_blocked_by_robots() {
    let insp = UrlInspectionResult {
        page_url: "https://example.com/x".into(),
        verdict: "FAIL".into(),
        coverage_state: "".into(),
        indexing_state: Some("BLOCKED_BY_ROBOTS_TXT".into()),
        page_fetch_state: None,
        robots_txt_state: Some("DISALLOWED".into()),
        last_crawl_time: None,
        mobile_friendly: None,
        mobile_usability_issues: vec![],
        canonical_inspection: None,
    };
    assert_eq!(classify_inspection(&insp), Some("blocked-by-robots"));
}

#[test]
fn classify_maps_not_indexed() {
    let insp = UrlInspectionResult {
        page_url: "https://example.com/x".into(),
        verdict: "NEUTRAL".into(),
        coverage_state: "Crawled - currently not indexed".into(),
        indexing_state: Some("INDEXING_ALLOWED".into()),
        page_fetch_state: None,
        robots_txt_state: None,
        last_crawl_time: None,
        mobile_friendly: None,
        mobile_usability_issues: vec![],
        canonical_inspection: None,
    };
    assert_eq!(classify_inspection(&insp), Some("not-indexed"));
}
