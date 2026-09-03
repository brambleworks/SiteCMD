//! Counts cross-site scripts in fetched HTML, not runtime requests.

use crate::checks::html_attrs::{attr_value, has_attr, tag_slices};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::collections::HashSet;

pub struct ThirdPartyScriptsCheck;

impl Check for ThirdPartyScriptsCheck {
    fn id(&self) -> &str {
        "performance.third_party"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let page_host = ctx.url.host_str().unwrap_or("").to_ascii_lowercase();

        let mut external_sites: HashSet<String> = HashSet::new();
        let mut external_origins: HashSet<String> = HashSet::new();
        let mut related_name_sites: HashSet<String> = HashSet::new();
        let mut related_name_count = 0;
        let mut external_count = 0;
        // Parser-blocking candidates have none of async/defer/type=module.
        // This source observation does not prove the request succeeds or the
        // script executes on the scanned navigation.
        let mut sync_count = 0;
        let mut external_script_srcs: Vec<String> = Vec::new();

        for tag in tag_slices(&ctx.body, ctx.body_lower(), "script") {
            let Some(src) = attr_value(tag, "src").filter(|value| !value.trim().is_empty()) else {
                continue;
            };
            let Ok(resolved) = ctx.url.join(src.trim()) else {
                continue;
            };
            if !matches!(resolved.scheme(), "http" | "https") {
                continue;
            }
            let Some(host) = resolved.host_str().map(str::to_ascii_lowercase) else {
                continue;
            };
            match site_relation(&host, &page_host) {
                SiteRelation::SameSite => continue,
                SiteRelation::RelatedName => {
                    related_name_sites.insert(registrable_site(&host));
                    related_name_count += 1;
                    continue;
                }
                SiteRelation::CrossSite => {}
            }

            external_sites.insert(registrable_site(&host));
            external_origins.insert(resolved.origin().ascii_serialization());
            external_count += 1;
            let is_module = attr_value(tag, "type")
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("module"));
            if !has_attr(tag, "async") && !has_attr(tag, "defer") && !is_module {
                sync_count += 1;
            }
            if external_script_srcs.len() < 10 {
                external_script_srcs.push(crate::log_sanitizer::evidence_safe_page_url(
                    resolved.as_str(),
                ));
            }
        }

        let (status, severity) = if external_count > 10 {
            (CheckStatus::Fail, Severity::Medium)
        } else if external_count > 5 {
            (CheckStatus::Warn, Severity::Low)
        } else {
            (CheckStatus::Pass, Severity::Low)
        };

        let mut sites: Vec<String> = external_sites.into_iter().collect();
        sites.sort();
        let mut origins: Vec<String> = external_origins.into_iter().collect();
        origins.sort();
        let mut related_sites: Vec<String> = related_name_sites.into_iter().collect();
        related_sites.sort();
        let related_note = if related_name_count == 0 {
            String::new()
        } else {
            format!(
                " {} script tag{} load from {}, whose registrable name extends this site's own under the same public suffix. Sites commonly deliver their own assets from such a domain, and the fetched markup does not establish ownership, so these are listed rather than counted as cross-site. Name similarity is not proof of common ownership, so security checks such as `security.sri` ignore this relation and require an exact registrable-domain match.",
                related_name_count,
                if related_name_count == 1 { "" } else { "s" },
                related_sites.join(", "),
            )
        };

