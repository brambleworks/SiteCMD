//! Canonical URL and robots-meta consistency checks.

use super::parsing::extract_attr_value;
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::collections::{BTreeMap, BTreeSet};

/// HTML canonical href values outside inert markup examples. HTTP Link
/// headers are a separate channel and are called out as an uninspected limit.
fn canonical_hrefs(body: &str) -> Vec<String> {
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(body, " ");
    let lower = scannable.to_ascii_lowercase();
    crate::checks::html_attrs::tag_slices(&scannable, &lower, "link")
        .into_iter()
        .filter(|tag| {
            extract_attr_value(tag, "rel").is_some_and(|rel| {
                rel.split_ascii_whitespace()
                    .any(|token| token.eq_ignore_ascii_case("canonical"))
            })
        })
        .map(|tag| extract_attr_value(tag, "href").unwrap_or_default())
        .collect()
}

pub struct CanonicalMismatchCheck;

impl Check for CanonicalMismatchCheck {
    fn id(&self) -> &str {
        "seo.canonical_mismatch"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let canonicals = canonical_hrefs(&ctx.body);
        let canonical = match canonicals.first() {
            Some(canonical) => canonical,
            None => return vec![],
        };

        let resolved_canonicals: Vec<Option<url::Url>> = canonicals
            .iter()
            .map(|value| {
                (!value.trim().is_empty())
                    .then(|| ctx.url.join(value.trim()).ok())
                    .flatten()
            })
            .collect();
        let safe_targets: Vec<String> = resolved_canonicals
            .iter()
            .map(|target| {
                target.as_ref().map_or_else(
                    || "[missing-or-unresolvable-url]".into(),
                    |url| crate::log_sanitizer::evidence_safe_page_url(url.as_str()),
                )
            })
            .collect();

        if canonicals.len() > 1 {
            let unique_targets = resolved_canonicals
                .iter()
                .map(|target| {
                    target.as_ref().map_or_else(
                        || "[missing-or-unresolvable-url]".into(),
                        normalized_for_compare,
                    )
                })
                .collect::<std::collections::HashSet<_>>()
                .len();
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Multiple HTML canonical declarations observed".into(),
                description: format!(
                    "Found {} rel=canonical declarations in the scannable initial HTML, resolving to {} distinct target representation{}. Multiple declarations are ambiguous even when their href values agree. SiteCMD did not inspect HTTP Link headers or the rendered head.",
                    canonicals.len(),
                    unique_targets,
                    if unique_targets == 1 { "" } else { "s" },
                ),
                status: CheckStatus::Warn,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: Some("Trace framework/layout metadata, CMS/plugins, templates, client head management, and edge headers. Emit one coherent HTML canonical for the route, then inspect the rendered production head and HTTP Link headers across navigation, locale, pagination, fallback, and error states.".into()),
                raw_data: Some(serde_json::json!({
                    "html_canonical_count": canonicals.len(),
                    "distinct_resolved_target_count": unique_targets,
                    "canonical_targets": safe_targets,
                    "source": "initial_html",
                    "http_link_headers_inspected": false,
                    "rendered_head_inspected": false,
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some("Multiple canonical declarations make the intended representative ambiguous; consumers treat canonical as a hint and can choose their own representative.".into()),
            }];
        }

        let canonical_resolved = resolved_canonicals.into_iter().next().flatten();
        let safe_current = crate::log_sanitizer::evidence_safe_page_url(ctx.url.as_str());
        let safe_canonical = safe_targets
            .into_iter()
            .next()
            .unwrap_or_else(|| "[missing-or-unresolvable-url]".into());
        let is_relative_reference = url::Url::parse(canonical.trim()).is_err()
            && canonical_resolved.is_some()
            && !canonical.trim().is_empty();

        let matches_page = canonical_resolved
            .as_ref()
            .map(|resolved| normalized_for_compare(resolved) == normalized_for_compare(&ctx.url))
            .unwrap_or(false);

        if matches_page {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Canonical URL match".into(),
                description: format!(
                    "The single HTML canonical resolves to the scanned URL ({}).{} This comparison does not inspect HTTP Link headers, duplicate content variants, target selection by a search engine, or later client-side head changes.",
                    safe_current,
                    if is_relative_reference {
                        " The source href is relative; it resolved against this response URL, but a fully qualified public URL is easier to audit across environments."
                    } else {
                        ""
                    },
                ),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "current_url": safe_current,
                    "canonical_url": safe_canonical,
                    "html_canonical_count": 1,
                    "relative_reference": is_relative_reference,
                    "http_link_headers_inspected": false,
                    "rendered_head_inspected": false,
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        let current_host = normalized_host(&ctx.url);
        let canonical_host = canonical_resolved
            .as_ref()
            .map(normalized_host)
            .unwrap_or_default();
        let is_different_domain =
            !canonical_host.is_empty() && !canonical_host.eq_ignore_ascii_case(&current_host);

        if ctx.is_localhost && is_different_domain {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Production canonical on localhost preview".into(),
                description: format!("The localhost response's single HTML canonical resolves to {}. A production-host canonical can be intentional in local preview, so this comparison is skipped; its deployed status, content equivalence, and HTTP Link headers were not checked.", safe_canonical),
                status: CheckStatus::Skipped,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "current_url": safe_current,
                    "canonical_url": safe_canonical,
                    "html_canonical_count": 1,
                    "reason": "localhost_preview_server"
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        vec![CheckResult {
            check_id: self.id().into(), category: self.category(),
            title: if canonical_resolved.is_none() {
                "HTML canonical URL is missing or unresolvable".into()
            } else if is_different_domain {
                "HTML canonical points to another host".into()
            } else {
                "HTML canonical points to another URL".into()
            },
            description: if canonical_resolved.is_none() {
                "The single rel=canonical declaration has an empty href or a value that cannot be resolved against the scanned URL. SiteCMD did not inspect HTTP Link headers or later client-side head changes.".into()
            } else if is_different_domain {
                format!("The single HTML canonical resolves from host {} to host {} (target {}). A cross-host canonical can be intentional for syndicated or duplicate content; this check does not establish content equivalence, ownership, target status/indexability, or which representative a search engine selects.", current_host, canonical_host, safe_canonical)
            } else {
                format!("The single HTML canonical resolves to another URL: {} (scanned URL: {}). This can be intentional for a duplicate or parameterized variant. SiteCMD did not verify content equivalence, target status/indexability, redirects, HTTP Link headers, or search-engine canonical selection.", safe_canonical, safe_current)
            },
            status: CheckStatus::Warn,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: Some("Confirm whether the scanned URL is a standalone page or a genuine duplicate/near-duplicate of the canonical target. Keep an intentional target only when it is crawlable, indexable, final, content-equivalent, and aligned with redirects, internal links, sitemaps, and hreflang; otherwise correct the owning metadata source. Inspect rendered HTML and HTTP Link headers after deployment.".into()),
            raw_data: Some(serde_json::json!({
                "current_url": safe_current,
                "canonical_url": safe_canonical,
                "html_canonical_count": 1,
                "canonical_resolved": canonical_resolved.is_some(),
                "different_domain": is_different_domain,
                "different_scheme": canonical_resolved.as_ref().is_some_and(|target| target.scheme() != ctx.url.scheme()),
                "relative_reference": is_relative_reference,
                "http_link_headers_inspected": false,
                "rendered_head_inspected": false,
                "target_response_inspected": false,
                "content_equivalence_verified": false,
            })),
            confidence: if canonical_resolved.is_none() {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: canonical_resolved.is_some().then(|| "The resolved URL difference is direct evidence, but whether it is an issue depends on duplicate-content intent, target response/indexability, other canonical sources, and consumer selection.".into()),
            why_it_matters: Some(if canonical_resolved.is_none() {
                "A consumer cannot use an empty or unresolvable canonical href as a clear representative-URL hint.".into()
            } else {
                "If the target is unintended or unsuitable, canonical signals may favor a different representative. Canonical is a hint, so the observed declaration does not prove that outcome.".into()
            }),
        }]
    }
}

fn normalized_host(u: &url::Url) -> String {
    u.host_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

/// Normalize a resolved URL for canonical comparison. Host case and the
/// root slash representation are normalized by the URL parser; www and apex
/// hosts, path case, non-root trailing slashes, query values, and URL userinfo
/// remain distinct.
fn normalized_for_compare(u: &url::Url) -> String {
    let port = u.port().map(|p| format!(":{p}")).unwrap_or_default();
    let path = if u.path() == "/" { "" } else { u.path() };
    let query = u.query().map(|q| format!("?{q}")).unwrap_or_default();
    let userinfo = if u.username().is_empty() && u.password().is_none() {
        ""
    } else {
        "[userinfo]@"
    };
    format!(
        "{}://{}{}{}{}{}",
        u.scheme(),
        userinfo,
        normalized_host(u),
        port,
        path,
        query
    )
}

pub struct MetaRobotsConflictCheck;

const GENERAL_ROBOTS_SCOPE: &str = "all-crawlers";

fn add_polarity_directives(
    by_scope: &mut BTreeMap<String, BTreeSet<String>>,
    scope: &str,
    content: &str,
) {
    let directives = by_scope.entry(scope.to_string()).or_default();
    for token in content
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .map(|token| token.trim().trim_matches(':').to_ascii_lowercase())
    {
        match token.as_str() {
            "index" | "noindex" | "follow" | "nofollow" => {
                directives.insert(token);
            }
            "none" => {
                directives.insert("noindex".into());
                directives.insert("nofollow".into());
            }
            "all" => {
                directives.insert("index".into());
                directives.insert("follow".into());
            }
            _ => {}
        }
    }
}

fn normalized_robots_scope(raw: &str) -> Option<String> {
    let scope = raw.trim().to_ascii_lowercase();
    if scope.is_empty()
        || scope.len() > 64
        || !scope
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'*'))
    {
        return None;
    }
    Some(if matches!(scope.as_str(), "*" | "robots") {
        GENERAL_ROBOTS_SCOPE.into()
    } else {
        scope
    })
}

fn is_valued_robots_directive(name: &str) -> bool {
    matches!(
        name,
        "max-snippet" | "max-image-preview" | "max-video-preview" | "unavailable_after"
    )
}

fn robots_scope_label(scope: &str) -> String {
    match scope {
        GENERAL_ROBOTS_SCOPE => "General crawler".into(),
        "googlebot" => "Googlebot".into(),
        "googlebot-news" => "Googlebot-News".into(),
        "bingbot" => "Bingbot".into(),
        other => other.to_string(),
    }
}

impl Check for MetaRobotsConflictCheck {
    fn id(&self) -> &str {
        "seo.meta_conflicts"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut directives_by_scope: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut tag_count = 0;
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let lower = scannable.to_ascii_lowercase();
        for tag in crate::checks::html_attrs::tag_slices(&scannable, &lower, "meta") {
            let Some(name) = extract_attr_value(tag, "name").map(|name| name.to_ascii_lowercase())
            else {
                continue;
            };
            let scope = match name.as_str() {
                "robots" => GENERAL_ROBOTS_SCOPE,
                "googlebot" | "googlebot-news" | "bingbot" => name.as_str(),
                _ => continue,
            };
            if let Some(content) = extract_attr_value(tag, "content") {
                tag_count += 1;
                add_polarity_directives(&mut directives_by_scope, scope, &content);
            }
        }
        let header_values: Vec<&str> = ctx
            .response_headers
            .get_all("x-robots-tag")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .collect();
        for header_value in &header_values {
            let mut current_scope = GENERAL_ROBOTS_SCOPE.to_string();
            for clause in header_value
                .split(',')
                .map(str::trim)
                .filter(|clause| !clause.is_empty())
            {
                if let Some((prefix, content)) = clause.split_once(':') {
                    let prefix = prefix.trim().to_ascii_lowercase();
                    if is_valued_robots_directive(&prefix) {
                        continue;
                    }
                    let Some(scope) = normalized_robots_scope(&prefix) else {
                        continue;
                    };
                    current_scope = scope;
                    add_polarity_directives(&mut directives_by_scope, &current_scope, content);
                } else {
                    add_polarity_directives(&mut directives_by_scope, &current_scope, clause);
                }
            }
        }

        let mut conflicts: Vec<String> = Vec::new();
        for (scope, directives) in &directives_by_scope {
            let label = robots_scope_label(scope);
            if directives.contains("index") && directives.contains("noindex") {
                conflicts.push(format!(
                    "{label} scope declares index and noindex; noindex is the restrictive outcome"
                ));
            }
            if directives.contains("follow") && directives.contains("nofollow") {
                conflicts.push(format!(
                    "{label} scope declares follow and nofollow; nofollow is the restrictive outcome"
                ));
            }
        }

        let declaration_count = tag_count + header_values.len();

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if conflicts.is_empty() {
                "Page-level robots directive consistency".into()
            } else {
                "Contradictory page-level robots directives".into()
            },
            description: if conflicts.is_empty() {
                if declaration_count == 0 {
                    "No supported page-level robots meta tags or X-Robots-Tag header values were observed. This check does not inspect robots.txt and does not establish that the URL is crawlable, indexable, canonical, or selected for indexing.".into()
                } else {
                    format!(
                        "No contradictory index/noindex or follow/nofollow pair was observed within the same crawler scope across {} parsed page-level declaration{}. Supporting crawlers can combine applicable declarations; this check does not validate unknown directives, robots.txt, or each consumer's current processing behavior.",
                        declaration_count,
                        if declaration_count == 1 { "" } else { "s" },
                    )
                }
            } else {
                format!(
                    "Observed contradictory directives within the same crawler scope: {}. Supporting search engines generally apply the most restrictive applicable directive, so the outcome is restrictive rather than undefined; the declarations still obscure authoring intent.",
                    conflicts.join("; "),
                )
            },
            status: if conflicts.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            // The restrictive directive wins, so this is authoring ambiguity rather than undefined behavior.
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if !conflicts.is_empty() {
                Some(
                    "For each affected crawler scope, decide the intended index and link-following policy, then remove the opposite directive from the responsible HTML meta tag or X-Robots-Tag header. Multiple declarations are allowed, so consolidate only when it improves ownership and prevents the contradiction. Re-fetch the production response and inspect the relevant search-console URL after deployment."
                        .into(),
                )
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "directives_by_scope": directives_by_scope,
                "meta_tag_count": tag_count,
                "header_value_count": header_values.len(),
                "conflicts": conflicts,
                "robots_txt_inspected": false,
            })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if conflicts.is_empty() {
                None
            } else {
                Some("Contradictory declarations can leave a restrictive indexing or link-following outcome in effect while making the intended policy difficult to audit and maintain. The behavior is consumer-specific but is not assumed to be undefined.".into())
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;
    use http::header::HeaderMap;

    fn ctx(body: &str) -> PageContext {
        ctx_at("https://example.com/page", body)
    }

    fn ctx_at(url: &str, body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse(url).unwrap(),
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
    fn normalize_treats_root_slash_as_insignificant() {
        let with = url::Url::parse("https://example.com/").unwrap();
        let without = url::Url::parse("https://example.com").unwrap();
        assert_eq!(
            normalized_for_compare(&with),
            normalized_for_compare(&without)
        );
    }

    #[test]
    fn normalize_keeps_non_root_trailing_slash_significant() {
        let with = url::Url::parse("https://example.com/page/").unwrap();
        let without = url::Url::parse("https://example.com/page").unwrap();
        assert_ne!(
            normalized_for_compare(&with),
            normalized_for_compare(&without)
        );
    }

    #[test]
    fn normalize_keeps_www_and_apex_hosts_distinct() {
        let www = url::Url::parse("https://WWW.Example.COM/Path").unwrap();
        let bare = url::Url::parse("https://example.com/Path").unwrap();
        assert_ne!(normalized_for_compare(&www), normalized_for_compare(&bare));
    }

    #[test]
    fn normalize_keeps_path_case_significant() {
        // Lowercasing the path hid real mismatches.
        let upper = url::Url::parse("https://example.com/Page").unwrap();
        let lower = url::Url::parse("https://example.com/page").unwrap();
        assert_ne!(
            normalized_for_compare(&upper),
            normalized_for_compare(&lower)
        );
    }

    #[test]
    fn canonical_match_passes() {
        let body = r#"<link rel="canonical" href="https://example.com/page">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn canonical_href_before_rel_ordering_is_detected() {
        // Exercises CANONICAL_RE2 (href attribute precedes rel).
        let body = r#"<link href="https://example.com/page" rel="canonical">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn no_canonical_tag_returns_no_result() {
        let results = CanonicalMismatchCheck.run(&ctx("<html></html>"));
        assert!(results.is_empty());
    }

    #[test]
    fn relative_canonical_matching_page_passes() {
        let body = r#"<link rel="canonical" href="/page">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn relative_canonical_pointing_elsewhere_warns() {
        let body = r#"<link rel="canonical" href="/other">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
    }

    #[test]
    fn canonical_path_case_difference_warns() {
        let body = r#"<link rel="canonical" href="https://example.com/PAGE">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn canonical_www_variant_is_a_distinct_host_and_needs_review() {
        let body = r#"<link rel="canonical" href="https://www.example.com/page">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn canonical_same_domain_mismatch_warns_medium() {
        let body = r#"<link rel="canonical" href="https://example.com/other">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
    }

    #[test]
    fn canonical_cross_domain_target_warns_medium_pending_intent_review() {
        let body = r#"<link rel="canonical" href="https://other.com/page">"#;
        let results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn canonical_evidence_redacts_query_values_and_ignores_script_examples() {
        let body = r#"<script>const example = '<link rel=canonical href=https://wrong.test/>';</script>
            <link rel=canonical href="https://example.com/other?token=secret#fragment">"#;
        let result = CanonicalMismatchCheck.run(&ctx(body)).remove(0);
        assert_eq!(result.status, CheckStatus::Warn);
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains("/other"));
        assert!(!serialized.contains("wrong.test"));
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("fragment"));
    }

    #[test]
    fn multiple_html_canonical_declarations_are_reported_even_if_first_matches() {
        let body = r#"<link rel=canonical href=https://example.com/page>
            <link rel=canonical href=https://example.com/other>"#;
        let result = CanonicalMismatchCheck.run(&ctx(body)).remove(0);
        assert_eq!(result.status, CheckStatus::Warn);
        assert!(result.title.contains("Multiple"));
        assert_eq!(result.raw_data.as_ref().unwrap()["html_canonical_count"], 2);
    }

    #[test]
    fn canonical_cross_domain_on_localhost_is_skipped() {
        let mut c = ctx_at(
            "http://localhost:3000/page",
            r#"<link rel="canonical" href="https://prod.example.com/page">"#,
        );
        c.is_localhost = true;
        let results = CanonicalMismatchCheck.run(&c);
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn meta_robots_index_and_noindex_conflict_fails() {
        let body = r#"<meta name="robots" content="index, noindex">"#;
        let results = MetaRobotsConflictCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::Medium);
    }

    #[test]
    fn meta_robots_follow_and_nofollow_conflict_fails() {
        let body = r#"<meta name="robots" content="follow, nofollow">"#;
        let results = MetaRobotsConflictCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Fail);
    }

    #[test]
    fn meta_robots_multiple_consistent_tags_pass() {
        let body = r#"<meta name="robots" content="index"><meta name="robots" content="follow">"#;
        let results = MetaRobotsConflictCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(
            !results[0].title.contains("Conflicting"),
            "consistent tags must not be titled Conflicting: {}",
            results[0].title
        );
        assert!(results[0].description.contains("combine"));
    }

    #[test]
    fn meta_robots_conflict_across_multiple_tags_still_fails() {
        let body = r#"<meta name="robots" content="index"><meta name="robots" content="noindex">"#;
        let results = MetaRobotsConflictCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert!(results[0].title.contains("Contradictory"));
    }

    #[test]
    fn minified_unquoted_canonical_and_robots_are_parsed() {
        let body =
            "<link rel=canonical href=https://example.com/page><meta name=robots content=noindex>";
        let canonical_results = CanonicalMismatchCheck.run(&ctx(body));
        assert_eq!(canonical_results[0].status, CheckStatus::Pass);

        let robots_results = MetaRobotsConflictCheck.run(&ctx(body));
        assert_eq!(robots_results[0].status, CheckStatus::Pass);
        assert!(robots_results[0].description.contains("noindex"));
    }

    #[test]
    fn meta_robots_single_directive_passes() {
        let body = r#"<meta name="robots" content="noindex">"#;
        let results = MetaRobotsConflictCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn meta_robots_absent_passes_default() {
        let results = MetaRobotsConflictCheck.run(&ctx("<html></html>"));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn meta_robots_x_robots_tag_header_is_read() {
        // Directives can arrive via the X-Robots-Tag header, not just the tag.
        let mut c = ctx("<html></html>");
        c.response_headers
            .insert("x-robots-tag", "index, noindex".parse().unwrap());
        let results = MetaRobotsConflictCheck.run(&c);
        assert_eq!(results[0].status, CheckStatus::Fail);
    }

    #[test]
    fn robots_markup_examples_in_inert_content_are_ignored() {
        let body = r#"<!-- <meta name=robots content="index,noindex"> -->
            <script>const example = '<meta name=robots content="follow,nofollow">';</script>"#;
        let result = &MetaRobotsConflictCheck.run(&ctx(body))[0];
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("No supported page-level"));
        assert!(!result.description.contains("default crawling behavior"));
    }

    #[test]
    fn none_shorthand_conflicts_with_index_in_the_same_scope() {
        let result =
            &MetaRobotsConflictCheck.run(&ctx(r#"<meta name=robots content="all, none">"#))[0];
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.description.contains("index and noindex"));
        assert!(result.description.contains("follow and nofollow"));
    }

    #[test]
    fn different_crawler_scopes_are_not_merged_into_a_false_conflict() {
        let mut c = ctx(r#"<meta name=robots content=index>"#);
        c.response_headers
            .append("x-robots-tag", "googlebot: noindex".parse().unwrap());
        c.response_headers
            .append("x-robots-tag", "bingbot: follow".parse().unwrap());
        let result = &MetaRobotsConflictCheck.run(&c)[0];
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("same crawler scope"));
    }

    #[test]
    fn repeated_header_values_are_all_compared_within_their_scope() {
        let mut c = ctx("<html></html>");
        c.response_headers
            .append("x-robots-tag", "googlebot: index".parse().unwrap());
        c.response_headers
            .append("x-robots-tag", "googlebot: noindex".parse().unwrap());
        let result = &MetaRobotsConflictCheck.run(&c)[0];
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.description.contains("Googlebot scope"));
        assert_eq!(result.raw_data.as_ref().unwrap()["header_value_count"], 2);
    }
}
