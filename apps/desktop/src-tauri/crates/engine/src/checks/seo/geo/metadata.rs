use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

const ARTICLE_TYPES: &[&str] = &["Article", "BlogPosting", "NewsArticle"];

fn meta_content(
    ctx: &PageContext,
    selector_attribute: &str,
    selector_value: &str,
) -> Option<String> {
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
    let scannable_lower = scannable.to_ascii_lowercase();
    crate::checks::html_attrs::tag_slices(&scannable, &scannable_lower, "meta")
        .into_iter()
        .find_map(|tag| {
            crate::checks::html_attrs::attr_value(tag, selector_attribute)
                .filter(|value| value.eq_ignore_ascii_case(selector_value))
                .and_then(|_| crate::checks::html_attrs::attr_value(tag, "content"))
                .filter(|value| !value.trim().is_empty())
        })
}

fn has_tag_with_nonempty_attr(ctx: &PageContext, tag_name: &str, attribute: &str) -> bool {
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
    let scannable_lower = scannable.to_ascii_lowercase();
    crate::checks::html_attrs::tag_slices(&scannable, &scannable_lower, tag_name)
        .into_iter()
        .any(|tag| {
            crate::checks::html_attrs::attr_value(tag, attribute)
                .is_some_and(|value| !value.trim().is_empty())
        })
}

pub struct CitationMetaCheck;

impl Check for CitationMetaCheck {
    fn id(&self) -> &str {
        "seo.citation_meta"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let json = super::super::structured_data::json_ld_signals(&ctx.body, ctx.body_lower());
        let has_article_type = json.has_any_type(ARTICLE_TYPES);
        let og_article = meta_content(ctx, "property", "og:type")
            .is_some_and(|value| value.eq_ignore_ascii_case("article"));
        let has_article_context = has_article_type || og_article;
        let has_author_meta = meta_content(ctx, "name", "author").is_some();
        let has_article_author_meta = meta_content(ctx, "property", "article:author").is_some();
        let has_citation_author = meta_content(ctx, "name", "citation_author").is_some();
        let has_citation_title = meta_content(ctx, "name", "citation_title").is_some();
        let has_schema_author = json.typed_property(ARTICLE_TYPES, "author");
        let has_schema_publisher = json.typed_property(ARTICLE_TYPES, "publisher");
        let has_author_signal =
            has_author_meta || has_article_author_meta || has_citation_author || has_schema_author;

        let status = if has_author_signal {
            CheckStatus::Pass
        } else if has_article_context {
            CheckStatus::Warn
        } else {
            CheckStatus::Skipped
        };

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: match status {
                CheckStatus::Pass => "Authorship metadata observed".into(),
                CheckStatus::Warn => "Article authorship metadata not observed".into(),
                _ => "Authorship metadata not assessed".into(),
            },
            description: match status {
                CheckStatus::Pass => "At least one nonempty authorship field was observed in a meta tag or on a parsed Article-family JSON-LD node. This presence check does not verify the person's identity, visible byline agreement, property value shape, or support by any particular consumer.".into(),
                CheckStatus::Warn => "Parsed Article-family JSON-LD or og:type=article identifies this as article-like content, but no nonempty author field was observed in the checked metadata. Confirm that the page is actually authored content and that any byline is truthful and represented in the markup used by the intended consumer.".into(),
                _ => "No strong Article-family metadata was observed, so this check cannot establish that page-level authorship metadata is applicable. Ordinary landing, product, utility, account, and other non-editorial pages are not treated as missing an author.".into(),
            },
            status,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if status == CheckStatus::Warn {
                Some("If this is genuinely authored editorial content, show an accurate byline and add the author to the Article-family JSON-LD node or other metadata required by the intended consumer. Link to a useful author profile when appropriate, keep the markup consistent with visible content, and validate the rendered production page. Do not add a synthetic author solely to clear this check.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "author_meta": has_author_meta,
                "article_author_meta": has_article_author_meta,
                "citation_author_meta": has_citation_author,
                "citation_title_meta": has_citation_title,
                "article_schema_author": has_schema_author,
                "article_schema_publisher": has_schema_publisher,
                "article_context_observed": has_article_context,
                "json_ld_blocks": json.block_count,
                "valid_json_ld_blocks": json.valid_block_count,
            })),
            confidence: if status == CheckStatus::Pass {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if status == CheckStatus::Pass {
                None
            } else {
                Some("Metadata presence or absence is directly observed, but SiteCMD cannot determine from source alone whether this page is an authored work, whether runtime markup adds a byline, or which consumer profile is intended.".into())
            },
            why_it_matters: if status == CheckStatus::Warn {
                Some("On genuinely authored content, accurate attribution helps readers and compatible consumers understand who is responsible for the work. It does not guarantee ranking, citation, or inclusion in an answer system.".into())
            } else {
                None
            },
        }]
    }
}