        vec![CheckResult {
            check_id: "performance.third_party".into(),
            category: ScanCategory::Performance,
            title: if external_count > 5 {
                format!("{} cross-site script tags in source markup", external_count)
            } else {
                "Cross-site script tags".into()
            },
            description: if external_count == 0 {
                "No cross-site `<script src>` tag was found in the fetched HTML. Runtime injection, workers, modules imported by scripts, subresources, and same-site third-party services are outside this source check.".to_string() + &related_note
            } else {
                format!(
                    "{} cross-site script tag{} from {} registrable site{} and {} origin{} appear{} in the fetched source markup: {}. Separate origins can require connection setup, though DNS caching, connection reuse/coalescing, and protocol behavior affect the cost.{}",
                    external_count,
                    if external_count == 1 { "" } else { "s" },
                    sites.len(),
                    if sites.len() == 1 { "" } else { "s" },
                    origins.len(),
                    if origins.len() == 1 { "" } else { "s" },
                    if external_count == 1 { "s" } else { "" },
                    sites.join(", "),
                    if sync_count > 0 {
                        format!(
                            " {} tag{} declare{} neither async, defer, nor type=module and can block HTML parsing if fetched and executed.",
                            sync_count,
                            if sync_count == 1 { "" } else { "s" },
                            if sync_count == 1 { "s" } else { "" },
                        )
                    } else {
                        " Every surfaced tag declares async, defer, or type=module, so none is parser-blocking under normal HTML script semantics.".to_string()
                    }
                ) + &related_note
            },
            status,
            severity,
            fix_prompt: None,
            manual_fix: if external_count > 5 {
                Some("Inventory each surfaced tag's owner, product purpose, consent/privacy requirements, dependency order, and measured network/main-thread cost. Remove stale or duplicate integrations. Use async, defer, modules, delayed loading, or self-hosting only when licensing and behavior permit, then test blocked/slow vendor failure states. Preconnect only to origins proven critical and otherwise discovered late.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "external_script_count": external_count,
                "sync_script_count": sync_count,
                "registrable_sites": sites,
                "origins": origins,
                "external_script_srcs": external_script_srcs,
                "related_name_script_count": related_name_count,
                "related_name_sites": related_sites,
            })),
            confidence: if status == CheckStatus::Pass {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: (status != CheckStatus::Pass).then(|| "The source tags, sites, origins, and loading attributes are directly observed, but tag count does not measure actual requests, execution cost, connection reuse, ownership, consent state, or user value.".into()),
            why_it_matters: match status {
                CheckStatus::Warn | CheckStatus::Fail => {
                    if sync_count > 0 {
                        Some("If these tags fetch and execute, cross-site scripts can add connection, transfer, privacy, availability, and main-thread cost; parser-blocking candidates can also delay document parsing.".into())
                    } else {
                        Some("If these tags fetch and execute, cross-site scripts can add connection, transfer, privacy, availability, and main-thread cost even when they do not block HTML parsing.".into())
                    }
                }
                _ => None,
            },
        }]
    }
}

/// The registrable site (PSL eTLD+1) a host belongs to, normalized the same
/// way `security::dns_email::registrable_domain_for_url` normalizes a scan
/// target: a leading `www.` is dropped first, because under a multi-label
/// public suffix (`gov.uk`, `co.uk`) the PSL alone answers `www.gov.uk` for
/// `www.gov.uk`. Hosts with no public suffix (localhost, IP literals) are
/// their own site.
pub fn registrable_site(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let candidate = host.strip_prefix("www.").unwrap_or(&host);
    psl::domain_str(candidate)
        .unwrap_or(candidate)
        .to_ascii_lowercase()
}

/// How a resource host relates to the site being scanned. Shared so
/// `performance.third_party` and `security.sri` answer "is this someone
/// else's code?" the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteRelation {
    /// The page's own registrable site, including any subdomain of it.
    SameSite,
    /// A different registrable site whose name extends the page's under the
    /// same public suffix: `github.com` to `github`assets`.com`,
    /// `bbc.co.uk` to `bbc`i`.co.uk`. Sites commonly deliver their own assets
    /// from such a domain, and the name is the only evidence fetched markup
    /// carries. It does not prove common ownership - the same rule relates
    /// `pay.com` to `paypal.com` - so it is used only to keep an advisory
    /// count honest, and a verdict names these hosts in its evidence instead
    /// of dropping them silently. Security checks require `SameSite`.
    RelatedName,
    /// An unrelated registrable site.
    CrossSite,
}

impl SiteRelation {
    /// Whether the resource comes from someone else's site as far as the
    /// fetched markup can tell.
    pub fn is_third_party(self) -> bool {
        matches!(self, Self::CrossSite)
    }
}

/// Shortest brand label that may take part in the name-extension rule. Two
/// characters would relate `x.com` to `xy.com`, which says nothing.
const MIN_RELATED_LABEL_LEN: usize = 3;

