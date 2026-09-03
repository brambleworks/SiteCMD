//! Cross-page analysis over collected page signals, with one optional sitemap fetch.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::core::page_signals::{normalize_page_url, PageSignals};
use std::collections::{HashMap, HashSet};

mod duplicates;
use duplicates::duplicate_field;

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

/// The sitemap URL set handed to session analysis, plus whether SiteCMD can
/// claim it read all of it.
#[derive(Debug, Clone, Copy)]
pub struct SessionSitemap<'a> {
    pub urls: &'a [String],
    /// Set when the URL set is known to be partial, naming why. A check that
    /// found nothing in a partial set has not established a clean verdict.
    pub partial_because: Option<&'a str>,
}

/// The pages a cross-page comparison may actually use, and what was left out.
///
/// Two selected URLs can land on one page (a trailing-slash twin, or a stale
/// sitemap entry that now redirects), and a dead URL still returns a body. A
/// comparison that counted either would report a page as its own duplicate or
/// grade a site's error template instead of its pages.
struct ComparedPages<'a> {
    pages: Vec<&'a PageSignals>,
    /// Every URL the scan asked for that landed on a given compared page,
    /// keyed by that page's effective URL and always including it. Dropping a
    /// repeated selection must not drop the URL it was reached through, or a
    /// page linked only through that URL becomes a false orphan.
    aliases: HashMap<&'a str, HashSet<&'a str>>,
    /// The compared page the scan started from, when it was comparable. Orphan
    /// analysis exempts it, and cannot run honestly without it.
    entry_page: Option<&'a str>,
    selected: usize,
    without_successful_response: usize,
    repeated_pages: usize,
}

impl<'a> ComparedPages<'a> {
    fn from_signals(pages: &'a [PageSignals]) -> Self {
        let mut compared: Vec<&PageSignals> = Vec::new();
        let mut aliases: HashMap<&str, HashSet<&str>> = HashMap::new();
        let mut entry_page = None;
        let mut without_successful_response = 0;
        let mut repeated_pages = 0;
        for (position, page) in pages.iter().enumerate() {
            // A page that never answered at all contributes no signals to this
            // analysis, so only a response that is not a page reaches here: an
            // error body, or a redirect that never resolved to a document.
            if !(200..300).contains(&page.status_code) {
                without_successful_response += 1;
                continue;
            }
            // `PageSignals.url` is normalized once, at extraction, so it is
            // already the cross-page identity.
            let known = aliases.entry(page.url.as_str()).or_default();
            let first_selection = known.is_empty();
            known.insert(page.url.as_str());
            known.insert(page.requested_url.as_str());
            if !first_selection {
                repeated_pages += 1;
                continue;
            }
            if position == 0 {
                entry_page = Some(page.url.as_str());
            }
            compared.push(page);
        }
        Self {
            pages: compared,
            aliases,
            entry_page,
            selected: pages.len(),
            without_successful_response,
            repeated_pages,
        }
    }

    /// Every URL a compared page answers to: the one it landed on plus every
    /// selected URL that reached it.
    fn aliases_of(&self, page: &PageSignals) -> impl Iterator<Item = &str> {
        self.aliases
            .get(page.url.as_str())
            .into_iter()
            .flat_map(|urls| urls.iter().copied())
    }

    /// How the outcome copy names what was actually compared.
    fn compared_phrase(&self) -> String {
        if self.excluded() == 0 {
            format!("all {} scanned pages", self.pages.len())
        } else {
            format!(
                "{} of the {} selected URLs",
                self.pages.len(),
                self.selected
            )
        }
    }

    fn excluded(&self) -> usize {
        self.without_successful_response + self.repeated_pages
    }

    /// The sentence appended to every session outcome when the compared set is
    /// smaller than the selected set. Empty when nothing was left out.
    fn exclusion_note(&self) -> String {
        let mut reasons = Vec::new();
        if self.without_successful_response > 0 {
            reasons.push(format!(
                "{} with no successful page response",
                self.without_successful_response
            ));
        }
        if self.repeated_pages > 0 {
            reasons.push(format!(
                "{} that resolved to a page already compared",
                self.repeated_pages
            ));
        }
        if reasons.is_empty() {
            return String::new();
        }
        format!(" Not compared: {}.", reasons.join(" and "))
    }
}