pub struct ContentFreshnessCheck;

impl Check for ContentFreshnessCheck {
    fn id(&self) -> &str {
        "seo.content_freshness"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let json = super::super::structured_data::json_ld_signals(&ctx.body, ctx.body_lower());
        let has_article_type = json.has_any_type(ARTICLE_TYPES);
        let og_article = meta_content(ctx, "property", "og:type")
            .is_some_and(|value| value.eq_ignore_ascii_case("article"));
        let has_published_meta = meta_content(ctx, "property", "article:published_time").is_some();
        let has_modified_meta = meta_content(ctx, "property", "article:modified_time").is_some();
        let has_schema_published = json.typed_property(ARTICLE_TYPES, "datePublished");
        let has_schema_modified = json.typed_property(ARTICLE_TYPES, "dateModified");
        let has_last_modified_header = ctx.response_headers.contains_key("last-modified");
        let has_time_element = has_tag_with_nonempty_attr(ctx, "time", "datetime");
        let has_article_context =
            has_article_type || og_article || has_published_meta || has_modified_meta;
        let has_editorial_date =
            has_published_meta || has_modified_meta || has_schema_published || has_schema_modified;
        let status = if has_editorial_date {
            CheckStatus::Pass
        } else if has_article_context {
            CheckStatus::Warn
        } else {
            CheckStatus::Skipped
        };

        let mut observed = Vec::new();
        if has_published_meta {
            observed.push("article:published_time");
        }
        if has_modified_meta {
            observed.push("article:modified_time");
        }
        if has_schema_published {
            observed.push("datePublished");
        }
        if has_schema_modified {
            observed.push("dateModified");
        }

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: match status {
                CheckStatus::Pass => "Article date metadata observed".into(),
                CheckStatus::Warn => "Article date metadata not observed".into(),
                _ => "Article date metadata not assessed".into(),
            },
            description: match status {
                CheckStatus::Pass => format!(
                    "Observed {} on article-like metadata. This is a property-presence check; it does not validate the date value, determine whether the content visibly shows the same date, or prove that a modification was editorially significant.",
                    observed.join(", ")
                ),
                CheckStatus::Warn if has_last_modified_header => "Article-like metadata was observed, but no nonempty article publication/modification property was found. The response has a Last-Modified header; that header describes the HTTP representation and does not establish an editorial publish or update date.".into(),
                CheckStatus::Warn => "Article-like metadata was observed, but no nonempty article publication or modification property was found in the checked meta tags or Article-family JSON-LD nodes. Confirm whether dates are meaningful for this content before adding them.".into(),
                _ => "No strong Article-family context was observed, so this check does not treat missing publication/update dates as an issue. Dates are not appropriate freshness metadata for every page type.".into(),
            },
            status,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if status == CheckStatus::Warn {
                Some("If this is genuinely date-bearing editorial content, expose the truthful publication date visibly and in the Article-family metadata used by the target consumer. Add a modification date only after a meaningful content change, keep it synchronized with the page, and validate the rendered value/format. Do not use build or deployment time as a synthetic freshness date.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "article_published_meta": has_published_meta,
                "article_modified_meta": has_modified_meta,
                "article_schema_date_published": has_schema_published,
                "article_schema_date_modified": has_schema_modified,
                "last_modified_header": has_last_modified_header,
                "time_element": has_time_element,
                "article_context_observed": has_article_context,
                "date_value_format_validated": false,
            })),
            confidence: if status == CheckStatus::Pass {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if status == CheckStatus::Pass {
                None
            } else {
                Some("The scanned metadata is directly observed, but page type, runtime metadata, the usefulness of dates, and the semantic meaning of Last-Modified cannot be established from this source check alone.".into())
            },
            why_it_matters: if status == CheckStatus::Warn {
                Some("For date-sensitive editorial content, accurate publication and meaningful update dates can help readers and compatible consumers interpret timing. They do not prove that content is current or guarantee search treatment.".into())
            } else {
                None
            },
        }]
    }
}

pub struct OrganizationIdentityCheck;

