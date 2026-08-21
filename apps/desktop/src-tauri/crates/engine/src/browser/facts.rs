//! Portable browser facts and evidence sanitization.
//!
//! Axe keeps every result bucket so clean, incomplete, and unmeasured rules have
//! distinct coverage outcomes.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use super::payload::AxeEvidenceCaps;

static HTML_ATTRIBUTE_VALUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(\s+[^\s=/>]+\s*=\s*)(?:"[^"]*"|'[^']*'|[^\s>]+)"#)
        .expect("static axe HTML attribute regex") // allow-expect: compile-time literal regex
});

static SELECTOR_ATTRIBUTE_VALUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(\[[^\]\s=]+)\s*=\s*(?:"[^"]*"|'[^']*'|[^\]\s]+)(\])"#)
        .expect("static axe selector attribute regex") // allow-expect: compile-time literal regex
});

static SELECTOR_ID_VALUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"#(?:\\[^\r\n]|[^\s#.:>+~,\[\]()'\"])+"#)
        .expect("static axe selector id regex") // allow-expect: compile-time literal regex
});

// Runtime browser metrics use None when the engine cannot observe a value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
pub struct CoreWebVitals {
    pub lcp_ms: Option<f64>,
    pub cls: Option<f64>,
    pub fcp_ms: Option<f64>,
    pub ttfb_ms: Option<f64>,
    // Long-task blocking observed after FCP until SiteCMD reads the page.
    // This is not Lighthouse TBT because the probe does not compute TTI.
    // Some(0.0) means no observed blocking; None means unsupported.
    #[serde(default)]
    pub observed_long_task_blocking_ms: Option<f64>,
    // Runtime JavaScript errors captured during page load (capped sample).
    #[serde(default)]
    pub js_errors: Vec<String>,
    // Total error count, including errors beyond the stored sample cap.
    #[serde(default)]
    pub js_error_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
pub struct AxeNodeEvidence {
    #[serde(default)]
    pub target: Vec<String>,
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub failure_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
pub struct AxeViolation {
    pub id: String,
    pub impact: String,
    pub description: String,
    pub help: String,
    pub help_url: String,
    pub nodes_count: u32,
    // A bounded sample of affected nodes. Older stored payloads deserialize
    // without it, so it must remain optional-by-default on the wire.
    #[serde(default)]
    pub nodes: Vec<AxeNodeEvidence>,
}

// One axe run: the findings plus the rule ids of every other bucket. See the
// module doc: the bucket arrays are what make an absent finding legible.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
pub struct AxeReport {
    #[serde(default)]
    pub violations: Vec<AxeViolation>,
    // Rules that executed and found no failing node.
    #[serde(default)]
    pub passes: Vec<String>,
    // Rules axe could not decide: needs review, never a pass.
    #[serde(default)]
    pub incomplete: Vec<String>,
    // Rules with nothing on the page to evaluate.
    #[serde(default)]
    pub inapplicable: Vec<String>,
}

/// What one axe rule established on one page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOutcome {
    /// The rule reported at least one failing node.
    Violated,
    /// The rule executed and proved absence (`passes` or `inapplicable`).
    Proved,
    /// The rule needs human review (`incomplete`): a coverage exception.
    Undecided,
    /// The rule appears in no bucket, so it did not execute on this page.
    NotRun,
}

impl AxeReport {
    /// What the run established for `rule`, by rule id (`image-alt`, not
    /// `accessibility.axe.image-alt`).
    pub fn rule_outcome(&self, rule: &str) -> RuleOutcome {
        if self.violations.iter().any(|violation| violation.id == rule) {
            return RuleOutcome::Violated;
        }
        if self.passes.iter().any(|id| id == rule) || self.inapplicable.iter().any(|id| id == rule)
        {
            return RuleOutcome::Proved;
        }
        if self.incomplete.iter().any(|id| id == rule) {
            return RuleOutcome::Undecided;
        }
        RuleOutcome::NotRun
    }

    /// Rules the run executed at all, in any bucket. This is the coverage
    /// denominator: everything else was never measured here.
    pub fn executed_rules(&self) -> Vec<&str> {
        let mut rules: Vec<&str> = self
            .violations
            .iter()
            .map(|violation| violation.id.as_str())
            .chain(self.passes.iter().map(String::as_str))
            .chain(self.incomplete.iter().map(String::as_str))
            .chain(self.inapplicable.iter().map(String::as_str))
            .collect();
        rules.sort_unstable();
        rules.dedup();
        rules
    }
}

/// Parse one payload result. The payload reports its own failures as
/// `{"error": "..."}`, which is a failed run and not an empty one - treating
/// it as an empty report would claim every rule proved absence.
pub fn parse_axe_report(json: &str) -> Result<AxeReport, String> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| format!("axe payload was not valid JSON: {error}"))?;
    if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
        return Err(error.to_string());
    }
    let mut report: AxeReport = serde_json::from_value(value)
        .map_err(|error| format!("axe payload did not match the report schema: {error}"))?;
    report.violations = report
        .violations
        .into_iter()
        .map(AxeViolation::sanitize_node_evidence)
        .collect();
    Ok(report)
}

impl AxeViolation {
    /// Apply persistence-safe bounds and remove DOM values that could contain
    /// form data, user content, or credentials. Selectors remain useful, while
    /// attribute values and element text never enter issue evidence.
    pub fn sanitize_node_evidence(mut self) -> Self {
        let caps = AxeEvidenceCaps::DEFAULT;
        self.nodes.truncate(caps.nodes);
        for node in &mut self.nodes {
            node.target.truncate(caps.target_parts);
            for selector in &mut node.target {
                let redacted = SELECTOR_ATTRIBUTE_VALUE
                    .replace_all(selector, "$1=\"[redacted]\"$2")
                    .into_owned();
                let redacted = SELECTOR_ID_VALUE
                    .replace_all(&redacted, "#[redacted]")
                    .into_owned();
                *selector = truncate_chars(
                    &crate::log_sanitizer::redact_secrets(&redacted),
                    caps.selector_chars,
                );
            }

            // axe returns outerHTML. Retain only the opening tag and redact
            // every attribute value; descendant text can contain customer data.
            let opening_tag = opening_html_tag(&node.html);
            let redacted = HTML_ATTRIBUTE_VALUE
                .replace_all(opening_tag, "$1\"[redacted]\"")
                .into_owned();
            node.html = truncate_chars(
                &crate::log_sanitizer::redact_secrets(&redacted),
                caps.html_chars,
            );
            node.failure_summary = node.failure_summary.as_deref().map(|summary| {
                truncate_chars(
                    &crate::log_sanitizer::redact_secrets(summary),
                    caps.failure_summary_chars,
                )
            });
        }
        self
    }
}

fn opening_html_tag(value: &str) -> &str {
    let mut quote = None;
    for (index, character) in value.char_indices() {
        match quote {
            Some(active_quote) if character == active_quote => quote = None,
            Some(_) => {}
            None if matches!(character, '\'' | '"') => quote = Some(character),
            None if character == '>' => return &value[..=index],
            None => {}
        }
    }
    value
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        truncated.push('…');
    }
    truncated
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
