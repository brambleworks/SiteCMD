use super::*;

// Page record for grading a known probe outcome under a fixed clock.
fn page(body: &str) -> PageContext {
    PageContext {
        evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .expect("static test time")
            .with_timezone(&chrono::Utc),
        url: url::Url::parse("https://example.com").unwrap(),
        response_headers: http::HeaderMap::new(),
        status_code: 200,
        body: body.to_string(),
        is_localhost: false,
        is_strict_localhost: false,
        http_version: Some("HTTP/2.0".to_string()),
        body_lower_cache: std::sync::OnceLock::new(),
    }
}

fn fetched(url: &str, xml: &str) -> SitemapProbe {
    let SitemapParse::WellFormed(document) = parse_sitemap_document(xml) else {
        panic!("test fixture must be a valid sitemap document");
    };
    SitemapProbe::Found(SitemapFetch::new(url, xml, &document))
}

fn missing() -> SitemapProbe {
    SitemapProbe::Missing {
        observations: vec![SitemapProbeObservation {
            url: "https://example.com/sitemap.xml".into(),
            outcome: "HTTP 404".into(),
        }],
    }
}

#[test]
fn found_urlset_passes_and_counts_entries() {
    let xml = r#"<?xml version="1.0"?><urlset>
        <url><loc>https://example.com/</loc></url>
        <url><loc>https://example.com/pricing</loc></url>
        <url><loc>https://example.com/docs</loc></url>
    </urlset>"#;
    let results = evaluate_sitemap(
        &page(""),
        &RobotsTxtFetch::Status(404),
        &fetched("https://example.com/sitemap.xml", xml),
    );
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert_eq!(results[0].raw_data.as_ref().unwrap()["entries"], 3);
}

#[test]
fn sitemap_index_entries_are_counted_too() {
    let xml = r#"<sitemapindex>
        <sitemap><loc>https://example.com/sitemap-pages.xml</loc></sitemap>
        <sitemap><loc>https://example.com/sitemap-posts.xml</loc></sitemap>
    </sitemapindex>"#;
    let results = evaluate_sitemap(
        &page(""),
        &RobotsTxtFetch::Status(404),
        &fetched("https://example.com/sitemap_index.xml", xml),
    );
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert_eq!(results[0].raw_data.as_ref().unwrap()["entries"], 2);
}

#[test]
fn missing_sitemap_fails_with_a_framework_specific_fix() {
    let body = r#"<html><body><img src="/wp-content/uploads/hero.jpg"></body></html>"#;
    let results = evaluate_sitemap(&page(body), &RobotsTxtFetch::Status(404), &missing());
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].severity, Severity::Low);
    assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["stack_hint"],
        "WordPress"
    );
    assert!(results[0]
        .manual_fix
        .as_ref()
        .unwrap()
        .contains("wp-sitemap"));
}

#[test]
fn cross_origin_robots_sitemap_is_named_and_hedged() {
    let robots = RobotsTxtFetch::Found {
        body: "User-agent: *\nDisallow:\nSitemap: https://cdn.example-assets.net/sitemap.xml\n"
            .into(),
    };
    let results = evaluate_sitemap(&page(""), &robots, &missing());
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].severity, Severity::Low);
    assert_eq!(
        results[0].title,
        "Cross-origin sitemap declaration not verified"
    );
    assert!(
        results[0]
            .description
            .contains("https://cdn.example-assets.net/sitemap.xml"),
        "cross-origin sitemap path must remain actionable while query/fragment secrets are removed: {}",
        results[0].description
    );
    assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["cross_origin_declared"][0],
        "https://cdn.example-assets.net/sitemap.xml"
    );
    assert!(results[0]
        .description
        .contains("not evidence that the cross-origin sitemap is absent"));
    assert!(results[0]
        .manual_fix
        .as_deref()
        .unwrap_or_default()
        .contains("no duplicate sitemap is needed"));
}

#[test]
fn inconclusive_probe_does_not_become_a_missing_sitemap() {
    let probe = SitemapProbe::Inconclusive {
        observations: vec![SitemapProbeObservation {
            url: "https://example.com/sitemap.xml".into(),
            outcome: "HTTP 503".into(),
        }],
    };
    let results = evaluate_sitemap(&page(""), &RobotsTxtFetch::Status(404), &probe);
    // The probe established nothing, so the row is skipped, not a warning the
    // operator is asked to clear.
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert_eq!(results[0].severity, Severity::Low);
    assert_eq!(results[0].title, "Sitemap probe did not complete");
    assert!(results[0].description.contains("does not prove"));
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["probe_outcome"],
        "inconclusive"
    );
}