impl Check for OrganizationIdentityCheck {
    fn id(&self) -> &str {
        "seo.organization_identity"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let json = super::super::structured_data::json_ld_signals(&ctx.body, ctx.body_lower());
        let identity_types: Vec<String> = json
            .types
            .iter()
            .filter(|type_name| super::super::structured_data::is_identity_type(type_name.as_str()))
            .cloned()
            .collect();
        let has_identity = !identity_types.is_empty();
        let has_same_as = json
            .properties_by_type
            .iter()
            .any(|(type_name, properties)| {
                identity_types
                    .iter()
                    .any(|identity| identity.eq_ignore_ascii_case(type_name))
                    && properties
                        .iter()
                        .any(|property| property.eq_ignore_ascii_case("sameAs"))
            });
        let has_website_schema = json.has_type("WebSite");

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if has_identity {
                "Entity identity markup observed".into()
            } else {
                "Entity identity markup not assessed".into()
            },
            description: if has_identity {
                format!(
                    "Parsed JSON-LD contains these identity-oriented types: {}. This confirms type/property presence only; it does not verify the entity, publisher relationship, profile ownership, visible-page agreement, or recognition by a consumer.{}",
                    identity_types.join(", "),
                    if has_same_as {
                        " A nonempty sameAs property is also present, but its destinations were not validated."
                    } else {
                        ""
                    }
                )
            } else if has_website_schema {
                "A WebSite JSON-LD node was observed, but no Organization, Person, LocalBusiness-family, or Organization-subtype node was found in parsed JSON-LD. Those identity nodes are optional and page-specific, so absence is not reported as weak or anonymous identity.".into()
            } else {
                "No identity-oriented JSON-LD type was observed. Organization and Person markup is optional, is normally placed only on an appropriate home/About page or shared graph, and is not a complete measure of who operates the site.".into()
            },
            status: if has_identity {
                CheckStatus::Pass
            } else {
                CheckStatus::Skipped
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "identity_types": identity_types,
                "same_as_property": has_same_as,
                "website_schema": has_website_schema,
                "json_ld_blocks": json.block_count,
                "valid_json_ld_blocks": json.valid_block_count,
                "identity_values_verified": false,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some(if has_identity {
                "The parsed type is direct evidence, but semantic accuracy, placement, entity ownership, and consumer interpretation were not verified.".into()
            } else {
                "Absence applies only to parsed JSON-LD in this response; identity may be communicated visibly, on a more appropriate page, through linked profiles, or through runtime markup.".into()
            }),
            why_it_matters: None,
        }]
    }
}

pub struct FaqSchemaCheck;

