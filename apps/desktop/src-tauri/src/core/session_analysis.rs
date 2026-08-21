//! Cross-page analysis over collected page signals, with one optional sitemap fetch.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::core::page_signals::{normalize_page_url, PageSignals};
use std::collections::{HashMap, HashSet};

/// Minimum scanned pages before orphan analysis says anything. On tiny scan
/// sets "no other scanned page links here" is noise, not signal.
const ORPHAN_MIN_PAGES: usize = 5;

/// Session evidence needs the path so a user can identify the page, but URL
/// credentials, query values, fragments, and token-looking path segments must
/// not be persisted in issue descriptions/raw data.
fn safe_page_url(raw: &str) -> String {
    crate::log_sanitizer::evidence_safe_page_url(raw)
}

/// Ordered session checks used to prove complete outcome coverage.
pub const SESSION_CHECK_IDS: &[&str] = &[
    "seo.duplicate_title_across_pages",
    "seo.duplicate_description_across_pages",
    "seo.duplicate_h1",
    "seo.orphan_pages",
    "seo.noindex_in_sitemap",
    "seo.canonical_loop",
    "seo.hreflang_reciprocity",
];

/// Run all session-level analyzers. `sitemap_urls` should be the discovered
/// sitemap URL list when available (used only by the noindex contradiction
/// check). Pure and synchronous: the caller does any fetching.
pub fn analyze_session(pages: &[PageSignals], sitemap_urls: Option<&[String]>) -> Vec<CheckResult> {
    if pages.len() < 2 {
        return Vec::new();
    }

    let mut results = Vec::new();
    results.extend(duplicate_field(
        pages,
        "seo.duplicate_title_across_pages",
        "title",
        |p| p.title.as_deref(),
    ));
    results.extend(duplicate_field(
        pages,
        "seo.duplicate_description_across_pages",
        "meta description",
        |p| p.meta_description.as_deref(),
    ));
    results.extend(duplicate_field(
        pages,
        "seo.duplicate_h1",
        "H1 heading",
        |p| p.h1.as_deref(),
    ));
    results.extend(orphan_pages(pages));
    if let Some(sitemap_urls) = sitemap_urls {
        results.extend(noindex_in_sitemap(pages, sitemap_urls));
    }
    results.extend(canonical_loops(pages));
    results.extend(hreflang_reciprocity(pages));
    results.extend(unreported_outcomes(&results, pages, sitemap_urls));
    results
}

/// Produces `Pass` only when the full set was evaluated; missing inputs produce
/// `Skipped` so coverage cannot resolve an earlier finding.
fn unreported_outcomes(
    reported: &[CheckResult],
    pages: &[PageSignals],
    sitemap_urls: Option<&[String]>,
) -> Vec<CheckResult> {
    SESSION_CHECK_IDS
        .iter()
        .filter(|check_id| {
            !reported
                .iter()
                .any(|result| result.check_id.as_str() == **check_id)
        })
        .map(|check_id| {
            let subject = session_check_subject(check_id);
            let skipped_because = match *check_id {
                "seo.orphan_pages" if pages.len() < ORPHAN_MIN_PAGES => Some(format!(
                    "This check needs at least {ORPHAN_MIN_PAGES} scanned pages to tell an unlinked page from a small scan; this scan covered {}.",
                    pages.len()
                )),
                "seo.noindex_in_sitemap" if sitemap_urls.is_none() => Some(
                    "No sitemap was found for this site, so no page could be compared against one."
                        .to_string(),
                ),
                _ => None,
            };
            let mut result = base_result(
                check_id,
                match &skipped_because {
                    Some(_) => format!("{subject} were not checked"),
                    None => format!("No {subject} across the scanned pages"),
                },
                skipped_because.clone().unwrap_or_else(|| {
                    format!(
                        "SiteCMD compared all {} scanned pages and found no {subject}.",
                        pages.len()
                    )
                }),
            );
            result.status = match skipped_because {
                Some(_) => CheckStatus::Skipped,
                None => CheckStatus::Pass,
            };
            result.confidence_reason = None;
            result
        })
        .collect()
}

