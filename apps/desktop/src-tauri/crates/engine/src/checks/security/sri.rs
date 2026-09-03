//! Inspect cross-origin scripts and stylesheets for Subresource Integrity.
//! Mutable endpoints are excluded; other omissions require review because source alone
//! cannot prove immutability or CORS support.

use crate::checks::html_attrs::{attr_value, has_attr, tag_slices, url_attr_value};
use crate::checks::performance::third_party::{site_relation, SiteRelation};
use rolling_endpoints::sri_exclusion_reason;

mod rolling_endpoints;
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// How many offending URLs the evidence lists inline. `count` keeps the total,
/// and the rest of this lane caps example lists the same way: at 55 hashed
/// bundles from one build the list is not the actionable part, the build
/// setting is.
const MAX_LISTED_URLS: usize = 5;

/// Checks for external scripts/stylesheets loaded without integrity attributes
pub struct SubresourceIntegrityCheck;

impl Check for SubresourceIntegrityCheck {
    fn id(&self) -> &str {
        "security.sri"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut missing: Vec<String> = Vec::new();
        let mut rolling_excluded: Vec<serde_json::Value> = Vec::new();
        let mut own_site_excluded: Vec<serde_json::Value> = Vec::new();
        // Counted so the row can address the obvious objection when the
        // resources it surfaces sit on a host that looks like the site's own.
        let mut name_similar_missing = 0usize;
        let mut name_similar_sites: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        let lower = ctx.body_lower();

        // Scoped so the closure's borrow of the three evidence vectors ends
        // before they are read below.
        {
            let mut classify = |tag: &str, reference: &str| {
                if !is_cross_origin(reference, &ctx.url) {
                    return;
                }
                if let Some(reason) = own_site_exclusion_reason(reference, &ctx.url) {
                    own_site_excluded.push(serde_json::json!({
                        "url": evidence_url(reference),
                        "reason": reason,
                    }));
                } else if let Some(reason) = sri_exclusion_reason(reference) {
                    rolling_excluded.push(serde_json::json!({
                        "url": evidence_url(reference),
                        "reason": reason,
                    }));
                } else if !has_attr(tag, "integrity") {
                    if let Some(site) = name_similar_site(reference, &ctx.url) {
                        name_similar_missing += 1;
                        name_similar_sites.insert(site);
                    }
                    missing.push(evidence_url(reference));
                }
            };

            for tag in tag_slices(&ctx.body, lower, "script") {
                let Some(src) = url_attr_value(tag, "src") else {
                    continue;
                };
                classify(tag, &src);
            }

            for tag in tag_slices(&ctx.body, lower, "link") {
                let is_stylesheet = attr_value(tag, "rel").is_some_and(|rel| {
                    rel.split_whitespace()
                        .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                });
                if !is_stylesheet {
                    continue;
                }
                let Some(href) = url_attr_value(tag, "href") else {
                    continue;
                };
                classify(tag, &href);
            }
        }

        let count = missing.len();
        let rolling_note = if !rolling_excluded.is_empty() {
            format!(
                " {} resource{} matched a known dynamic or rolling vendor endpoint and {} excluded from the missing-integrity count because a fixed hash can break when its response bytes change. The exact URL{} and reason{} are preserved in the evidence.",
                rolling_excluded.len(),
                if rolling_excluded.len() == 1 { "" } else { "s" },
                if rolling_excluded.len() == 1 { "was" } else { "were" },
                if rolling_excluded.len() == 1 { "" } else { "s" },
                if rolling_excluded.len() == 1 { "" } else { "s" },
            )
        } else {
            String::new()
        };
        // The rows this check surfaces on a site's own delivery domain are its
        // best candidates, not noise: a content-hashed bundle is exactly what a
        // fixed hash can pin, and a compromised delivery path is the attack SRI
        // answers. What makes them read as noise is surfacing them with no
        // reply to the obvious objection, so the reply travels with them.
        let name_similar_note = if name_similar_missing == 0 {
            String::new()
        } else {
            format!(
                " Of those, {} {} served from {}, whose registrable name extends this site's own. Name similarity is not evidence of common ownership, and integrity applies by whether a resource can carry a stable hash rather than by who publishes it: a content-hashed bundle on a site's own delivery domain is the strongest candidate on the page, and compromise of that delivery path is the attack SRI addresses.",
                name_similar_missing,
                if name_similar_missing == 1 { "is" } else { "are" },
                name_similar_sites.iter().cloned().collect::<Vec<_>>().join(", "),
            )
        };
        let own_site_note = if own_site_excluded.is_empty() {
            String::new()
        } else {
            format!(
                " {} cross-origin resource{} excluded because {} served from the page's own registrable domain, so a change to those bytes is a change to the site's own build rather than to a resource another party controls. A host whose name merely resembles the site's is still counted: name similarity is not evidence of common ownership. Each excluded URL and reason is preserved in the evidence.",
                own_site_excluded.len(),
                if own_site_excluded.len() == 1 { " was" } else { "s were" },
                if own_site_excluded.len() == 1 { "it is" } else { "they are" },
            )
        };

        vec![CheckResult {
            check_id: "security.sri".into(),
            category: ScanCategory::Security,
            title: if count == 0 {
                "Subresource integrity (SRI)".into()
            } else {
                "Cross-origin resources without integrity attributes".into()
            },
            description: if count == 0 {
                format!(
                    "All pinnable external scripts and stylesheets use integrity attributes, or no external resources found.{}{}",
                    rolling_note, own_site_note
                )
            } else {
                format!(
                    "{} cross-origin resource{} loaded without an integrity attribute{}. SRI can pin a stable, versioned resource to expected bytes, but this source check does not establish that each URL is immutable or that its server permits the CORS request SRI requires.{}{}{}",
                    count,
                    if count == 1 { "" } else { "s" },
                    if count > MAX_LISTED_URLS {
                        format!(" (the evidence lists the first {MAX_LISTED_URLS})")
                    } else {
                        String::new()
                    },
                    name_similar_note,
                    rolling_note,
                    own_site_note
                )
            },
            status: if count == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: match count {
                0 => None,
                // A handful of hand-written tags: each one is its own decision.
                1..=MAX_LISTED_URLS => Some("Review each surfaced URL. When it identifies stable, versioned bytes and the origin supports CORS, generate a SHA-384 hash from the exact response and add integrity=\"sha384-...\" with crossorigin=\"anonymous\". For intentionally mutable vendor scripts, follow the vendor's trust and loading guidance instead of pinning a hash that will break on update; self-host a reviewed version only when the license and update process support it.".into()),
                // Dozens of hashed filenames come from one build, so the fix is
                // a build setting rather than dozens of hand-generated hashes.
                _ => Some("At this count the tags almost certainly come from one build, so fix it there rather than per URL: enable the integrity option in the bundler or asset pipeline that emits these tags so every emitted tag carries `integrity` and `crossorigin=\"anonymous\"`. Confirm the delivery host returns `Access-Control-Allow-Origin` for these assets, since a browser fails a cross-origin integrity check without it, and re-run after a deploy so the hashes match the bytes actually served. Handle any remaining hand-written vendor tags individually, following the vendor's guidance for intentionally mutable resources.".into()),
            },
            raw_data: if count > 0 || !rolling_excluded.is_empty() || !own_site_excluded.is_empty() {
                Some(serde_json::json!({
                    "missing_integrity": missing.iter().take(MAX_LISTED_URLS).collect::<Vec<_>>(),
                    "count": count,
                    "name_similar_count": name_similar_missing,
                    "excluded_rolling_resources": rolling_excluded,
                    "excluded_own_site_resources": own_site_excluded,
                }))
            } else {
                None
            },
            confidence: if count > 0 {
                crate::checks::IssueConfidence::Confirmed
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: (count > 0).then(|| "The integrity attribute is directly absent, but URL immutability, vendor update policy, and cross-origin SRI compatibility were not verified.".into()),
            why_it_matters: if count == 0 {
                None
            } else {
                Some("For a stable external script or stylesheet, SRI can prevent the browser from executing unexpectedly changed bytes after a CDN, account, or delivery-path compromise. It is not applicable to every mutable vendor resource.".into())
            },
        }]
    }
}

fn is_cross_origin(reference: &str, page_url: &url::Url) -> bool {
    let Ok(resolved) = page_url.join(reference.trim()) else {
        return false;
    };
    matches!(resolved.scheme(), "http" | "https") && resolved.origin() != page_url.origin()
}

/// Why a cross-origin reference is nonetheless the site's own delivery, so
/// SRI's third-party threat model does not apply to it. SRI protects a page
/// from bytes another organization can change, and an asset host on the site's
/// own registrable domain is published by the same build that publishes the
/// page.
///
/// Only an exact registrable-domain match qualifies. `performance.third_party`
/// also treats a name-extending host (`github.com` to `githubassets.com`) as
/// the site's own delivery domain, but that relation is a name similarity, not
/// evidence of common ownership: it would equally relate `pay.com` to
/// `paypal.com`. Dropping a genuinely third-party script from a security
/// control on that guess is not a trade this check makes, so the relation
/// stays with the advisory count.
/// The registrable site of a reference whose name extends the page's own, for
/// the sentence that explains why such a resource is still counted.
fn name_similar_site(reference: &str, page_url: &url::Url) -> Option<String> {
    let resolved = page_url.join(reference.trim()).ok()?;
    let host = resolved.host_str()?;
    let page_host = page_url.host_str()?;
    match site_relation(host, page_host) {
        SiteRelation::RelatedName => Some(
            crate::checks::performance::third_party::registrable_site(host),
        ),
        SiteRelation::SameSite | SiteRelation::CrossSite => None,
    }
}

fn own_site_exclusion_reason(reference: &str, page_url: &url::Url) -> Option<&'static str> {
    let resolved = page_url.join(reference.trim()).ok()?;
    let host = resolved.host_str()?;
    let page_host = page_url.host_str()?;
    match site_relation(host, page_host) {
        SiteRelation::SameSite => Some(
            "Served from the page's own registrable domain, so the bytes come from the same publisher rather than a third party",
        ),
        SiteRelation::RelatedName | SiteRelation::CrossSite => None,
    }
}