/// Run all session-level analyzers. `sitemap` should be the discovered sitemap
/// URL set when available (used only by the noindex contradiction check). Pure
/// and synchronous: the caller does any fetching.
pub fn analyze_session(
    pages: &[PageSignals],
    sitemap: Option<SessionSitemap<'_>>,
) -> Vec<CheckResult> {
    if pages.len() < 2 {
        return Vec::new();
    }
    let compared = ComparedPages::from_signals(pages);
    let comparable = compared.pages.as_slice();

    let mut results = Vec::new();
    results.extend(duplicate_field(
        comparable,
        &compared,
        "seo.duplicate_title_across_pages",
        "title",
        |p| p.title.as_deref(),
    ));
    results.extend(duplicate_field(
        comparable,
        &compared,
        "seo.duplicate_description_across_pages",
        "meta description",
        |p| p.meta_description.as_deref(),
    ));
    results.extend(duplicate_field(
        comparable,
        &compared,
        "seo.duplicate_h1",
        "H1 heading",
        |p| p.h1.as_deref(),
    ));
    results.extend(orphan_pages(comparable, &compared));
    if let Some(sitemap) = sitemap {
        results.extend(noindex_in_sitemap(comparable, &compared, sitemap));
    }
    results.extend(canonical_loops(comparable, &compared));
    results.extend(hreflang_reciprocity(comparable, &compared));
    results.extend(unreported_outcomes(&results, &compared, sitemap));
    results
}

/// Produces `Pass` only when the full set was evaluated; missing inputs produce
/// `Skipped` so coverage cannot resolve an earlier finding.
fn unreported_outcomes(
    reported: &[CheckResult],
    compared: &ComparedPages<'_>,
    sitemap: Option<SessionSitemap<'_>>,
) -> Vec<CheckResult> {
    let comparable = compared.pages.len();
    let note = compared.exclusion_note();
    let phrase = compared.compared_phrase();
    SESSION_CHECK_IDS
        .iter()
        .filter(|check_id| {
            !reported
                .iter()
                .any(|result| result.check_id.as_str() == **check_id)
        })
        .map(|check_id| {
            let subject = session_check_subject(check_id);
            // Too few comparable pages is a missing input for every check, so
            // it is decided before the per-check reasons.
            let skipped_because = if comparable < 2 {
                Some(format!(
                    "A cross-page comparison needs at least two comparable pages; this scan produced {comparable}.{note}"
                ))
            } else {
                match *check_id {
                "seo.orphan_pages" if compared.entry_page.is_none() => Some(
                    "The page this scan started from did not return a successful page response, so the links it publishes are missing from the graph and every page it links to would look unreachable.".to_string(),
                ),
                "seo.orphan_pages" if comparable < ORPHAN_MIN_PAGES => Some(format!(
                    "This check needs at least {ORPHAN_MIN_PAGES} scanned pages to tell an unlinked page from a small scan; this scan compared {comparable}.{note}"
                )),
                "seo.noindex_in_sitemap" if sitemap.is_none() => Some(
                    "No sitemap was found for this site, so no page could be compared against one."
                        .to_string(),
                ),
                "seo.noindex_in_sitemap" => sitemap
                    .and_then(|sitemap| sitemap.partial_because)
                    .map(|reason| format!(
                        "SiteCMD read only part of this site's sitemap ({reason}), so a noindex page listed in the part it did not read would not have been seen."
                    )),
                _ => None,
                }
            };
            let mut result = base_result(
                check_id,
                match &skipped_because {
                    Some(_) => format!("{subject} were not checked"),
                    None => format!("No {subject} across the scanned pages"),
                },
                skipped_because.clone().unwrap_or_else(|| {
                    format!("SiteCMD compared {phrase} and found no {subject}.{note}")
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

/// Pages in the scan set that no other scanned page links to.
fn orphan_pages(pages: &[&PageSignals], compared: &ComparedPages<'_>) -> Vec<CheckResult> {
    if pages.len() < ORPHAN_MIN_PAGES {
        return Vec::new();
    }
    // Without the entry page's own links the graph is missing the page most
    // others hang off, so an orphan list built from what is left would be
    // guesswork. `unreported_outcomes` reports that as Skipped with a reason.
    let Some(entry_page) = compared.entry_page else {
        return Vec::new();
    };

    // A page answers to every URL that reached it, not only the one it landed
    // on: a link written to a pre-redirect URL, or to the URL a second
    // selection used, is a real inbound link. The same aliases exclude a
    // page's link to itself.
    let mut linked: HashSet<&str> = HashSet::new();
    for page in pages {
        let own: HashSet<&str> = compared.aliases_of(page).collect();
        for link in &page.internal_links {
            if !own.contains(link.as_str()) {
                linked.insert(link.as_str());
            }
        }
    }

    // Nothing needs to link to the page the scan started from (usually the
    // homepage) for it to be reachable.
    let orphans: Vec<&str> = pages
        .iter()
        .filter(|p| p.url != entry_page)
        .filter(|p| !compared.aliases_of(p).any(|alias| linked.contains(alias)))
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
            "In the bounded graph of {} compared pages, {} page{} received no link from an initial-HTML <a href> on another scanned page, at any of the URLs that reached it: {}{}. The scan entry page is excluded. This does not establish that a URL is unreachable: links may exist on unscanned pages, in a rendered or conditional state, through external sources, or beyond the per-page collection cap{}.{}",
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
            compared.exclusion_note(),
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
        "every_url_that_reached_a_page_counts_as_inbound": true,
        "same_site_scheme_and_www_twins_count_as_inbound": true,
        "rendered_dom_inspected": false,
        "unscanned_pages_inspected": false,
        "per_page_link_cap": crate::core::page_signals::MAX_INTERNAL_LINKS_PER_PAGE,
        "link_collection_truncated": links_truncated,
        "pages_compared": compared.pages.len(),
        "selected_urls": compared.selected,
        "selected_urls_without_a_successful_response": compared.without_successful_response,
        "selected_urls_resolving_to_an_already_compared_page": compared.repeated_pages,
    }));
    vec![result]
}

/// Pages that are noindex AND listed in the sitemap: the sitemap invites
/// crawlers to a page that then tells them to go away.
fn noindex_in_sitemap(
    pages: &[&PageSignals],
    compared: &ComparedPages<'_>,
    sitemap: SessionSitemap<'_>,
) -> Vec<CheckResult> {
    let sitemap_set: HashSet<String> = sitemap.urls.iter().map(|u| normalize_page_url(u)).collect();

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
            "These scanned pages had a noindex directive in the initial HTML or final X-Robots-Tag response and also appeared in the supplied sitemap URL set: {}{}. That is a discovery/indexing contradiction whose intent needs review; it can reflect an accidental noindex, a stale sitemap, or a deliberate URL that remains discoverable for another reason.{}{}",
            safe_contradictions.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
            if contradictions.len() > 5 {
                format!(" and {} more", contradictions.len() - 5)
            } else {
                String::new()
            },
            match sitemap.partial_because {
                Some(reason) => format!(" SiteCMD read only part of this site's sitemap ({reason}), so there may be more."),
                None => String::new(),
            },
            compared.exclusion_note(),
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
        "sitemap_url_set_complete": sitemap.partial_because.is_none(),
        "indexing_state_verified": false,
        "pages_compared": compared.pages.len(),
        "selected_urls": compared.selected,
        "selected_urls_without_a_successful_response": compared.without_successful_response,
        "selected_urls_resolving_to_an_already_compared_page": compared.repeated_pages,
    }));
    vec![result]
}

