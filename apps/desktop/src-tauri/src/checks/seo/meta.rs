//! Meta, Open Graph, viewport, and charset checks.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

use sitecmd_engine::checks::seo::parsing::extract_document_title;
pub(crate) use sitecmd_engine::checks::seo::parsing::{extract_attr_value, extract_meta};

fn bounded_metadata_evidence(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_string(), false);
    }
    let cut = crate::checks::floor_char_boundary(value, max_bytes);
    (format!("{}...", &value[..cut]), true)
}

pub struct TitleTagCheck;

impl Check for TitleTagCheck {
    fn id(&self) -> &str {
        "seo.title"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let title = extract_document_title(&ctx.body).filter(|value| !value.trim().is_empty());
        match title {
            Some(t) => {
                // Character count, not byte length: a 30-character CJK title is
                // ~90 bytes, which `.len` reported as "exceeds 70 characters"
                // on every non-Latin-script site.
                let len = t.chars().count();
                // Search displays render by width and may rewrite title
                // links. Character count is therefore only an extreme-value
                // review heuristic, not proof of quality or truncation.
                let (status, result_title, desc) = if len < 20 {
                    (CheckStatus::Warn, "Short title tag needs review", format!("The title contains {} characters. Length alone does not establish quality, but a very short title may be too generic or fail to distinguish this page. Review the rendered title and page intent rather than padding it to a quota.", len))
                } else if len > 70 {
                    (CheckStatus::Warn, "Long title tag needs review", format!("The title contains {} characters. Search title links render by width and may be rewritten, so this extreme count is a review signal; it does not prove truncation or that a particular query will display the supplied title unchanged.", len))
                } else {
                    (CheckStatus::Pass, "Title tag", format!("A non-empty title is present ({} characters) and did not trigger the extreme-length heuristic. This check does not assess relevance, uniqueness across pages, rendered width, or search-engine rewrites.", len))
                };
                let (title_evidence, title_evidence_truncated) =
                    bounded_metadata_evidence(&t, 500);
                vec![CheckResult {
                    check_id: "seo.title".into(), category: ScanCategory::Seo,
                    title: result_title.into(), description: desc,
                    status, severity: Severity::High,
                    fix_prompt: None, manual_fix: if status == CheckStatus::Pass {
                        None
                    } else if len < 20 {
                        Some("Review the page intent and the title shown in the rendered document. If the wording is generic or ambiguous, replace it with a concise, page-specific label; include the brand only when it helps users distinguish the result. Do not add filler solely to reach a character target.".into())
                    } else {
                        Some("Review the title's wording, rendered width, and representative search contexts. Remove redundant or low-value text while preserving the page-specific topic; do not force a character quota or assume that shortening guarantees a particular search display.".into())
                    },
                    raw_data: Some(serde_json::json!({
                        "title": title_evidence,
                        "length": len,
                        "title_evidence_truncated": title_evidence_truncated,
                        "source": "initial_html",
                        "rendered_title_inspected": false,
                    })),
                    confidence: if status == CheckStatus::Pass { crate::checks::IssueConfidence::High } else { crate::checks::IssueConfidence::NeedsReview },
                    confidence_reason: if status == CheckStatus::Pass { None } else { Some("Character count is directly observed, but rendered width, query context, title-link rewriting, and whether the wording is sufficiently descriptive require review.".into()) },
                    why_it_matters: if status != CheckStatus::Pass { Some("If the title is generic or too wide in the actual search/browser context, users may receive a less useful or rewritten label. The count alone does not establish that outcome.".into()) } else { None },
                }]
            }
            None => vec![CheckResult {
                check_id: "seo.title".into(), category: ScanCategory::Seo,
                title: "Missing title tag".into(),
                description: "No non-empty document <title> was found in the fetched HTML. Browsers use this element for tabs and bookmarks, and search systems commonly use it as one input when generating a title link, although they may derive or rewrite that label.".into(),
                status: CheckStatus::Fail, severity: Severity::High,
                fix_prompt: None, manual_fix: Some("Add one real `<title>` tag in `<head>` that states what this page is for and, if helpful, includes the brand. Keep it specific to the page instead of reusing the site-wide name everywhere.".into()),
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some("Pages without titles are harder to understand in browser tabs, bookmarks, and search result snippets.".into()),
            }],
        }
    }
}