impl Check for FaqSchemaCheck {
    fn id(&self) -> &str {
        "seo.faq_schema"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let json = super::super::structured_data::json_ld_signals(&ctx.body, ctx.body_lower());
        let has_faq = json.has_type("FAQPage");
        let has_howto = json.has_type("HowTo");
        let has_qa = json.has_type("QAPage");

        if has_faq || has_howto || has_qa {
            let mut context = Vec::new();
            if has_faq {
                context.push("FAQPage is present; Google's current supported-feature index does not list an FAQ rich-result feature, while other consumers may still interpret the Schema.org type");
            }
            if has_howto {
                context.push("HowTo is present; Google's current supported-feature index does not list a HowTo rich-result feature, while other consumers may still interpret the Schema.org type");
            }
            if has_qa {
                context.push("QAPage is present; its semantics are intended for a page centered on one question with user-submitted answers");
            }
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Question-oriented structured data observed".into(),
                description: format!(
                    "Parsed JSON-LD contains these types: {}. {}. This check does not validate nested properties, visible-content agreement, page semantics, feature eligibility, or use by answer systems.",
                    [
                        has_faq.then_some("FAQPage"),
                        has_howto.then_some("HowTo"),
                        has_qa.then_some("QAPage"),
                    ]
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", "),
                    context.join("; ")
                ),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "faq_page": has_faq,
                    "how_to": has_howto,
                    "qa_page": has_qa,
                    "json_ld_blocks": json.block_count,
                    "valid_json_ld_blocks": json.valid_block_count,
                    "semantic_accuracy_verified": false,
                })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The JSON-LD type is directly observed in a syntactically valid block, but the graph, visible content, page semantics, and consumer support were not validated.".into()),
                why_it_matters: None,
            }];
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CitationMetaCheck, ContentFreshnessCheck, FaqSchemaCheck, OrganizationIdentityCheck,
    };
    use crate::checks::{Check, CheckStatus, IssueConfidence, PageContext, Severity};

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
    fn ordinary_landing_page_is_not_treated_as_an_authored_work() {
        let result = CitationMetaCheck
            .run(&ctx("<main><h1>Product</h1></main>"))
            .remove(0);
        assert_eq!(result.status, CheckStatus::Skipped);
        assert!(result.manual_fix.is_none());
    }

    #[test]
    fn article_without_author_metadata_is_a_needs_review_finding() {
        let html =
            r#"<script type="application/ld+json">{"@type":"Article","headline":"Audit"}</script>"#;
        let result = CitationMetaCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.severity, Severity::Low);
        assert_eq!(result.confidence, IssueConfidence::NeedsReview);
        assert!(result.description.contains("Article"));
        assert!(!result.description.contains("AI tools"));
    }

    #[test]
    fn article_with_author_on_the_article_node_passes() {
        let html = r#"<script type="application/ld+json">{"@type":"Article","headline":"Audit","author":{"@type":"Person","name":"A"}}</script>"#;
        let result = CitationMetaCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("observed"));
        assert!(!result.description.contains("clearer sense"));
    }

    #[test]
    fn json_ld_words_in_javascript_do_not_count_as_article_metadata() {
        let html = r#"<script>const example = '{\"@type\":\"Article\",\"author\":{}}';</script>"#;
        let result = CitationMetaCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Skipped);
    }

    #[test]
    fn meta_tag_examples_inside_javascript_do_not_count_as_article_metadata() {
        let html = r#"<script>const example = '<meta property="og:type" content="article"><meta name="author" content="Example"><meta property="article:published_time" content="2026-07-01">';</script>"#;
        let citation = CitationMetaCheck.run(&ctx(html)).remove(0);
        let freshness = ContentFreshnessCheck.run(&ctx(html)).remove(0);
        assert_eq!(citation.status, CheckStatus::Skipped);
        assert_eq!(freshness.status, CheckStatus::Skipped);
    }

    #[test]
    fn article_without_editorial_dates_warns_but_last_modified_is_not_proof() {
        let html = r#"<script type="application/ld+json">{"@type":"NewsArticle","headline":"News"}</script>"#;
        let mut context = ctx(html);
        context.response_headers.insert(
            http::header::LAST_MODIFIED,
            http::header::HeaderValue::from_static("Tue, 14 Jul 2026 12:00:00 GMT"),
        );
        let result = ContentFreshnessCheck.run(&context).remove(0);
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.confidence, IssueConfidence::NeedsReview);
        assert!(result.description.contains("Last-Modified"));
        assert!(result.description.contains("does not establish"));
    }

    #[test]
    fn article_date_property_on_the_article_node_passes() {
        let html = r#"<script type="application/ld+json">{"@type":"BlogPosting","headline":"Post","datePublished":"2026-07-01"}</script>"#;
        let result = ContentFreshnessCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("datePublished"));
    }

    #[test]
    fn ordinary_page_without_dates_is_not_a_freshness_issue() {
        let result = ContentFreshnessCheck
            .run(&ctx("<main><h1>Pricing</h1></main>"))
            .remove(0);
        assert_eq!(result.status, CheckStatus::Skipped);
    }

    #[test]
    fn missing_organization_schema_is_not_reported_as_weak_identity() {
        let result = OrganizationIdentityCheck
            .run(&ctx("<main><h1>Acme</h1></main>"))
            .remove(0);
        assert_eq!(result.status, CheckStatus::Skipped);
        assert!(!result.description.contains("weak"));
    }

    #[test]
    fn organization_schema_without_same_as_still_passes_bounded_presence_check() {
        let html =
            r#"<script type="application/ld+json">{"@type":"Organization","name":"Acme"}</script>"#;
        let result = OrganizationIdentityCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(!result.description.contains("not strongly tied"));
    }

    #[test]
    fn local_business_subtype_counts_as_identity_markup() {
        let html = r#"<script type="application/ld+json">{"@type":"Restaurant","name":"Cafe","address":"1 Main Street"}</script>"#;
        let result = OrganizationIdentityCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("Restaurant"));
    }

    #[test]
    fn invented_suffix_type_does_not_count_as_identity_markup() {
        let html = r#"<script type="application/ld+json">{"@type":"Disorganization","name":"Example"}</script>"#;
        let result = OrganizationIdentityCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Skipped);
        assert!(result.raw_data.as_ref().unwrap()["identity_types"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn question_heading_does_not_create_a_schema_issue() {
        let html = r#"<h2>How do I install it?</h2><script type="application/ld+json">{"@type":"Organization","name":"Acme"}</script>"#;
        assert!(FaqSchemaCheck.run(&ctx(html)).is_empty());
    }

    #[test]
    fn faqpage_presence_uses_current_feature_scope_without_a_brittle_removal_date() {
        let html =
            r#"<script type="application/ld+json">{"@type":"FAQPage","mainEntity":[]}</script>"#;
        let result = FaqSchemaCheck.run(&ctx(html)).remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result
            .description
            .contains("current supported-feature index"));
        assert!(!result.description.contains("May 2026"));
        assert!(!result.description.contains("AI-friendly"));
        assert!(!result.description.contains("eligible"));
    }
}
