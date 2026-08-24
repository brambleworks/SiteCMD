//! Detects JSON-LD, Microdata, and RDFa, then validates recognized JSON-LD types.
//!
//! Property checks cover a bounded profile, not complete Schema.org or rich-result eligibility.

mod json_ld;
#[cfg(test)]
mod tests;
mod validate;

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

static OPENING_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<[a-z][a-z0-9:_-]*(?:\s[^<>]*?)?/?>")
        .expect("valid structured-data opening-tag regex")
});

pub struct StructuredDataCheck;

#[derive(Default)]
struct TypeGaps {
    required: BTreeSet<String>,
    recommended: BTreeSet<String>,
}

/// Everything learned from parsing and validating the page's JSON-LD.
#[derive(Default)]
struct JsonLdAnalysis {
    block_count: usize,
    failures: Vec<json_ld::ParseFailure>,
    gaps: BTreeMap<String, TypeGaps>,
    validated_types: BTreeSet<String>,
    unvalidated_types: BTreeSet<String>,
}

/// Bounded facts other SEO checks can reuse without falling back to raw
/// substring matching inside JavaScript, comments, or invalid JSON-LD.
#[derive(Default)]
pub(super) struct JsonLdSignals {
    pub block_count: usize,
    pub valid_block_count: usize,
    pub types: BTreeSet<String>,
    pub properties_by_type: BTreeMap<String, BTreeSet<String>>,
}

impl JsonLdSignals {
    pub fn has_type(&self, expected: &str) -> bool {
        self.types
            .iter()
            .any(|type_name| type_name.eq_ignore_ascii_case(expected))
    }

    pub fn has_any_type(&self, expected: &[&str]) -> bool {
        expected.iter().any(|name| self.has_type(name))
    }

    pub fn typed_property(&self, types: &[&str], property: &str) -> bool {
        self.properties_by_type
            .iter()
            .any(|(type_name, properties)| {
                types
                    .iter()
                    .any(|expected| type_name.eq_ignore_ascii_case(expected))
                    && properties
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(property))
            })
    }
}

impl JsonLdAnalysis {
    fn has_no_profile_findings(&self) -> bool {
        self.failures.is_empty() && self.gaps.is_empty() && !self.validated_types.is_empty()
    }
}

impl Check for StructuredDataCheck {
    fn id(&self) -> &str {
        "seo.structured_data"
    }
    fn emitted_ids(&self) -> Vec<String> {
        vec![
            "seo.structured_data".to_string(),
            "seo.structured_data.incomplete".to_string(),
            "seo.structured_data.invalid".to_string(),
        ]
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let parsed_json_ld = analyze_json_ld(&ctx.body, lower);
        let has_json_ld = parsed_json_ld.block_count > 0;
        let analysis = has_json_ld.then_some(parsed_json_ld);

        // JSON-LD lives in scripts and is handled above. Microdata/RDFa
        // markers must be real attributes on opening tags outside scripts,
        // styles, and comments; page-source substring matches also fire in
        // code examples and JavaScript strings.
        let scannable = super::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let mut has_microdata = false;
        let mut has_rdfa_type = false;
        let mut has_rdfa_property = false;
        for tag in OPENING_TAG_RE.find_iter(&scannable) {
            let tag = tag.as_str();
            has_microdata |= crate::checks::html_attrs::has_attr(tag, "itemscope")
                || crate::checks::html_attrs::has_attr(tag, "itemtype");
            has_rdfa_type |= crate::checks::html_attrs::has_attr(tag, "typeof")
                || crate::checks::html_attrs::has_attr(tag, "vocab");
            has_rdfa_property |= crate::checks::html_attrs::has_attr(tag, "property");
        }
        let has_rdfa = has_rdfa_type && has_rdfa_property;

        let mut results = vec![presence_result(
            has_json_ld,
            has_microdata,
            has_rdfa,
            analysis.as_ref(),
        )];
        if let Some(analysis) = analysis {
            if !analysis.failures.is_empty() {
                results.push(invalid_result(&analysis));
            }
            if !analysis.gaps.is_empty() {
                results.push(incomplete_result(&analysis));
            }
        }
        results
    }
}

fn analyze_json_ld(body: &str, body_lower: &str) -> JsonLdAnalysis {
    let blocks = json_ld::extract_blocks(body, body_lower);
    let extraction = json_ld::parse_blocks(&blocks);

    let mut analysis = JsonLdAnalysis {
        block_count: extraction.block_count,
        failures: extraction.failures,
        ..Default::default()
    };
    for node in &extraction.nodes {
        match validate::validate_node(node) {
            validate::NodeValidation::Recognized(findings) => {
                if findings.is_complete() {
                    analysis.validated_types.insert(findings.type_name);
                } else {
                    let gap = analysis.gaps.entry(findings.type_name).or_default();
                    gap.required.extend(findings.missing_required);
                    gap.recommended.extend(findings.missing_recommended);
                }
            }
            validate::NodeValidation::Unrecognized(names) => {
                analysis.unvalidated_types.extend(names);
            }
            validate::NodeValidation::Untyped => {}
        }
    }
    analysis
}