pub fn site_relation(resource_host: &str, page_host: &str) -> SiteRelation {
    let resource_site = registrable_site(resource_host);
    let page_site = registrable_site(page_host);
    if resource_site == page_site {
        return SiteRelation::SameSite;
    }
    let split = |site: &str| {
        site.split_once('.')
            .map(|(label, suffix)| (label.to_string(), suffix.to_string()))
    };
    let (Some((resource_label, resource_suffix)), Some((page_label, page_suffix))) =
        (split(&resource_site), split(&page_site))
    else {
        return SiteRelation::CrossSite;
    };
    let extends = resource_suffix == page_suffix
        && resource_label.len() >= MIN_RELATED_LABEL_LEN
        && page_label.len() >= MIN_RELATED_LABEL_LEN
        && (resource_label.starts_with(&page_label) || page_label.starts_with(&resource_label));
    if extends {
        SiteRelation::RelatedName
    } else {
        SiteRelation::CrossSite
    }
}

#[cfg(test)]
mod tests {
    use super::{site_relation, SiteRelation, ThirdPartyScriptsCheck};
    use crate::checks::{Check, CheckStatus, PageContext};

    fn ctx(body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: http::header::HeaderMap::new(),
            status_code: 200,
            body: body.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn async_and_defer_scripts_are_not_claimed_to_block_rendering() {
        let scripts: String = (0..6)
            .map(|i| {
                format!(r#"<script async src="https://widget{i}.vendor{i}.net/w.js"></script>"#)
            })
            .collect();
        let results = ThirdPartyScriptsCheck.run(&ctx(&scripts));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(
            !results[0].description.contains("block rendering")
                && results[0].description.contains("none is parser-blocking"),
            "async scripts must not be described as render-blocking: {}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["sync_script_count"],
            0
        );
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn synchronous_scripts_are_counted_and_described_as_blocking() {
        let scripts: String = (0..6)
            .map(|i| format!(r#"<script src="https://widget{i}.vendor{i}.net/w.js"></script>"#))
            .collect();
        let results = ThirdPartyScriptsCheck.run(&ctx(&scripts));
        assert!(
            results[0].description.contains("6 tags declare neither")
                && results[0].description.contains("can block HTML parsing"),
            "sync scripts must be called out: {}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["sync_script_count"],
            6
        );
    }

    #[test]
    fn fix_scopes_preconnect_to_render_critical_origins() {
        // Preconnect must remain tied to measured critical-path evidence.
        let scripts: String = (0..6)
            .map(|i| format!(r#"<script src="https://widget{i}.vendor{i}.net/w.js"></script>"#))
            .collect();
        let results = ThirdPartyScriptsCheck.run(&ctx(&scripts));
        let fix = results[0].manual_fix.as_deref().unwrap_or("");
        assert!(
            fix.contains("origins proven critical") && fix.contains("discovered late"),
            "fix must scope preconnect advice: {fix}"
        );
    }

    #[test]
    fn relative_paths_are_not_third_party_hosts() {
        let result = ThirdPartyScriptsCheck
            .run(&ctx(r#"<script src="js/app.js"></script>
                   <script src="assets/main.js"></script>
                   <script src="/static/bundle.js"></script>
                   <script src="./vendor.js"></script>
                   <script src="../lib/x.js"></script>
                   <script src="bundle.min.js"></script>"#))
            .remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(
            result.raw_data.as_ref().unwrap()["external_script_count"],
            0
        );
    }

    #[test]
    fn absolute_and_protocol_relative_urls_are_resolved_by_the_check() {
        let result = ThirdPartyScriptsCheck
            .run(&ctx(r#"<script src="https://cdn.vendor.net/a.js"></script>
                   <script src="//fonts.gstatic.com/x.js"></script>
                   <script src="http://Analytics.VENDOR.org:8080/t.js"></script>"#))
            .remove(0);
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(raw["external_script_count"], 3);
        assert_eq!(raw["origins"].as_array().map(Vec::len), Some(3));
        assert!(raw["origins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "http://analytics.vendor.org:8080"));
    }

    #[test]
    fn a_dotless_cross_host_is_still_observed_as_cross_site() {
        let result = ThirdPartyScriptsCheck
            .run(&ctx(r#"<script src="//localhost/x.js"></script>"#))
            .remove(0);
        assert_eq!(
            result.raw_data.as_ref().unwrap()["external_script_count"],
            1
        );
    }

    #[test]
    fn same_site_uses_the_public_suffix_list_instead_of_last_two_labels() {
        assert_eq!(
            site_relation("static.example.co.uk", "www.example.co.uk"),
            SiteRelation::SameSite
        );
        assert_eq!(
            site_relation("evil.co.uk", "www.example.co.uk"),
            SiteRelation::CrossSite
        );
        assert_ne!(
            site_relation("githubassets.com", "github.com"),
            SiteRelation::SameSite,
            "a separate registration is never the same site"
        );
    }

    #[test]
    fn a_site_delivery_domain_is_related_by_name_not_cross_site() {
        // The two live cases: github.com serves 9 tags from
        // github.githubassets.com, bbc.co.uk serves 55 from
        // static.files.bbci.co.uk.
        assert_eq!(
            site_relation("github.githubassets.com", "github.com"),
            SiteRelation::RelatedName
        );
        assert_eq!(
            site_relation("static.files.bbci.co.uk", "www.bbc.co.uk"),
            SiteRelation::RelatedName
        );
        assert_eq!(
            site_relation("static.example.com", "www.example.com"),
            SiteRelation::SameSite
        );
        assert!(!SiteRelation::RelatedName.is_third_party());
        assert!(SiteRelation::CrossSite.is_third_party());
    }

    #[test]
    fn an_unrelated_vendor_is_never_related_by_name() {
        for (resource, page) in [
            ("cdn.vendor.net", "example.com"),
            // A different public suffix is a different registration entirely.
            ("facebook.net", "face.com"),
            // Two-character labels relate nothing.
            ("xy.com", "x.com"),
            // Neither name extends the other.
            ("assetsgithub.com", "gitlab.com"),
        ] {
            assert_eq!(
                site_relation(resource, page),
                SiteRelation::CrossSite,
                "{resource} on {page}"
            );
        }
    }

    #[test]
    fn a_name_related_delivery_domain_is_listed_instead_of_counted() {
        let scripts: String = (0..12)
            .map(|i| {
                format!(
                    r#"<script defer src="https://static.files.bbci.co.uk/core/a{i}.js"></script>"#
                )
            })
            .collect();
        let page = PageContext {
            url: url::Url::parse("https://www.bbc.co.uk/").unwrap(),
            ..ctx(&scripts)
        };
        let result = ThirdPartyScriptsCheck.run(&page).remove(0);

        assert_eq!(
            result.status,
            CheckStatus::Pass,
            "12 tags from one name-related delivery domain are not a failure: {}",
            result.description
        );
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(raw["external_script_count"], 0);
        assert_eq!(raw["related_name_script_count"], 12);
        assert_eq!(raw["related_name_sites"][0], "bbci.co.uk");
        assert!(
            result.description.contains("bbci.co.uk")
                && result.description.contains("does not establish ownership"),
            "{}",
            result.description
        );
    }

    #[test]
    fn unquoted_cross_site_scripts_are_counted_and_secret_query_is_redacted() {
        let scripts: String = (0..6)
            .map(|i| {
                format!(
                    "<script src=https://cdn{i}.vendor{i}.net/private/widget.js?token=secret{i}></script>"
                )
            })
            .collect();
        let result = ThirdPartyScriptsCheck.run(&ctx(&scripts)).remove(0);
        assert_eq!(result.status, CheckStatus::Warn);
        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(
            serialized.contains("/private/widget.js"),
            "non-secret path evidence should remain locatable: {serialized}"
        );
        assert!(!serialized.contains("token"), "{serialized}");
        assert!(!serialized.contains("secret"), "{serialized}");
        assert!(result.description.contains("cross-site script tags"));
        assert!(result.description.contains("source markup"));
    }
}