pub struct MetaDescriptionCheck;

impl Check for MetaDescriptionCheck {
    fn id(&self) -> &str {
        "seo.meta_description"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let desc = extract_meta(&ctx.body, "description").filter(|value| !value.trim().is_empty());
        match desc {
            Some(d) => {
                // Character count, not byte length (see seo.title): a CJK
                // description measured in bytes over-reports length ~3x.
                let len = d.chars().count();
                // Snippet length varies by query, device, and search system,
                // and the supplied description may be rewritten. Character
                // count is an editorial review heuristic only.
                let (status, result_title, msg) = if len < 50 {
                    (CheckStatus::Warn, "Short meta description needs review", format!("The meta description contains {} characters. It may be intentionally concise, but review whether it gives a useful, page-specific summary; do not pad it solely to meet a character target.", len))
                } else if len > 170 {
                    (CheckStatus::Warn, "Long meta description needs review", format!("The meta description contains {} characters. Search snippets vary by query and device and may be rewritten, so the tail may not be shown; this count does not prove truncation.", len))
                } else {
                    (CheckStatus::Pass, "Meta description", format!("A non-empty meta description is present ({} characters) and did not trigger the extreme-length heuristic. This check does not assess relevance, uniqueness, displayed snippet length, or whether a search system will use it.", len))
                };
                let (description_evidence, description_evidence_truncated) =
                    bounded_metadata_evidence(&d, 1_000);
                vec![CheckResult {
                    check_id: "seo.meta_description".into(), category: ScanCategory::Seo,
                    title: result_title.into(), description: msg,
                    status, severity: Severity::Medium,
                    fix_prompt: None, manual_fix: if status == CheckStatus::Pass {
                        None
                    } else if len < 50 {
                        Some("Review the page intent and representative search contexts. If this description is too generic to explain the page, replace it with a specific plain-language summary of what the visitor will find; add substance, not filler, and do not target a character quota.".into())
                    } else {
                        Some("Review the description in representative search contexts. Move the most useful page-specific information earlier and remove redundant wording that may not add value when a snippet is shortened; do not assume a fixed character limit or that a search system will use the supplied text.".into())
                    },
                    raw_data: Some(serde_json::json!({
                        "description": description_evidence,
                        "length": len,
                        "description_evidence_truncated": description_evidence_truncated,
                        "source": "initial_html",
                        "rendered_head_inspected": false,
                    })),
                    confidence: if status == CheckStatus::Pass { crate::checks::IssueConfidence::High } else { crate::checks::IssueConfidence::NeedsReview },
                    confidence_reason: if status == CheckStatus::Pass { None } else { Some("The length is directly observed, but usefulness, query fit, device rendering, and search-engine snippet selection are contextual.".into()) },
                    why_it_matters: if status == CheckStatus::Pass {
                        None
                    } else if len < 50 {
                        Some("An underspecified authored summary can give a consumer little page-specific context, but usefulness and click-through impact cannot be inferred from character count alone.".into())
                    } else {
                        Some("A consumer may omit the tail of a long authored summary, but displayed length, usefulness, and click-through impact cannot be inferred from character count alone.".into())
                    },
                }]
            }
            None => vec![CheckResult {
                check_id: "seo.meta_description".into(), category: ScanCategory::Seo,
                title: "Missing meta description".into(),
                description: "No non-empty meta description was found in the fetched HTML. Search systems may use a supplied description or may generate a query-dependent snippet from visible page content; absence does not mean the result has no snippet.".into(),
                status: CheckStatus::Fail, severity: Severity::Medium,
                fix_prompt: None, manual_fix: Some("Add one `<meta name=\"description\">` tag with a concise, page-specific summary when an authored default would help. Inspect the deployed HTML and representative search queries; search systems may still choose visible page text instead.".into()),
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some("Without an authored default, link/search consumers may derive a summary from page content. That can be appropriate, but it gives the site less control over the fallback wording.".into()),
            }],
        }
    }
}