pub(super) fn json_ld_signals(body: &str, body_lower: &str) -> JsonLdSignals {
    let blocks = json_ld::extract_blocks(body, body_lower);
    let extraction = json_ld::parse_blocks(&blocks);
    let mut signals = JsonLdSignals {
        block_count: extraction.block_count,
        valid_block_count: extraction
            .block_count
            .saturating_sub(extraction.failures.len()),
        ..Default::default()
    };
    for node in extraction.nodes {
        let Some(object) = node.as_object() else {
            continue;
        };
        let type_names = match object.get("@type") {
            Some(serde_json::Value::String(name)) => vec![name.as_str()],
            Some(serde_json::Value::Array(names)) => {
                names.iter().filter_map(serde_json::Value::as_str).collect()
            }
            _ => Vec::new(),
        };
        for type_name in type_names {
            signals.types.insert(type_name.to_string());
            let properties = signals
                .properties_by_type
                .entry(type_name.to_string())
                .or_default();
            properties.extend(
                object
                    .iter()
                    .filter(|(_, value)| property_value_present(value))
                    .map(|(name, _)| name.clone()),
            );
        }
    }
    signals
}

pub(super) fn is_identity_type(type_name: &str) -> bool {
    validate::is_identity_type(type_name)
}

fn property_value_present(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(value) => !value.trim().is_empty(),
        serde_json::Value::Array(values) => !values.is_empty(),
        serde_json::Value::Object(values) => !values.is_empty(),
        _ => true,
    }
}