fn evidence_url(url: &str) -> String {
    let safe = crate::log_sanitizer::evidence_safe_url_reference(url);
    if safe.len() > 100 {
        let cut = crate::checks::floor_char_boundary(&safe, 97);
        format!("{}…", &safe[..cut])
    } else {
        safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};
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

    #[test]
    fn test_sri_external_script_with_integrity_pass() {
        let html = r#"<script src="https://cdn.jsdelivr.net/lib.js" integrity="sha384-abc123" crossorigin="anonymous"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_sri_external_script_without_integrity_fail() {
        let html = r#"<script src="https://cdn.jsdelivr.net/lib.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::Confirmed
        );
        assert!(results[0].description.contains("stable, versioned"));
        assert!(!results[0].description.contains("could be injected"));
    }

    #[test]
    fn test_sri_first_party_script_ignored() {
        let html = r#"<script src="/assets/app.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "first-party scripts don't need SRI"
        );
    }

    #[test]
    fn test_sri_same_host_script_ignored() {
        let html = r#"<script src="https://example.com/app.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "same-host scripts don't need SRI"
        );
    }

    #[test]
    fn test_sri_external_stylesheet_without_integrity_fail() {
        let html = r#"<link rel="stylesheet" href="https://cdn.example.net/style.css">"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn unquoted_external_script_on_minified_html_is_flagged() {
        let html = "<script src=https://cdn.jsdelivr.net/lib.js></script>";
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn integrity_in_the_src_url_is_not_an_integrity_attribute() {
        let html = r#"<script src="https://cdn.example.net/lib.js?integrity=1"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn stripe_js_rolling_script_is_not_flagged() {
        let html = r#"<script src="https://js.stripe.com/v3"></script>
            <script src="https://www.google-analytics.com/analytics.js"></script>
            <script src="https://www.paypal.com/sdk/js?client-id=test"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
        assert!(
            results[0]
                .description
                .contains("excluded from the missing-integrity count"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn rolling_scripts_are_excluded_from_the_missing_count() {
        // One pinnable CDN script without SRI + one rolling Stripe script:
        // the count must be 1 and the description must explain the exclusion.
        let html = r#"<script src="https://js.stripe.com/v3"></script>
            <script src="https://cdn.jsdelivr.net/lib.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(
            results[0]
                .description
                .contains("1 cross-origin resource loaded"),
            "{}",
            results[0].description
        );
        assert!(
            results[0]
                .description
                .contains("excluded from the missing-integrity count"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn google_fonts_stylesheet_is_not_flagged() {
        let html = r#"<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;700&display=swap">"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "Google Fonts CSS cannot use SRI and must not be flagged"
        );
    }

    #[test]
    fn stable_files_on_vendor_hosts_are_not_blanket_excluded() {
        let html = r#"<script src="https://www.googletagmanager.com/static/vendor-1.2.3.js"></script>
            <script src="https://www.paypalobjects.com/assets/checkout-4.5.6.js"></script>
            <script src="https://plausible.io/vendor/lib-1.2.3.js"></script>
            <script src="https://cdn.mxpnl.com/libs/mixpanel-2.45.0.min.js"></script>
            <script src="https://browser.sentry-cdn.com/7.0.0/bundle.min.js"></script>
            <script src="https://www.google.com/js/bg/stable-1.2.3.js"></script>
            <script src="https://static.cloudflareinsights.com/vendor-1.0.0.js"></script>
            <script src="https://pagead2.googlesyndication.com/pagead/managed/js/adsense-1.2.3.js"></script>"#;
        let result = &SubresourceIntegrityCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.raw_data.as_ref().unwrap()["count"], 8);
    }

    #[test]
    fn a_different_port_on_the_same_site_is_the_page_publisher_not_a_third_party() {
        let html = r#"<script src="https://example.com:444/app.js?token=supersecret"></script>"#;
        let result = &SubresourceIntegrityCheck.run(&ctx(html))[0];
        let serialized = serde_json::to_string(result).expect("serialize result");

        // Cross-origin by the URL rules, but published by the same site, so
        // SRI's third-party threat model does not apply. The evidence still
        // records it, and still redacts the query secret.
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.raw_data.as_ref().unwrap()["count"], 0);
        assert_eq!(
            result.raw_data.as_ref().unwrap()["excluded_own_site_resources"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(serialized.contains("https://example.com:444/app.js"));
        assert!(!serialized.contains("supersecret"));
        assert!(!serialized.contains("token="));
    }

    #[test]
    fn the_pages_own_registrable_domain_is_not_graded_for_missing_integrity() {
        let html = r#"<script src="https://cdn.example.com/core/bundle.js"></script>
            <link rel="stylesheet" href="https://cdn.example.com/core/main.css">
            <script src="https://cdn.unrelated-vendor.net/widget.js"></script>"#;
        let result = &SubresourceIntegrityCheck.run(&ctx(html))[0];
        let raw = result.raw_data.as_ref().expect("evidence");

        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(raw["count"], 1, "only the unrelated vendor is counted");
        assert!(raw["missing_integrity"][0]
            .as_str()
            .unwrap()
            .contains("unrelated-vendor.net"));
        assert_eq!(
            raw["excluded_own_site_resources"].as_array().unwrap().len(),
            2
        );
        assert!(
            result.description.contains("own registrable domain"),
            "{}",
            result.description
        );
    }

    #[test]
    fn a_name_similar_host_carries_the_reasoning_on_the_row_that_surfaces_it() {
        // The row surfaces resources on a host that plainly looks like the
        // site's own, so the reply to that objection has to travel with them:
        // the exclusion note cannot carry it, because nothing was excluded.
        let scripts: String = (0..12)
            .map(|i| {
                format!(
                    r#"<script src="https://static.files.bbci.co.uk/core/bundle-{i}.js"></script>"#
                )
            })
            .collect();
        let page = PageContext {
            url: url::Url::parse("https://www.bbc.co.uk/").unwrap(),
            ..ctx(&scripts)
        };
        let result = &SubresourceIntegrityCheck.run(&page)[0];
        let raw = result.raw_data.as_ref().expect("evidence");

        assert_eq!(raw["count"], 12);
        assert_eq!(raw["name_similar_count"], 12);
        assert_eq!(
            raw["excluded_own_site_resources"].as_array().unwrap().len(),
            0,
            "nothing is excluded, so the exclusion note cannot be the explanation"
        );
        for expected in [
            "Of those, 12 are served from bbci.co.uk",
            "Name similarity is not evidence of common ownership",
            "content-hashed bundle on a site's own delivery domain is the strongest candidate",
            "compromise of that delivery path is the attack SRI addresses",
        ] {
            assert!(
                result.description.contains(expected),
                "missing {expected:?} in: {}",
                result.description
            );
        }
    }

    #[test]
    fn many_bundles_are_listed_briefly_and_fixed_at_the_build() {
        let scripts: String = (0..40)
            .map(|i| format!(r#"<script src="https://cdn.vendor.net/app-{i}.js"></script>"#))
            .collect();
        let result = &SubresourceIntegrityCheck.run(&ctx(&scripts))[0];
        let raw = result.raw_data.as_ref().expect("evidence");

        assert_eq!(raw["count"], 40, "the total is kept");
        assert_eq!(
            raw["missing_integrity"].as_array().unwrap().len(),
            super::MAX_LISTED_URLS,
            "the list is capped like every other example list in this lane"
        );
        assert!(
            result
                .description
                .contains("the evidence lists the first 5"),
            "{}",
            result.description
        );
        let fix = result.manual_fix.as_deref().expect("guidance");
        assert!(
            fix.contains("bundler or asset pipeline")
                && fix.contains("Access-Control-Allow-Origin"),
            "40 tags from one build are fixed at the build: {fix}"
        );
        assert!(
            !fix.starts_with("Review each surfaced URL"),
            "per-URL guidance does not scale to 40 bundles: {fix}"
        );
    }

    #[test]
    fn a_handful_of_hand_written_tags_keeps_the_per_url_guidance() {
        let scripts: String = (0..3)
            .map(|i| format!(r#"<script src="https://cdn.vendor.net/widget-{i}.js"></script>"#))
            .collect();
        let result = &SubresourceIntegrityCheck.run(&ctx(&scripts))[0];
        let raw = result.raw_data.as_ref().expect("evidence");

        assert_eq!(raw["missing_integrity"].as_array().unwrap().len(), 3);
        assert!(!result.description.contains("the evidence lists the first"));
        assert!(result
            .manual_fix
            .as_deref()
            .expect("guidance")
            .starts_with("Review each surfaced URL"));
    }

    #[test]
    fn the_row_does_not_call_the_sites_own_delivery_domain_a_third_party() {
        let html = r#"<script src="https://static.files.bbci.co.uk/core/bundle.js"></script>"#;
        let page = PageContext {
            url: url::Url::parse("https://www.bbc.co.uk/").unwrap(),
            ..ctx(html)
        };
        let result = &SubresourceIntegrityCheck.run(&page)[0];
        let why = result.why_it_matters.as_deref().expect("why it matters");

        assert!(
            !why.contains("third-party"),
            "the surfaced resource may well be the site's own delivery domain: {why}"
        );
        assert!(why.contains("delivery-path compromise"), "{why}");
    }

    #[test]
    fn a_merely_name_similar_host_is_still_graded_for_missing_integrity() {
        // `performance.third_party` treats a name-extending host as the site's
        // own delivery domain. That relation would equally relate pay.com to
        // paypal.com, so a security control does not act on it: the resource
        // stays in the missing-integrity count.
        let html = r#"<script src="https://static.files.bbci.co.uk/core/bundle.js"></script>"#;
        let page = PageContext {
            url: url::Url::parse("https://www.bbc.co.uk/").unwrap(),
            ..ctx(html)
        };
        let result = &SubresourceIntegrityCheck.run(&page)[0];
        let raw = result.raw_data.as_ref().expect("evidence");

        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(raw["count"], 1);
        assert_eq!(
            raw["excluded_own_site_resources"].as_array().unwrap().len(),
            0
        );
    }

    #[test]
    fn every_rolling_endpoint_example_is_excluded_with_its_reason() {
        for (url, reason_fragment) in super::rolling_endpoints::ROLLING_EXAMPLES {
            let reason = sri_exclusion_reason(url)
                .unwrap_or_else(|| panic!("{url} must match a rolling endpoint"));
            assert!(
                reason.contains(reason_fragment),
                "{url}: reason {reason:?} must mention {reason_fragment}"
            );
            let html = format!(r#"<script src="{url}"></script>"#);
            let result = &SubresourceIntegrityCheck.run(&ctx(&html))[0];
            assert_eq!(
                result.status,
                CheckStatus::Pass,
                "{url}: {}",
                result.description
            );
            let raw = result.raw_data.as_ref().expect("excluded URL evidence");
            assert_eq!(raw["count"], 0, "{url}");
            assert_eq!(
                raw["excluded_rolling_resources"].as_array().unwrap().len(),
                1,
                "{url}"
            );
        }
    }

    #[test]
    fn rolling_exclusion_is_preserved_as_evidence() {
        let html = r#"<script src="https://js.stripe.com/v3"></script>"#;
        let result = &SubresourceIntegrityCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Pass);
        let raw = result.raw_data.as_ref().expect("excluded URL evidence");
        assert_eq!(
            raw["excluded_rolling_resources"].as_array().unwrap().len(),
            1
        );
        assert!(raw["excluded_rolling_resources"][0]["url"]
            .as_str()
            .unwrap()
            .contains("js.stripe.com/v3"));
        assert!(raw["excluded_rolling_resources"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("change in place"));
    }
}