/// Canonical relationships that loop or chain between scanned pages.
fn canonical_loops(pages: &[&PageSignals], compared: &ComparedPages<'_>) -> Vec<CheckResult> {
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
            "The first parsed initial-HTML canonical on the scanned pages forms {}. A cycle has no terminal target within the observed set; an indirect chain can still settle on a final target but asks a consumer to follow more than one canonical relationship. This analysis did not inspect HTTP Link headers, duplicate canonical declarations, target status/indexability, content equivalence, or canonical selection by a search engine.{}",
            findings.join(". "),
            compared.exclusion_note(),
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
fn hreflang_reciprocity(pages: &[&PageSignals], compared: &ComparedPages<'_>) -> Vec<CheckResult> {
    let by_url: HashMap<&str, &PageSignals> = pages.iter().map(|p| (p.url.as_str(), *p)).collect();

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
            "These scanned pages contain an initial-HTML hreflang alternate whose scanned target has no parsed initial-HTML annotation back to the source URL: {}{}. Google Search documents return links for hreflang pairs and may ignore a non-returning annotation. Other consumers and annotations delivered through HTTP headers or sitemaps were not evaluated; only pairs whose targets were also scanned were checked.{}",
            missing.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
            if missing.len() > 5 {
                format!(" and {} more", missing.len() - 5)
            } else {
                String::new()
            },
            compared.exclusion_note(),
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