fn presence_result(
    has_json_ld: bool,
    has_microdata: bool,
    has_rdfa: bool,
    analysis: Option<&JsonLdAnalysis>,
) -> CheckResult {
    let has_any = has_json_ld || has_microdata || has_rdfa;

    let format_name = if has_json_ld {
        "JSON-LD"
    } else if has_microdata {
        "Microdata"
    } else if has_rdfa {
        "RDFa"
    } else {
        "none"
    };

    let raw_data = match analysis {
        Some(analysis) => serde_json::json!({
            "json_ld": has_json_ld,
            "microdata": has_microdata,
            "rdfa": has_rdfa,
            "json_ld_blocks": analysis.block_count,
            "validated_types": analysis.validated_types,
            "unvalidated_types": analysis.unvalidated_types,
        }),
        None => {
            serde_json::json!({"json_ld": has_json_ld, "microdata": has_microdata, "rdfa": has_rdfa})
        }
    };

    CheckResult {
        check_id: "seo.structured_data".into(),
        category: ScanCategory::Seo,
        title: if has_any {
            "Structured data".into()
        } else {
            "No structured data found".into()
        },
        description: if let Some(analysis) = analysis.filter(|a| a.has_no_profile_findings()) {
            format!(
                "JSON-LD was parsed successfully, and SiteCMD's limited property profiles found no gaps for these recognized types: {}. This does not validate every Schema.org constraint, page-content match, consumer-specific eligibility rule, or rendered search feature.",
                analysis.validated_types.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        } else if has_any {
            if has_json_ld {
                format!("{} markup is present. JSON syntax and the limited recognized-type property profiles are reported separately; unrecognized types, semantic accuracy, visible-content agreement, and consumer-specific eligibility are not fully validated.", format_name)
            } else {
                format!("{} attribute markers are present. This presence check does not parse the resulting graph, validate vocabulary/property values, compare the markup with visible content, or establish eligibility for a search feature.", format_name)
            }
        } else {
            "No JSON-LD block or tokenized Microdata/RDFa marker set was found in the initial HTML. Structured data is optional and page-specific; absence is actionable only when this page represents content or an entity for which a target consumer supports a useful feature.".into()
        },
        status: if has_any {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: if has_any {
            None
        } else {
            Some("First identify a current consumer or search feature that is relevant to this page; do not add markup solely to clear this check. If useful, emit JSON-LD whose most specific applicable Schema.org types and values accurately describe visible page content. Follow that consumer's current feature documentation for required/recommended properties, validate the general graph with Schema.org tooling, test supported Google features with the Rich Results Test, and monitor the deployed URL in the relevant search console. Valid markup permits consideration but does not guarantee a rich result.".into())
        },
        raw_data: Some(raw_data),
        confidence: if has_any {
            if has_json_ld {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            }
        } else {
            crate::checks::IssueConfidence::NeedsReview
        },
        confidence_reason: if !has_any {
            Some("The initial HTML lacks the scanned markup forms, but structured data is optional, may be injected after load, and has value only for applicable page types and target consumers.".into())
        } else if !has_json_ld {
            Some("Microdata/RDFa attributes were directly observed, but SiteCMD does not parse or semantically validate that graph.".into())
        } else {
            None
        },
        why_it_matters: if has_any {
            None
        } else {
            Some("For an eligible page and supported consumer, accurate structured data can communicate explicit entity/content facts and enable consideration for enhanced presentation. It does not guarantee that presentation, and many pages do not need it.".into())
        },
    }
}

fn invalid_result(analysis: &JsonLdAnalysis) -> CheckResult {
    let listing = analysis
        .failures
        .iter()
        .map(|failure| format!("Block {}: {}", failure.block_index + 1, failure.message))
        .collect::<Vec<_>>()
        .join("; ");

    let invalid_blocks: Vec<serde_json::Value> = analysis
        .failures
        .iter()
        .map(|failure| {
            serde_json::json!({
                "index": failure.block_index,
                "line": failure.line,
                "column": failure.column,
                "error": failure.message,
            })
        })
        .collect();

    CheckResult {
        check_id: "seo.structured_data.invalid".into(),
        category: ScanCategory::Seo,
        title: "Invalid JSON-LD structured data".into(),
        description: format!(
            "{} of {} JSON-LD {} on this page failed to parse. {}.",
            analysis.failures.len(),
            analysis.block_count,
            if analysis.block_count == 1 { "block" } else { "blocks" },
            listing
        ),
        status: CheckStatus::Warn,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: Some(
            "Fix the JSON syntax error at the reported line and column in each failing <script type=\"application/ld+json\"> block. Common causes include trailing commas, single-quoted JSON strings, unescaped quotes, comments, and a literal </script> sequence that terminates the HTML script element. Validate strict JSON first, then validate the resulting JSON-LD graph and any target search feature before re-scanning.".into(),
        ),
        raw_data: Some(serde_json::json!({
            "block_count": analysis.block_count,
            "invalid_blocks": invalid_blocks,
        })),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: Some(
            "A consumer cannot parse a syntactically invalid block as JSON, so the statements in that block may be ignored. Other valid structured-data blocks and the visible page remain separate and may still be processed.".into(),
        ),
    }
}

fn incomplete_result(analysis: &JsonLdAnalysis) -> CheckResult {
    let required_parts: Vec<String> = analysis
        .gaps
        .iter()
        .filter(|(_, gap)| !gap.required.is_empty())
        .map(|(type_name, gap)| {
            format!(
                "{} profile is missing {}",
                type_name,
                join_props(&gap.required)
            )
        })
        .collect();
    let recommended_parts: Vec<String> = analysis
        .gaps
        .iter()
        .filter(|(_, gap)| !gap.recommended.is_empty())
        .map(|(type_name, gap)| {
            format!(
                "{} profile recommends {}",
                type_name,
                join_props(&gap.recommended)
            )
        })
        .collect();

    let has_required = !required_parts.is_empty();
    let description = if has_required {
        let mut text = format!(
            "Against SiteCMD's limited Google-oriented rule profiles, {}.",
            required_parts.join("; ")
        );
        if !recommended_parts.is_empty() {
            text.push_str(&format!(
                " Additional profile recommendations: {}.",
                recommended_parts.join("; ")
            ));
        }
        text.push_str(" Requirements differ by consumer and search feature; Schema.org vocabulary itself does not make these properties universally required.");
        text
    } else {
        format!(
            "SiteCMD's limited Google-oriented profiles found only recommended-property gaps: {}. These are not universal Schema.org requirements and may be irrelevant to the page's target consumers.",
            recommended_parts.join("; ")
        )
    };

    let types: serde_json::Map<String, serde_json::Value> = analysis
        .gaps
        .iter()
        .map(|(type_name, gap)| {
            (
                type_name.clone(),
                serde_json::json!({
                    "missing_required_for_profile": gap.required,
                    "missing_recommended_for_profile": gap.recommended,
                }),
            )
        })
        .collect();

    CheckResult {
        check_id: "seo.structured_data.incomplete".into(),
        category: ScanCategory::Seo,
        title: "Structured data property profile needs review".into(),
        description,
        status: CheckStatus::Warn,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: Some("First choose the actual consumer and feature this JSON-LD is intended to support. Compare the flagged node with that consumer's current type-specific documentation; add only applicable, truthful properties represented by visible content, and remove or change a type if it misstates the page. Use Schema.org to confirm vocabulary meaning, not as a source of Google-specific required fields. Validate the graph, run the appropriate feature test, inspect the rendered production block, and monitor consumer/search-console reports. If the profile is not relevant, no markup change is required.".into()),
        raw_data: Some(serde_json::json!({
            "types": types,
            "unvalidated_types": analysis.unvalidated_types,
        })),
        confidence: crate::checks::IssueConfidence::NeedsReview,
        confidence_reason: Some("The property absence is directly observed, but SiteCMD validates only a small, versioned profile and cannot know the intended consumer, rich-result subtype, page eligibility, visible-content truth, or current platform-specific requirements.".into()),
        why_it_matters: Some(
            "For a chosen feature, omitted feature-required properties can make a node ineligible, while recommended properties can improve completeness. The effect is consumer-specific, and complete markup still does not guarantee enhanced presentation.".into(),
        ),
    }
}

fn join_props(props: &BTreeSet<String>) -> String {
    props.iter().cloned().collect::<Vec<_>>().join(", ")
}