#[test]
fn relative_robots_declaration_is_reported_without_leaking_its_query() {
    let robots = RobotsTxtFetch::Found {
        body: "User-agent: *\nSitemap: /private-map.xml?token=secret#fragment\n".into(),
    };
    let result = &evaluate_sitemap(&page(""), &robots, &missing())[0];
    assert_eq!(result.title, "Sitemap declaration needs review");
    assert_eq!(
        result.raw_data.as_ref().unwrap()["invalid_declaration_count"],
        1
    );
    let evidence = result.raw_data.as_ref().unwrap()["robots_declared"].to_string();
    assert!(evidence.contains("private-map.xml"));
    assert!(evidence.contains("relative as declared"));
    assert!(!evidence.contains("secret"));
    assert!(!evidence.contains("fragment"));
}

#[test]
fn excessive_same_origin_declarations_are_disclosed_as_a_bounded_sample() {
    let body = (0..=SITEMAP_DECLARATION_PROBE_LIMIT)
        .map(|index| format!("Sitemap: https://example.com/map-{index}.xml"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = &evaluate_sitemap(&page(""), &RobotsTxtFetch::Found { body }, &missing())[0];
    assert_eq!(result.title, "Sitemap declaration sample incomplete");
    assert_eq!(
        result.raw_data.as_ref().unwrap()["same_origin_probe_truncated"],
        true
    );
    assert!(result
        .description
        .contains("remaining declared candidates were not requested"));
}

#[test]
fn empty_sitemap_warns_instead_of_passing() {
    let results = evaluate_sitemap(
        &page(""),
        &RobotsTxtFetch::Status(404),
        &fetched("https://example.com/sitemap.xml", "<urlset />"),
    );
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].severity, Severity::Low);
    assert!(results[0].title.contains("no entries"));
}

#[test]
fn sitemap_parser_rejects_malformed_or_loc_less_documents() {
    assert!(
        sitemap_document_summary("<urlset><url><loc>https://example.com/</url></urlset>").is_err()
    );
    assert!(sitemap_document_summary("<urlset><url /></urlset>").is_err());
    assert!(sitemap_document_summary(
        "<html><urlset><url><loc>https://example.com/</loc></url></urlset></html>"
    )
    .is_err());
}

#[test]
fn sitemap_parser_accepts_namespaced_entries_with_nonempty_locations() {
    let summary = sitemap_document_summary(
        r#"<sm:urlset xmlns:sm="http://www.sitemaps.org/schemas/sitemap/0.9">
            <sm:url><sm:loc>https://example.com/</sm:loc></sm:url>
        </sm:urlset>"#,
    )
    .expect("namespaced sitemap");
    assert_eq!(summary, ("urlset", 1));
}

#[test]
fn stack_hint_detects_next_js_from_body_markers() {
    let page =
        page(r#"<div id="__next"></div><script src="/_next/static/chunks/main.js"></script>"#);
    assert_eq!(detect_stack_hint(&page), Some("Next.js"));
}

#[test]
fn stack_hint_is_none_for_an_unrecognized_stack() {
    let page = page("<html><body><h1>Widgets</h1></body></html>");
    assert_eq!(detect_stack_hint(&page), None);
    assert!(framework_specific_sitemap_fix(None).contains("robots.txt"));
}

#[test]
fn origin_with_port_preserves_non_default_localhost_ports() {
    let url = url::Url::parse("http://localhost:8080/deep/page").unwrap();
    assert_eq!(origin_with_port(&url), "http://localhost:8080");
}

#[test]
fn origin_with_port_keeps_https_origin_stable() {
    let url = url::Url::parse("https://example.com/path?q=1").unwrap();
    assert_eq!(origin_with_port(&url), "https://example.com");
}

#[test]
fn sitemap_candidate_urls_include_hyphenated_index_paths() {
    let urls = sitemap_candidate_urls("https://example.com");
    assert!(urls.contains(&"https://example.com/sitemap-index.xml".to_string()));
    assert!(urls.contains(&"https://example.com/sitemap_index.xml".to_string()));
}

#[test]
fn sitemap_candidate_urls_include_wordpress_default() {
    let urls = sitemap_candidate_urls("https://example.com");
    assert!(urls.contains(&"https://example.com/wp-sitemap.xml".to_string()));
}

#[test]
fn sitemap_urls_parsed_from_robots_directives() {
    let robots = "User-agent: *\nDisallow:\nSitemap: https://example.com/a.xml\n# Sitemap: https://example.com/commented.xml\nSITEMAP: https://example.com/b.xml\nSitemap:\n";
    assert_eq!(
        sitemap_urls_from_robots(robots),
        vec![
            "https://example.com/a.xml".to_string(),
            "https://example.com/b.xml".to_string(),
        ]
    );
}

#[test]
fn same_origin_test_rejects_other_hosts_and_unparseable_urls() {
    assert!(url_is_same_origin(
        "https://example.com/sitemap.xml",
        "https://example.com"
    ));
    assert!(!url_is_same_origin(
        "https://cdn.example.net/sitemap.xml",
        "https://example.com"
    ));
    assert!(!url_is_same_origin("/sitemap.xml", "https://example.com"));
}
