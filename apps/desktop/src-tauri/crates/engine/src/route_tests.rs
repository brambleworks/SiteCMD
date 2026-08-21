use super::*;

fn route_of(url: &str) -> CanonicalRoute {
    canonical_route(&url::Url::parse(url).expect("fixture url"))
}

#[test]
fn a_route_is_a_path_not_a_url() {
    // The environment pins scheme and host, so carrying them in the route
    // would key identity on facts that belong to the site record.
    assert_eq!(route_of("https://example.com/pricing").route, "/pricing");
    assert_eq!(route_of("https://example.com").route, "/");
}

#[test]
fn trailing_slashes_are_distinct_routes() {
    assert_eq!(route_of("https://example.com/checkout").route, "/checkout");
    assert_eq!(
        route_of("https://example.com/checkout/").route,
        "/checkout/"
    );
}

#[test]
fn a_query_string_is_stripped_and_flagged() {
    let observed = route_of("https://example.com/product?id=1");
    assert_eq!(observed.route, "/product");
    // The stripped route cannot tell ?id=1 from ?id=2, so the merge is
    // recorded rather than hidden; verification refuses to compare these.
    assert!(observed.query_dependent);
    assert!(!route_of("https://example.com/product").query_dependent);
    // An empty query is not a variant of anything.
    assert!(!route_of("https://example.com/product?").query_dependent);
}

#[test]
fn a_fragment_never_reaches_the_route() {
    let observed = route_of("https://example.com/docs#install");
    assert_eq!(observed.route, "/docs");
    assert!(!observed.query_dependent);
}

#[test]
fn dot_segments_resolve_and_repeated_slashes_stay() {
    assert_eq!(canonical_path("/a/b/../c"), "/a/c");
    assert_eq!(canonical_path("/a/./b"), "/a/b");
    assert_eq!(canonical_path("/a/.."), "/");
    // Preserved: `//a` and `/a` can serve different content, and the rules
    // say what is assumed rather than what looks tidy.
    assert_eq!(canonical_path("//a"), "//a");
    assert_eq!(canonical_path("/a//b"), "/a//b");
}

#[test]
fn an_encoded_slash_is_never_decoded() {
    // Decoding it would turn one segment into two and merge two resources
    // whose paths only look alike after the rewrite.
    assert_eq!(canonical_path("/files/a%2Fb"), "/files/a%2Fb");
    assert_eq!(canonical_path("/files/a%2fb"), "/files/a%2Fb");
}

#[test]
fn percent_encoding_is_normalized_without_changing_what_it_means() {
    // Unreserved characters decode: %7E is a tilde in every reading.
    assert_eq!(canonical_path("/%7Euser"), "/~user");
    assert_eq!(canonical_path("/%61bc"), "/abc");
    // Everything else keeps its escape, with uppercase hex.
    assert_eq!(canonical_path("/a%20b"), "/a%20b");
    assert_eq!(canonical_path("/a%2bb"), "/a%2Bb");
    // Non-ASCII bytes stay as sent: no Unicode normalization, because two
    // byte sequences a human reads alike can still be two resources.
    assert_eq!(canonical_path("/caf%C3%A9"), "/caf%C3%A9");
    assert_eq!(canonical_path("/caf%c3%a9"), "/caf%C3%A9");
}

#[test]
fn a_malformed_escape_is_left_exactly_as_sent() {
    // Rewriting it would invent a route the server never served.
    assert_eq!(canonical_path("/100%"), "/100%");
    assert_eq!(canonical_path("/a%zz"), "/a%zz");
}

#[test]
fn path_case_is_preserved() {
    // Servers differ on whether case matters; assuming it does not would
    // merge two pages on the ones where it does.
    assert_eq!(canonical_path("/Docs/Install"), "/Docs/Install");
}

#[test]
fn an_authored_route_gains_its_leading_slash() {
    assert_eq!(canonical_path("pricing"), "/pricing");
    assert_eq!(canonical_path(""), "/");
    // Authored routes carry no query or fragment either.
    assert_eq!(canonical_path("/search?q=x#top"), "/search");
}

#[test]
fn canonicalizing_twice_changes_nothing() {
    // Storage round-trips routes, so a second pass has to be a no-op or
    // stored identity would drift from freshly observed identity.
    for path in [
        "/a/b/../c",
        "/files/a%2Fb",
        "/%7Euser",
        "//a",
        "/checkout/",
        "/caf%C3%A9",
        "/100%",
    ] {
        let once = canonical_path(path);
        assert_eq!(canonical_path(&once), once, "{path}");
    }
}

#[test]
fn the_canonicalizer_version_is_pinned() {
    assert_eq!(CANONICALIZER_VERSION, 1);
}
