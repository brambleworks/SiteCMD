use super::*;

fn entry(url: &str) -> url::Url {
    url::Url::parse(url).expect("fixture url")
}

fn scope_of(selected: &[&str]) -> ScanScope {
    build_scope(
        &entry("https://example.com/"),
        &selected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        vec!["seo".into()],
        None,
    )
    .expect("scope builds")
}

#[test]
fn the_entry_route_is_always_in_scope() {
    // Origin-scoped checks require the entry page, so a scope without it
    // would make every origin check permanently uncoverable.
    let scope = scope_of(&["/pricing"]);
    assert_eq!(scope.entry_route, "/");
    assert_eq!(
        scope
            .routes
            .iter()
            .map(|r| r.route.as_str())
            .collect::<Vec<_>>(),
        vec!["/", "/pricing"]
    );
}

#[test]
fn an_entry_route_selected_again_is_counted_once() {
    let scope = scope_of(&["/", "/pricing", "/"]);
    assert_eq!(scope.len(), 2);
}

#[test]
fn selections_are_canonicalized_before_they_are_stored() {
    // A route typed by hand and one discovered from a URL must not become
    // two entries that name the same page.
    let scope = scope_of(&["pricing", "/docs/../guides", "/product?id=1"]);
    assert_eq!(
        scope
            .routes
            .iter()
            .map(|r| r.route.as_str())
            .collect::<Vec<_>>(),
        vec!["/", "/pricing", "/guides", "/product"]
    );
}

#[test]
fn an_entry_url_with_a_path_keeps_that_path_as_its_route() {
    let scope =
        build_scope(&entry("https://example.com/app/"), &[], vec![], None).expect("scope builds");
    assert_eq!(scope.entry_route, "/app/");
    assert_eq!(scope.routes.len(), 1);
}

#[test]
fn a_scope_over_the_plan_cap_is_refused_by_name() {
    let selected: Vec<String> = (0..5).map(|n| format!("/page-{n}")).collect();
    let error = build_scope(&entry("https://example.com/"), &selected, vec![], Some(4))
        .expect_err("cap refuses");
    assert_eq!(
        error,
        ScopeError::ExceedsPlan {
            requested: 6,
            cap: 4
        }
    );
    assert!(error.message().contains('4'), "{}", error.message());
    assert!(error.message().contains('6'), "{}", error.message());
}

#[test]
fn a_scope_at_the_cap_is_allowed() {
    let selected: Vec<String> = (0..3).map(|n| format!("/page-{n}")).collect();
    let scope = build_scope(&entry("https://example.com/"), &selected, vec![], Some(4))
        .expect("a scope at the cap fits");
    assert_eq!(scope.len(), 4);
}

#[test]
fn the_wire_limit_bounds_the_resource_itself() {
    let selected: Vec<String> = (0..SCOPE_WIRE_LIMIT).map(|n| format!("/p{n}")).collect();
    let error = build_scope(&entry("https://example.com/"), &selected, vec![], None)
        .expect_err("the wire limit refuses");
    assert_eq!(
        error,
        ScopeError::ExceedsWireLimit {
            requested: SCOPE_WIRE_LIMIT + 1,
            limit: SCOPE_WIRE_LIMIT
        }
    );
}

#[test]
fn truncation_reserves_the_entry_route_and_is_otherwise_alphabetical() {
    // Lexicographic order alone would truncate "/" away last but could put
    // an entry like "/app/" anywhere, so the entry rank is explicit.
    let scope = build_scope(
        &entry("https://example.com/app/"),
        &["/zebra", "/alpha", "/middle"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        vec![],
        None,
    )
    .expect("scope builds");
    let (kept, overflow) = scope.effective_prefix(2);
    assert_eq!(
        kept.iter().map(|r| r.route.as_str()).collect::<Vec<_>>(),
        vec!["/app/", "/alpha"]
    );
    assert_eq!(
        overflow
            .iter()
            .map(|r| r.route.as_str())
            .collect::<Vec<_>>(),
        vec!["/middle", "/zebra"]
    );
}

#[test]
fn a_scope_under_the_cap_overflows_nothing() {
    let scope = scope_of(&["/pricing"]);
    let (kept, overflow) = scope.effective_prefix(HOSTED_SCOPE_CEILING);
    assert_eq!(kept.len(), 2);
    assert!(overflow.is_empty());
}

#[test]
fn check_families_come_from_the_capability_manifest() {
    let families = engine_check_families();
    // Derived, not a second hand-kept list: the manifest already decides
    // which checks exist and which lane can produce them.
    assert!(families.contains(&"security".to_string()));
    assert!(families.contains(&"accessibility".to_string()));
    assert!(families.windows(2).all(|pair| pair[0] < pair[1]));
    // A family whose every check is unsupported is not something a scan can
    // be expected to run.
    assert!(!families.iter().any(|family| family.is_empty()));
}

#[test]
fn routes_resolve_back_to_urls_against_the_environment() {
    let scope = scope_of(&["/pricing", "/a%2Fb"]);
    assert_eq!(
        scope_urls(&entry("https://example.com/"), &scope.routes),
        vec![
            "https://example.com/",
            "https://example.com/pricing",
            "https://example.com/a%2Fb",
        ]
    );
}