/// What each session check is about, in the words a person reads. A row that
/// named its check id would put a developer's identifier in front of the
/// people who see these results.
fn session_check_subject(check_id: &str) -> &str {
    match check_id {
        "seo.duplicate_title_across_pages" => "shared page titles",
        "seo.duplicate_description_across_pages" => "shared meta descriptions",
        "seo.duplicate_h1" => "shared H1 headings",
        "seo.orphan_pages" => "pages without an inbound link",
        "seo.noindex_in_sitemap" => "noindex pages listed in the sitemap",
        "seo.canonical_loop" => "canonical loops",
        "seo.hreflang_reciprocity" => "one-way hreflang pairs",
        other => other,
    }
}

fn base_result(check_id: &str, title: String, description: String) -> CheckResult {
    CheckResult {
        check_id: check_id.to_string(),
        category: ScanCategory::Seo,
        title,
        description,
        status: CheckStatus::Warn,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Group pages by a normalized text field and report groups sharing a value.
fn duplicate_field<'a>(
    pages: &'a [PageSignals],
    check_id: &str,
    label: &str,
    field: impl Fn(&'a PageSignals) -> Option<&'a str>,
) -> Vec<CheckResult> {
    let mut groups: HashMap<String, Vec<&str>> = HashMap::new();
    for page in pages {
        if let Some(value) = field(page) {
            let key = value.trim().to_ascii_lowercase();
            if !key.is_empty() {
                groups.entry(key).or_default().push(page.url.as_str());
            }
        }
    }

    let mut duplicated: Vec<(&String, &Vec<&str>)> =
        groups.iter().filter(|(_, urls)| urls.len() >= 2).collect();
    if duplicated.is_empty() {
        return Vec::new();
    }
    duplicated.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));

    let total_pages: usize = duplicated.iter().map(|(_, urls)| urls.len()).sum();
    let preview = duplicated
        .iter()
        .take(3)
        .map(|(value, urls)| {
            format!(
                "\"{}\" ({} pages)",
                crate::log_sanitizer::redact_secrets(&truncate(value, 70)),
                urls.len()
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    let evidence_scope = if label == "H1 heading" {
        "first non-empty H1 text extracted from each initial HTML response"
    } else if label == "title" {
        "document-title text extracted from each initial HTML response"
    } else {
        "meta-description content extracted from each initial HTML response"
    };

    let mut result = base_result(
        check_id,
        format!(
            "{} pages share the same {}",
            total_pages,
            label.to_ascii_lowercase()
        ),
        format!(
            "{} scanned pages share the same normalized {} with at least one other page: {}{}. The comparison uses the {}. It does not establish that the pages have the same purpose, that the value is inappropriate, or that a search engine will select a particular snippet or page.",
            total_pages,
            label,
            preview,
            if duplicated.len() > 3 {
                format!(" and {} more group(s)", duplicated.len() - 3)
            } else {
                String::new()
            },
            evidence_scope,
        ),
    );
    result.severity = if check_id == "seo.duplicate_h1" {
        Severity::Low
    } else {
        Severity::Medium
    };
    result.confidence = IssueConfidence::NeedsReview;
    result.confidence_reason = Some(
        "The repeated text is directly observed within this scanned set, but page intent, canonical relationships, localization, pagination, rendered metadata, and search-engine selection were not evaluated."
            .into(),
    );
    result.manual_fix = Some(match label {
        "H1 heading" => "Review whether the surfaced pages have distinct primary topics. If they do, give each page a visible page-level heading that describes its own content; preserve a shared heading when the pages intentionally present the same topic or state. Check the rendered heading structure rather than changing text solely to make it unique.".into(),
        "title" => "First distinguish duplicate <title> elements on one page from one effective title repeated across scanned pages. Consolidate same-page declarations to one authoritative metadata source. For genuinely distinct indexable pages, give each an accurate page-specific title; preserve intentional repetition for equivalent, paginated, localized, or application states when that matches the content and canonical strategy.".into(),
        _ => "First distinguish duplicate meta-description elements on one page from one effective description repeated across scanned pages. Consolidate conflicting same-page declarations. For distinct indexable pages where a description is useful, write an accurate page-specific summary; search engines may still generate a different snippet from page content.".into(),
    });
    result.why_it_matters = Some(match label {
        "H1 heading" => "If pages have different primary topics, repeating the same visible page heading can make those pages harder for visitors and assistive-technology users to distinguish. Repetition is not automatically a defect.".into(),
        "title" => "If distinct pages use the same document title, browser tabs, bookmarks, assistive technology, and search-result candidates can be harder to distinguish; search engines may rewrite title links.".into(),
        _ => "If distinct pages use the same intended summary, their search-result candidates may be less differentiated; search engines can ignore or rewrite meta descriptions.".into(),
    });
    result.raw_data = Some(serde_json::json!({
        "groups": duplicated
            .iter()
            .take(10)
            .map(|(value, urls)| serde_json::json!({
                "value": crate::log_sanitizer::redact_secrets(&truncate(value, 200)),
                "pages": urls.iter().map(|url| safe_page_url(url)).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
        "group_count": duplicated.len(),
        "comparison_scope": evidence_scope,
        "rendered_dom_inspected": false,
        "canonical_relationships_considered": false,
    }));
    vec![result]
}

/// Pages in the scan set that no other scanned page links to.
fn orphan_pages(pages: &[PageSignals]) -> Vec<CheckResult> {
    if pages.len() < ORPHAN_MIN_PAGES {
        return Vec::new();
    }

    let mut linked: HashSet<&str> = HashSet::new();
    for page in pages {
        for link in &page.internal_links {
            if link != &page.url {
                linked.insert(link.as_str());
            }
        }
    }

    // The first URL is the scan entry point (usually the homepage); nothing
    // needs to link to it for it to be reachable.
    let orphans: Vec<&str> = pages
        .iter()
        .skip(1)
        .filter(|p| !linked.contains(p.url.as_str()))
        .map(|p| p.url.as_str())
        .collect();

    if orphans.is_empty() {
        return Vec::new();
    }
    let safe_orphans: Vec<String> = orphans.iter().map(|url| safe_page_url(url)).collect();
    let links_truncated = pages.iter().any(|page| page.internal_links_truncated);

    let mut result = base_result(
        "seo.orphan_pages",
        format!(
            "{} page{} not linked from any other scanned page",
            orphans.len(),
            if orphans.len() == 1 { " is" } else { "s are" }
        ),
        format!(
            "In the bounded graph of {} scanned pages, {} page{} received no link from an initial-HTML <a href> on another scanned page: {}{}. The scan entry page is excluded. This does not establish that a URL is unreachable: links may exist on unscanned pages, in a rendered or conditional state, through external sources, or beyond the per-page collection cap{}.",
            pages.len(),
            orphans.len(),
            if orphans.len() == 1 { "" } else { "s" },
            safe_orphans.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
            if orphans.len() > 5 {
                format!(" and {} more", orphans.len() - 5)
            } else {
                String::new()
            },
            if links_truncated { " on at least one page" } else { "" },
        ),
    );
    result.severity = Severity::Low;
    result.confidence = IssueConfidence::NeedsReview;
    result.confidence_reason = Some(
        "Only initial-HTML anchor links between scanned pages were evaluated; rendered navigation, unscanned pages, external links, direct visits, authentication states, and any links beyond the collection cap remain outside this evidence.".into(),
    );
    result.manual_fix = Some(
        "Review each surfaced URL in the full production crawl and rendered navigation before changing it. If people and crawlers should discover the page through the site, add a useful crawlable link from a relevant page or shared navigation. If it is deliberately unlisted, confirm that its sitemap, canonical, noindex, access-control, campaign, and post-action behavior match that intent; sitemap inclusion and indexability are separate decisions.".into(),
    );
    result.why_it_matters = Some(
        "If a useful page truly has no crawlable path from the site's navigation graph, visitors and link-following crawlers may be less likely to discover it. This bounded scan does not prove that condition.".into(),
    );
    result.raw_data = Some(serde_json::json!({
        "pages_without_observed_inbound_link": safe_orphans,
        "scanned_pages": pages.len(),
        "scan_entry_excluded": true,
        "initial_html_anchor_links_only": true,
        "rendered_dom_inspected": false,
        "unscanned_pages_inspected": false,
        "per_page_link_cap": crate::core::page_signals::MAX_INTERNAL_LINKS_PER_PAGE,
        "link_collection_truncated": links_truncated,
    }));
    vec![result]
}

/// Pages that are noindex AND listed in the sitemap: the sitemap invites
/// crawlers to a page that then tells them to go away.
fn noindex_in_sitemap(pages: &[PageSignals], sitemap_urls: &[String]) -> Vec<CheckResult> {
    let sitemap_set: HashSet<String> = sitemap_urls.iter().map(|u| normalize_page_url(u)).collect();

    let contradictions: Vec<&str> = pages
        .iter()
        .filter(|p| p.noindex && sitemap_set.contains(&p.url))
        .map(|p| p.url.as_str())
        .collect();

    if contradictions.is_empty() {
        return Vec::new();
    }

    let safe_contradictions: Vec<String> = contradictions
        .iter()
        .map(|url| safe_page_url(url))
        .collect();
    let mut result = base_result(
        "seo.noindex_in_sitemap",
        format!(
            "{} noindex page{} listed in the sitemap",
            contradictions.len(),
            if contradictions.len() == 1 { " is" } else { "s are" }
        ),
        format!(
            "These scanned pages had a noindex directive in the initial HTML or final X-Robots-Tag response and also appeared in the supplied sitemap URL set: {}{}. That is a discovery/indexing contradiction whose intent needs review; it can reflect an accidental noindex, a stale sitemap, or a deliberate URL that remains discoverable for another reason.",
            safe_contradictions.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
            if contradictions.len() > 5 {
                format!(" and {} more", contradictions.len() - 5)
            } else {
                String::new()
            },
        ),
    );
    result.confidence = IssueConfidence::NeedsReview;
    result.confidence_reason = Some(
        "The noindex and sitemap membership are directly observed for this scan, but URL intent, other sitemap sources, canonical selection, later response changes, and target-crawler processing were not evaluated."
            .into(),
    );
    result.manual_fix = Some(
        "Decide the intended state for each URL. If it should be eligible for indexing, remove every unintended noindex source and verify status, canonical, robots access, internal links, and rendered content. If exclusion is intentional, remove it from index-oriented sitemap output unless a documented consumer or workflow requires it there. Regenerate and fetch the deployed sitemap, then verify the final HTML and X-Robots-Tag response.".into(),
    );
    result.why_it_matters = Some(
        "A noindex URL in a sitemap sends inconsistent maintenance signals and may reveal either accidental exclusion or stale discovery data. It does not by itself prove crawl waste or a particular indexing outcome.".into(),
    );
    result.raw_data = Some(serde_json::json!({
        "pages": safe_contradictions,
        "noindex_sources_checked": ["initial_html_meta", "final_x_robots_tag"],
        "sitemap_url_set_supplied": true,
        "indexing_state_verified": false,
    }));
    vec![result]
}

/// Canonical relationships that loop or chain between scanned pages.
fn canonical_loops(pages: &[PageSignals]) -> Vec<CheckResult> {
    let canon: HashMap<&str, &str> = pages
        .iter()
        .filter_map(|p| {
            p.canonical
                .as_deref()
                .filter(|c| *c != p.url)
                .map(|c| (p.url.as_str(), c))
        })
        .collect();

    if canon.is_empty() {
        return Vec::new();
    }

    // Find cycles of any length. Canonicalize each cycle by rotating it to
    // its lexicographically smallest URL so walking from every member still
    // emits one piece of evidence.
    let mut loop_keys = std::collections::BTreeSet::new();
    for start in canon.keys().copied() {
        let mut positions: HashMap<&str, usize> = HashMap::new();
        let mut path: Vec<&str> = Vec::new();
        let mut current = start;
        loop {
            if let Some(position) = positions.get(current).copied() {
                let cycle = &path[position..];
                if !cycle.is_empty() {
                    let minimum = cycle
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, url)| *url)
                        .map(|(index, _)| index)
                        .unwrap_or(0);
                    let rotated = cycle[minimum..]
                        .iter()
                        .chain(cycle[..minimum].iter())
                        .copied()
                        .collect::<Vec<_>>();
                    loop_keys.insert(rotated.join("\u{0}"));
                }
                break;
            }
            positions.insert(current, path.len());
            path.push(current);
            let Some(next) = canon.get(current).copied() else {
                break;
            };
            current = next;
        }
    }
    let loops: Vec<String> = loop_keys
        .iter()
        .map(|key| {
            let nodes = key.split('\0').collect::<Vec<_>>();
            nodes
                .iter()
                .chain(nodes.first())
                .map(|url| safe_page_url(url))
                .collect::<Vec<_>>()
                .join(" -> ")
        })
        .collect();

    // Report maximal indirect chains from graph roots. A direct A -> B
    // canonical is not a chain; A -> B -> C is.
    let incoming: HashSet<&str> = canon.values().copied().collect();
    let mut chains = Vec::new();
    for start in canon
        .keys()
        .copied()
        .filter(|candidate| !incoming.contains(candidate))
    {
        let mut seen = HashSet::new();
        let mut path = vec![start];
        let mut current = start;
        seen.insert(start);
        while let Some(next) = canon.get(current).copied() {
            path.push(next);
            if !seen.insert(next) {
                break;
            }
            current = next;
        }
        if path.len() >= 3 {
            chains.push(
                path.iter()
                    .map(|url| safe_page_url(url))
                    .collect::<Vec<_>>()
                    .join(" -> "),
            );
        }
    }
    chains.sort();

    if loops.is_empty() && chains.is_empty() {
        return Vec::new();
    }

    let has_loop = !loops.is_empty();
    let mut findings = Vec::new();
    if !loops.is_empty() {
        findings.push(format!("canonical loops: {}", loops.join("; ")));
    }
    if !chains.is_empty() {
        findings.push(format!(
            "canonical chains: {}",
            chains
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }

    let mut result = base_result(
        "seo.canonical_loop",
        if has_loop {
            "Initial-HTML canonical cycle observed".into()
        } else {
            "Indirect initial-HTML canonical chain observed".into()
        },
        format!(
            "The first parsed initial-HTML canonical on the scanned pages forms {}. A cycle has no terminal target within the observed set; an indirect chain can still settle on a final target but asks a consumer to follow more than one canonical relationship. This analysis did not inspect HTTP Link headers, duplicate canonical declarations, target status/indexability, content equivalence, or canonical selection by a search engine.",
            findings.join(". "),
        ),
    );
    result.status = if has_loop {
        CheckStatus::Fail
    } else {
        CheckStatus::Warn
    };
    result.confidence = IssueConfidence::NeedsReview;
    result.confidence_reason = Some(
        "The initial-HTML relationships are directly observed, but alternate canonical sources, target responses, page equivalence, and search-engine canonical selection were not evaluated."
            .into(),
    );
    result.manual_fix = Some(
        "First confirm that the URLs are duplicate or near-duplicate representations and choose the representative that should carry their signals. Point each duplicate directly to that crawlable final representative, align redirects/internal links/sitemaps/hreflang, and give a standalone representative a coherent self-canonical where appropriate. Inspect both rendered HTML and HTTP Link headers, then verify final statuses, indexability, content equivalence, and the deployed relationship graph.".into(),
    );
    result.why_it_matters = Some(if has_loop {
        "A canonical cycle gives no consistent terminal representative in the observed HTML relationship graph. Consumers treat canonical annotations as hints and may choose a different representative.".into()
    } else {
        "An indirect canonical chain can weaken the clarity of the intended representative and is easier for templates, redirects, and sitemap signals to contradict. It does not prove an indexing failure.".into()
    });
    result.raw_data = Some(serde_json::json!({
        "loops": loops,
        "chains": chains,
        "canonical_source": "first_initial_html_link",
        "http_link_headers_inspected": false,
        "duplicate_canonical_declarations_inspected": false,
        "target_responses_inspected": false,
        "content_equivalence_verified": false,
    }));
    vec![result]
}

/// hreflang declarations between scanned pages that are not reciprocated.
fn hreflang_reciprocity(pages: &[PageSignals]) -> Vec<CheckResult> {
    let by_url: HashMap<&str, &PageSignals> = pages.iter().map(|p| (p.url.as_str(), p)).collect();

    let mut missing: Vec<String> = Vec::new();
    for page in pages {
        for (_, target_url) in &page.hreflang {
            if target_url == &page.url {
                continue;
            }
            // Only pairs where the target was scanned are verifiable.
            let Some(target) = by_url.get(target_url.as_str()) else {
                continue;
            };
            let reciprocated = target.hreflang.iter().any(|(_, href)| href == &page.url);
            if !reciprocated {
                missing.push(format!(
                    "{} -> {}",
                    safe_page_url(&page.url),
                    safe_page_url(target_url)
                ));
            }
        }
    }

    if missing.is_empty() {
        return Vec::new();
    }

    let mut result = base_result(
        "seo.hreflang_reciprocity",
        format!(
            "{} hreflang link{} not reciprocated",
            missing.len(),
            if missing.len() == 1 { " is" } else { "s are" }
        ),
        format!(
            "These scanned pages contain an initial-HTML hreflang alternate whose scanned target has no parsed initial-HTML annotation back to the source URL: {}{}. Google Search documents return links for hreflang pairs and may ignore a non-returning annotation. Other consumers and annotations delivered through HTTP headers or sitemaps were not evaluated; only pairs whose targets were also scanned were checked.",
            missing.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
            if missing.len() > 5 {
                format!(" and {} more", missing.len() - 5)
            } else {
                String::new()
            },
        ),
    );
    result.severity = Severity::Low;
    result.confidence = IssueConfidence::NeedsReview;
    result.confidence_reason = Some(
        "The missing return link is direct evidence in the parsed initial HTML of two scanned pages, but HTTP-header/sitemap annotations, target equivalence, language-code meaning, canonicals, and consumer processing were not evaluated."
            .into(),
    );
    result.manual_fix = Some(
        "Confirm that the source and target are localized equivalents. For each intended pair, emit a return annotation and a language-specific self-reference through one consistently managed channel (HTML, HTTP headers, or sitemap), using supported language/region values and fully qualified canonical final URLs. Generate the mapping from one locale source of truth, then verify both deployed responses and the target consumer's current requirements.".into(),
    );
    result.why_it_matters = Some(
        "A consumer that requires reciprocal alternate relationships may ignore a one-directional annotation, reducing the usefulness of the intended language/region mapping. This scan does not establish which version users are shown.".into(),
    );
    result.raw_data = Some(serde_json::json!({
        "missing_return_links": missing,
        "annotation_source": "initial_html_link",
        "only_scanned_targets_checked": true,
        "http_header_annotations_inspected": false,
        "sitemap_annotations_inspected": false,
        "target_equivalence_verified": false,
    }));
    vec![result]
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        let cut = crate::checks::floor_char_boundary(text, max);
        format!("{}...", &text[..cut])
    }
}

#[cfg(test)]
#[path = "session_analysis_tests.rs"]
mod tests;
