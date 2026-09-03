use super::StylesheetCache;

#[test]
fn a_fresh_cache_reports_no_entry_for_an_unseen_url() {
    let cache = StylesheetCache::new();
    assert_eq!(cache.get("https://example.com/a.css"), None);
    assert!(cache.is_empty());
}

#[test]
fn recorded_outcomes_are_returned_verbatim_including_failures() {
    let cache = StylesheetCache::new();
    cache.insert("https://example.com/a.css", Some("body{}".into()));
    cache.insert("https://example.com/b.css", None);

    assert_eq!(
        cache.get("https://example.com/a.css"),
        Some(Some("body{}".to_string()))
    );
    assert_eq!(cache.get("https://example.com/b.css"), Some(None));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.bytes(), "body{}".len());
}

#[test]
fn the_first_outcome_for_a_url_wins() {
    let cache = StylesheetCache::new();
    cache.insert("https://example.com/a.css", Some("first".into()));
    cache.insert("https://example.com/a.css", Some("second".into()));

    assert_eq!(
        cache.get("https://example.com/a.css"),
        Some(Some("first".to_string()))
    );
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.bytes(), "first".len());
}

#[test]
fn the_byte_budget_evicts_the_oldest_stylesheets() {
    let cache = StylesheetCache::with_limits(64, 20);
    cache.insert("https://example.com/a.css", Some("a".repeat(10)));
    cache.insert("https://example.com/b.css", Some("b".repeat(10)));
    assert_eq!(cache.bytes(), 20);

    cache.insert("https://example.com/c.css", Some("c".repeat(10)));

    assert_eq!(
        cache.get("https://example.com/a.css"),
        None,
        "oldest evicted"
    );
    assert_eq!(
        cache.get("https://example.com/b.css"),
        Some(Some("b".repeat(10)))
    );
    assert_eq!(
        cache.get("https://example.com/c.css"),
        Some(Some("c".repeat(10)))
    );
    assert_eq!(cache.bytes(), 20);
    assert_eq!(cache.len(), 2);
}

#[test]
fn the_entry_cap_evicts_the_oldest_stylesheets() {
    let cache = StylesheetCache::with_limits(2, 1024 * 1024);
    cache.insert("https://example.com/a.css", Some("a".into()));
    cache.insert("https://example.com/b.css", Some("b".into()));
    cache.insert("https://example.com/c.css", Some("c".into()));

    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get("https://example.com/a.css"), None);
    assert!(cache.get("https://example.com/b.css").is_some());
    assert!(cache.get("https://example.com/c.css").is_some());
}

#[test]
fn a_stylesheet_larger_than_the_whole_budget_is_not_stored() {
    let cache = StylesheetCache::with_limits(64, 8);
    cache.insert("https://example.com/small.css", Some("1234".into()));
    cache.insert("https://example.com/huge.css", Some("x".repeat(9)));

    assert_eq!(
        cache.get("https://example.com/huge.css"),
        None,
        "an oversized body must not evict the whole cache to fail anyway"
    );
    assert_eq!(
        cache.get("https://example.com/small.css"),
        Some(Some("1234".to_string()))
    );
    assert_eq!(cache.len(), 1);
}

#[test]
fn failure_outcomes_never_consume_byte_budget() {
    let cache = StylesheetCache::with_limits(64, 4);
    cache.insert("https://example.com/a.css", None);
    cache.insert("https://example.com/b.css", None);
    cache.insert("https://example.com/c.css", Some("abcd".into()));

    assert_eq!(cache.len(), 3);
    assert_eq!(cache.bytes(), 4);
    assert_eq!(cache.get("https://example.com/a.css"), Some(None));
}