pub struct CanonicalCheck;

impl Check for CanonicalCheck {
    fn id(&self) -> &str {
        "seo.canonical"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Token-aware attributes handle minified/unquoted markup without
        // accepting data-rel="canonical" or a rel token with no href.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let lower = scannable.to_ascii_lowercase();
        let has_canonical = crate::checks::html_attrs::tag_slices(&scannable, &lower, "link")
            .into_iter()
            .any(|tag| {
                crate::checks::html_attrs::attr_value(tag, "rel").is_some_and(|rel| {
                    rel.split_whitespace()
                        .any(|token| token.eq_ignore_ascii_case("canonical"))
                }) && crate::checks::html_attrs::has_attr(tag, "href")
            });
        vec![CheckResult {
            check_id: "seo.canonical".into(),
            category: ScanCategory::Seo,
            title: if has_canonical {
                "Canonical URL".into()
            } else {
                "Missing canonical URL".into()
            },
            description: if has_canonical {
                "A rel=canonical declaration is present. This presence check does not validate its href, resolve redirects, compare it with the scanned URL, or prove that a search engine will select it.".into()
            } else {
                "No rel=canonical declaration was found in the fetched HTML. Search engines can infer a canonical without the tag; an explicit declaration is most useful when this content is reachable through duplicate or parameterized URLs.".into()
            },
            status: if has_canonical {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            // A missing canonical is hygiene risk, not an indexing block.
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if has_canonical {
                None
            } else {
                Some(
                    "If this page has duplicate, parameterized, or alternate URL variants, add one canonical link in `<head>` that points to the final public URL you prefer for indexing. Verify the absolute/relative URL resolves correctly in production and is itself crawlable and suitable for indexing; do not add a self-canonical solely to silence the check when it adds no value."
                        .into(),
                )
            },
            raw_data: None,
            confidence: if has_canonical {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if has_canonical {
                None
            } else {
                Some("Absence in the fetched HTML is direct, but duplicate URL exposure, HTTP-header canonicals, framework/runtime injection, and search-engine canonical selection are outside this presence check.".into())
            },
            why_it_matters: if has_canonical {
                None
            } else {
                Some("Duplicate URL variants make it harder for crawlers to understand which page should be treated as the preferred version.".into())
            },
        }]
    }
}

pub struct ViewportCheck;

impl Check for ViewportCheck {
    fn id(&self) -> &str {
        "seo.viewport"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Parse tag attributes so quoted and unquoted minified forms agree.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let scannable_lower = scannable.to_ascii_lowercase();
        let viewport_contents: Vec<String> =
            crate::checks::html_attrs::tag_slices(&scannable, &scannable_lower, "meta")
                .into_iter()
                .filter_map(|tag| {
                    extract_attr_value(tag, "name")
                        .filter(|value| value.eq_ignore_ascii_case("viewport"))
                        .map(|_| extract_attr_value(tag, "content").unwrap_or_default())
                })
                .collect();
        let viewport_content = viewport_contents.first().cloned();
        let viewport_tag_count = viewport_contents.len();
        let has_viewport = viewport_tag_count > 0;
        let has_device_width = viewport_contents.iter().any(|content| {
            content
                .split(',')
                .map(str::trim)
                .filter_map(|directive| directive.split_once('='))
                .any(|(name, value)| {
                    name.trim().eq_ignore_ascii_case("width")
                        && value.trim().eq_ignore_ascii_case("device-width")
                })
        });
        let duplicate_viewports = viewport_tag_count > 1;
        let pass = has_device_width && !duplicate_viewports;
        vec![CheckResult {
            check_id: "seo.viewport".into(),
            category: ScanCategory::Seo,
            title: if duplicate_viewports {
                "Multiple viewport meta tags need review".into()
            } else if has_device_width {
                "Mobile viewport".into()
            } else if has_viewport {
                "Viewport configuration needs review".into()
            } else {
                "Missing viewport meta tag".into()
            },
            description: if duplicate_viewports && has_device_width {
                format!("Found {} viewport meta tags. At least one includes width=device-width, but multiple declarations can interact differently across clients and maintenance paths; consolidate to one intentional directive and test the rendered page. Presence still does not prove that the layout is mobile-friendly.", viewport_tag_count)
            } else if duplicate_viewports {
                format!("Found {} viewport meta tags, and neither includes width=device-width. Multiple declarations can interact differently across clients and maintenance paths; consolidate to one intentional directive that matches the target devices, then test the rendered page. Source markup alone does not prove that the layout is mobile-friendly or broken.", viewport_tag_count)
            } else if has_device_width {
                "A viewport declaration with width=device-width is present. This confirms the baseline device-width instruction only; it does not prove that the layout, text, controls, or responsive breakpoints are mobile-friendly.".into()
            } else if has_viewport {
                "A viewport meta tag is present, but its content does not include width=device-width. Fixed, empty, or unusual viewport rules may be intentional for a specialized document, so verify the target devices and rendered layout before changing it.".into()
            } else {
                "No viewport meta tag was observed in the initial HTML outside comments, scripts, and styles. Ordinary mobile browsers may use a wider default layout viewport, but this source observation does not prove that the rendered page is broken or that a specialized client requires the standard directive.".into()
            },
            status: if pass {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if pass {
                None
            } else {
                Some("For an ordinary responsive web page, keep exactly one `<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">` in `<head>`, remove conflicting duplicates, then test representative narrow, zoomed, large-text, and orientation states. Preserve a specialized fixed viewport only when the product explicitly requires it and the target clients are documented.".into())
            },
            raw_data: Some(serde_json::json!({
                "viewport_content": viewport_content,
                "viewport_contents": viewport_contents,
                "viewport_tag_count": viewport_tag_count,
                "has_device_width": has_device_width,
                "responsive_layout_verified": false,
            })),
            confidence: if pass {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if pass {
                None
            } else {
                Some("Viewport declaration state is directly observed in the initial HTML, but specialized client intent, runtime changes, and rendered behavior were not measured by this source check.".into())
            },
            why_it_matters: if pass {
                None
            } else {
                Some("Without an appropriate viewport for the target devices, a page can render at an unintended virtual width and require horizontal panning or excessive zoom. Actual impact must be confirmed in the rendered layout.".into())
            },
        }]
    }
}

pub struct OpenGraphCheck;

impl Check for OpenGraphCheck {
    fn id(&self) -> &str {
        "seo.open_graph"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let og_title = extract_meta(&ctx.body, "og:title").filter(|value| !value.trim().is_empty());
        let og_desc =
            extract_meta(&ctx.body, "og:description").filter(|value| !value.trim().is_empty());
        let og_image = extract_meta(&ctx.body, "og:image").filter(|value| !value.trim().is_empty());

        let mut missing = Vec::new();
        if og_title.is_none() {
            missing.push("og:title");
        }
        if og_desc.is_none() {
            missing.push("og:description");
        }
        if og_image.is_none() {
            missing.push("og:image");
        }

        vec![CheckResult {
            check_id: "seo.open_graph".into(),
            category: ScanCategory::Seo,
            title: if missing.is_empty() {
                "Open Graph tags".into()
            } else if missing.len() == 1 {
                "Missing Open Graph tag".into()
            } else {
                "Missing Open Graph tags".into()
            },
            description: if missing.is_empty() {
                "og:title, og:description, and og:image are present. These fields give compatible link-preview clients the basic metadata they need, but preview rendering still depends on image fetchability, platform rules, and cache state.".into()
            } else {
                format!(
                    "Missing Open Graph tag{}: {}. Compatible link-preview clients may fall back to the document title, description, another image, or a reduced preview when these fields are absent.",
                    if missing.len() == 1 { "" } else { "s" },
                    missing.join(", "),
                )
            },
            status: if missing.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if missing.is_empty() {
                None
            } else {
                Some("Add OG tags in <head>: <meta property=\"og:title\" content=\"...\"> <meta property=\"og:description\" content=\"...\"> <meta property=\"og:image\" content=\"...\">".into())
            },
            raw_data: Some(
                serde_json::json!({"og_title": og_title, "og_description": og_desc, "og_image": og_image}),
            ),
            confidence: if missing.is_empty() {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if missing.is_empty() {
                None
            } else {
                Some("The fields are directly absent or empty in the fetched HTML, but whether Open Graph metadata is required depends on the site's sharing channels and each client's fallback behavior.".into())
            },
            why_it_matters: if missing.is_empty() {
                None
            } else {
                Some("Open Graph fields let compatible clients choose an intentional title, summary, and image; exact rendering and caching remain platform-specific.".into())
            },
        }]
    }
}

mod charset;
mod og_absolute_urls;
mod og_resolvable;
pub use charset::MetaCharsetCheck;
pub use og_absolute_urls::OgImageAbsoluteCheck;
pub use og_resolvable::OgImageResolvableCheck;

pub struct TwitterCardCheck;

impl Check for TwitterCardCheck {
    fn id(&self) -> &str {
        "seo.twitter_cards"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let card = extract_meta(&ctx.body, "twitter:card").filter(|value| !value.trim().is_empty());
        let has_complete_og_fallback = extract_meta(&ctx.body, "og:title").is_some()
            && extract_meta(&ctx.body, "og:description").is_some()
            && extract_meta(&ctx.body, "og:image").is_some();

        vec![CheckResult {
            check_id: "seo.twitter_cards".into(),
            category: ScanCategory::Seo,
            title: if card.is_some() {
                "Twitter Card type".into()
            } else {
                "Missing Twitter Card type".into()
            },
            description: if card.is_some() {
                "A non-empty twitter:card value is present. This confirms an explicit card-type request only; it does not verify that the value is currently supported, that title/image fields are complete, or that X/Twitter will render a preview.".into()
            } else if has_complete_og_fallback {
                "No twitter:card type was found. Complete Open Graph fields are present and can supply fallback values, but an explicit card type is needed to request a particular X/Twitter card format; final rendering remains platform-controlled.".into()
            } else {
                "No twitter:card type was found, and the basic Open Graph fallback set is incomplete. X/Twitter may render a reduced link treatment or derive values from other page metadata.".into()
            },
            status: if card.is_some() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if card.is_some() {
                None
            } else {
                Some("Add an explicit <meta name=\"twitter:card\" content=\"summary\"> or content=\"summary_large_image\"> based on the image treatment you intend, and provide valid title, description, and image values through twitter:* tags or their supported Open Graph fallbacks. Verify the production URL after the platform recrawls it.".into())
            },
            raw_data: Some(serde_json::json!({
                "twitter_card": card.as_deref(),
                "has_complete_open_graph_fallback": has_complete_og_fallback,
            })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if card.is_some() {
                None
            } else {
                Some("An explicit card type documents the intended X/Twitter presentation. Open Graph can provide field-level fallbacks, but it does not guarantee a particular card layout or that a preview will render.".into())
            },
        }]
    }
}

mod noindex;
pub use noindex::NoindexCheck;

pub struct HreflangCheck;

impl Check for HreflangCheck {
    fn id(&self) -> &str {
        "seo.hreflang"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let lower = scannable.to_ascii_lowercase();
        let annotations: Vec<(String, Option<String>)> =
            crate::checks::html_attrs::tag_slices(&scannable, &lower, "link")
                .into_iter()
                .filter(|tag| {
                    crate::checks::html_attrs::attr_value(tag, "rel").is_some_and(|rel| {
                        rel.split_ascii_whitespace()
                            .any(|token| token.eq_ignore_ascii_case("alternate"))
                    })
                })
                .filter_map(|tag| {
                    crate::checks::html_attrs::attr_value(tag, "hreflang").map(|language| {
                        (
                            language.trim().to_ascii_lowercase(),
                            crate::checks::html_attrs::attr_value(tag, "href"),
                        )
                    })
                })
                .collect();

        let has_default = annotations
            .iter()
            .any(|(language, _)| language == "x-default");
        let mut incomplete_count = 0usize;
        let mut non_absolute_count = 0usize;
        let mut has_language_self_reference = false;
        for (language, href) in &annotations {
            let Some(href) = href
                .as_deref()
                .map(str::trim)
                .filter(|href| !href.is_empty())
            else {
                incomplete_count += 1;
                continue;
            };
            let Ok(target) = url::Url::parse(href) else {
                non_absolute_count += 1;
                continue;
            };
            if !matches!(target.scheme(), "http" | "https") {
                non_absolute_count += 1;
                continue;
            }
            if language != "x-default" {
                let mut target_without_fragment = target;
                target_without_fragment.set_fragment(None);
                let mut current_without_fragment = ctx.url.clone();
                current_without_fragment.set_fragment(None);
                if target_without_fragment == current_without_fragment {
                    has_language_self_reference = true;
                }
            }
        }

        let has_issue = !annotations.is_empty()
            && (incomplete_count > 0 || non_absolute_count > 0 || !has_language_self_reference);
        let description = if annotations.is_empty() {
            "No HTML link annotation with rel=alternate and hreflang was observed. That is fine when this page has no localized equivalent. This source check does not inspect hreflang delivered through HTTP headers or sitemaps."
                .into()
        } else if has_issue {
            let mut concerns = Vec::new();
            if incomplete_count > 0 {
                concerns.push(format!(
                    "{} annotation{} ha{} an empty language or missing/empty href",
                    incomplete_count,
                    if incomplete_count == 1 { "" } else { "s" },
                    if incomplete_count == 1 { "s" } else { "ve" },
                ));
            }
            if non_absolute_count > 0 {
                concerns.push(format!(
                    "{} href{} not a fully qualified HTTP(S) URL",
                    non_absolute_count,
                    if non_absolute_count == 1 {
                        " is"
                    } else {
                        "s are"
                    },
                ));
            }
            if !has_language_self_reference {
                concerns.push("the HTML set has no language-specific self-reference".into());
            }
            format!(
                "Observed {} HTML hreflang annotation{}; {}. Google Search documents fully qualified alternate URLs and a self-reference in each HTML language set. x-default is optional and is not the reason for this finding. This check did not validate BCP 47 meaning, target responses/canonicals, return links, HTTP-header annotations, or sitemap annotations.",
                annotations.len(),
                if annotations.len() == 1 { "" } else { "s" },
                concerns.join("; "),
            )
        } else {
            format!(
                "Observed {} HTML hreflang annotation{} with a language-specific self-reference. {} This source check does not validate BCP 47 meaning, target responses/canonicals, return links, HTTP-header annotations, or sitemap annotations.",
                annotations.len(),
                if annotations.len() == 1 { "" } else { "s" },
                if has_default {
                    "An x-default fallback is also present; its target and suitability were not evaluated."
                } else {
                    "No x-default fallback is present; x-default is optional and is most useful when an unmatched-language fallback is intentional."
                },
            )
        };

        vec![CheckResult {
            check_id: "seo.hreflang".into(),
            category: ScanCategory::Seo,
            title: if has_issue {
                "Hreflang HTML annotations need review".into()
            } else {
                "Hreflang HTML annotations".into()
            },
            description,
            status: if has_issue {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: has_issue.then(|| "Review the emitted HTML alternate set for this exact page. Give every annotation a supported BCP 47 language/region value (or x-default), a fully qualified canonical final HTTP(S) URL, and include the current page under its own language value. Generate corresponding return annotations from one locale map, then verify the production HTML, target responses, canonicals, and the target search engine's current requirements. Do not add x-default solely to clear this finding.".into()),
            raw_data: Some(serde_json::json!({
                "html_annotation_count": annotations.len(),
                "languages": annotations.iter().map(|(language, _)| language).collect::<Vec<_>>(),
                "x_default_present": has_default,
                "incomplete_annotation_count": incomplete_count,
                "non_absolute_href_count": non_absolute_count,
                "language_self_reference_present": has_language_self_reference,
                "http_header_annotations_inspected": false,
                "sitemap_annotations_inspected": false,
                "target_responses_inspected": false,
                "return_links_inspected_here": false,
            })),
            confidence: if has_issue {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: has_issue.then(|| "The HTML attributes are directly observed, but SiteCMD did not validate the language values, alternate targets, equivalent content, other annotation channels, or target-consumer processing.".into()),
            why_it_matters: has_issue.then(|| "A malformed or incomplete HTML alternate set may be ignored or misinterpreted by consumers that process hreflang. The actual effect depends on the target consumer and any annotations delivered through other supported channels.".into()),
        }]
    }
}

pub struct DuplicateMetaCheck;

impl Check for DuplicateMetaCheck {
    fn id(&self) -> &str {
        "seo.duplicate_meta"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let mut results = Vec::new();

        // Ignore non-content blocks and SVG titles, which label icons rather than documents.
        let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(lower, " ");
        let without_svg =
            sitecmd_engine::checks::seo::parsing::SVG_BLOCK_RE.replace_all(&scannable, " ");
        let title_count =
            crate::checks::html_attrs::tag_slices(&without_svg, &without_svg, "title").len();
        if title_count > 1 {
            results.push(CheckResult {
                check_id: "seo.duplicate_title".into(),
                category: ScanCategory::Seo,
                title: "Multiple document title elements observed".into(),
                description: format!(
                    "Found {} document <title> elements in the scannable initial HTML after excluding comments, scripts, styles, and inline SVG title elements. HTML documents have one document title; SiteCMD did not execute client-side head management or determine which value a browser/search consumer ultimately uses.",
                    title_count
                ),
                status: CheckStatus::Fail,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: Some("Trace every framework, layout, CMS/plugin, template, and client head-management source, then emit one coherent document <title> for the route. Inspect both the production response and rendered head across navigation, fallback, locale, and error states.".into()),
                raw_data: Some(serde_json::json!({
                    "document_title_element_count": title_count,
                    "source": "initial_html",
                    "rendered_head_inspected": false,
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some("Conflicting document-title declarations make the intended title ambiguous to browsers, assistive technology, and crawlers; downstream consumers can apply their own selection or rewriting behavior.".into()),
            });
        }

        // Count only description meta tags, not unrelated elements with that name.
        let desc_count = crate::checks::html_attrs::tag_slices(&scannable, &scannable, "meta")
            .into_iter()
            .filter(|tag| {
                extract_attr_value(tag, "name")
                    .is_some_and(|value| value.eq_ignore_ascii_case("description"))
            })
            .count();
        if desc_count > 1 {
            results.push(CheckResult {
                check_id: "seo.duplicate_description".into(),
                category: ScanCategory::Seo,
                title: "Duplicate meta descriptions".into(),
                description: format!(
                    "Found {} <meta name=\"description\"> elements in the scannable initial HTML. The intended summary is ambiguous; parsers and search engines can select, ignore, or rewrite these declarations, and SiteCMD did not inspect client-side head changes.",
                    desc_count
                ),
                status: CheckStatus::Fail,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: Some("Trace framework, layout, CMS/plugin, template, and client head-management sources, then emit one coherent meta description when the page needs one. Inspect the production response and rendered head across navigation, fallback, locale, and error states.".into()),
                raw_data: Some(serde_json::json!({
                    "meta_description_element_count": desc_count,
                    "source": "initial_html",
                    "rendered_head_inspected": false,
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some("Conflicting description declarations make the page's intended summary ambiguous to crawlers and link-preview consumers; actual search snippets are selected by the search engine.".into()),
            });
        }

        if results.is_empty() {
            results.push(CheckResult {
                check_id: "seo.duplicate_meta".into(),
                category: ScanCategory::Seo,
                title: "Duplicate meta tags".into(),
                description: "No repeated document <title> or <meta name=\"description\"> elements were found in the scannable initial HTML. This pass does not inspect client-side head changes or compare values across pages.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            });
        }

        results
    }
}

#[cfg(test)]
mod tests;
