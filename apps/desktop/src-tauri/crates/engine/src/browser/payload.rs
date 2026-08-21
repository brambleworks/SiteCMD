//! Browser instrumentation shared byte-for-byte across all runtimes.
//!
//! Payloads expose results through both a window global and a returned JSON value
//! to support the Tauri and CDP readback paths.

/// The bundled axe-core release. Recorded in the execution profile so a
/// comparison across runs can tell a detector update from a page change; the
/// asset and this constant are pinned together by a test.
pub const AXE_CORE_VERSION: &str = "4.11.2";

/// Core Web Vitals observer, injected BEFORE navigation so it can catch LCP,
/// layout-shift, and long-task entries as the page loads.
pub const CWV_OBSERVER_SCRIPT: &str = include_str!("../../browser/cwv_observer.js");

/// Core Web Vitals readback, evaluated after the page settles.
pub const CWV_READ_SCRIPT: &str = include_str!("../../browser/cwv_read.js");

/// axe-core payload for native injectors; wasm runtimes provide it as an asset.
#[cfg(feature = "browser-payload")]
pub const AXE_CORE_SCRIPT: &str = include_str!("../../browser/axe.min.js");

/// The rule set every runtime runs: WCAG 2.0/2.1/2.2 level A and AA. Widening
/// this changes which rules can appear in the four buckets, so it is a
/// comparability fact, not an adapter detail.
pub const AXE_RUN_TAGS: [&str; 5] = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

/// Evidence caps enforced in-page and again before persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxeEvidenceCaps {
    /// Affected nodes retained per violation.
    pub nodes: usize,
    /// Selector parts retained per node.
    pub target_parts: usize,
    /// Characters retained per selector part.
    pub selector_chars: usize,
    /// Characters retained of the node's opening tag.
    pub html_chars: usize,
    /// Characters retained of axe's failure summary.
    pub failure_summary_chars: usize,
}

impl AxeEvidenceCaps {
    /// The shipped caps. Every runtime uses these; the hosted runner cannot
    /// widen them without changing the comparability contract.
    pub const DEFAULT: Self = Self {
        nodes: 5,
        target_parts: 8,
        selector_chars: 300,
        html_chars: 600,
        failure_summary_chars: 1_000,
    };
}

impl Default for AxeEvidenceCaps {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The axe run script, with the tag set and evidence caps interpolated.
/// All four axe buckets are retained so callers can distinguish proved absence,
/// incomplete review, and rules that never ran.
pub fn axe_run_script(caps: AxeEvidenceCaps) -> String {
    let tags = AXE_RUN_TAGS
        .iter()
        .map(|tag| format!("'{tag}'"))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        r#"(async function () {{
    var payload;
    try {{
        for (var i = 0; i < 20 && typeof axe === 'undefined'; i++) {{
            await new Promise(function (resolve) {{ setTimeout(resolve, 50); }});
        }}
        if (typeof axe === 'undefined') {{
            payload = {{ error: 'axe-core not loaded' }};
        }} else {{
            var results = await axe.run(document, {{
                runOnly: {{ type: 'tag', values: [{tags}] }}
            }});
            var ruleIds = function (bucket) {{
                return (bucket || []).map(function (entry) {{ return String(entry.id); }});
            }};
            payload = {{
                violations: results.violations.map(function (v) {{
                    return {{
                        id: v.id,
                        impact: v.impact || 'minor',
                        description: v.description,
                        help: v.help,
                        help_url: v.helpUrl,
                        nodes_count: v.nodes.length,
                        nodes: v.nodes.slice(0, {nodes}).map(function (n) {{
                            return {{
                                target: (Array.isArray(n.target) ? n.target : [])
                                    .slice(0, {target_parts})
                                    .map(function (part) {{
                                        return String(Array.isArray(part) ? part.join(' > ') : part)
                                            .slice(0, {selector_chars});
                                    }}),
                                html: String(n.html || '').slice(0, {html_chars}),
                                failure_summary: n.failureSummary
                                    ? String(n.failureSummary).slice(0, {failure_chars})
                                    : null
                            }};
                        }})
                    }};
                }}),
                passes: ruleIds(results.passes),
                incomplete: ruleIds(results.incomplete),
                inapplicable: ruleIds(results.inapplicable)
            }};
        }}
    }} catch (e) {{
        payload = {{ error: (e && e.message) || 'axe-core failed' }};
    }}
    window.{global} = payload;
    return JSON.stringify(payload);
}})()"#,
        tags = tags,
        nodes = caps.nodes,
        target_parts = caps.target_parts,
        selector_chars = caps.selector_chars,
        html_chars = caps.html_chars,
        failure_chars = caps.failure_summary_chars,
        global = AXE_RESULT_GLOBAL,
    )
}

/// Where the payload parks its result for runtimes that cannot read an eval
/// value back (the Tauri webview polls this through `document.title`).
pub const AXE_RESULT_GLOBAL: &str = "__SHK_AXE__";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_script_carries_the_shared_tag_set_and_caps() {
        let script = axe_run_script(AxeEvidenceCaps::DEFAULT);
        for tag in AXE_RUN_TAGS {
            assert!(
                script.contains(&format!("'{tag}'")),
                "tag {tag} interpolated"
            );
        }
        assert!(script.contains("slice(0, 5)"), "node cap interpolated");
        assert!(
            script.contains("slice(0, 300)"),
            "selector cap interpolated"
        );
        assert!(script.contains("slice(0, 600)"), "html cap interpolated");
        assert!(
            script.contains("slice(0, 1000)"),
            "failure cap interpolated"
        );
        assert!(
            !script.contains("__AXE_"),
            "no placeholder survives into the injected script"
        );
    }

    #[test]
    fn the_run_script_requests_all_four_buckets() {
        // The coverage claim depends on this: violations alone cannot tell a
        // rule that passed from a rule that never executed.
        let script = axe_run_script(AxeEvidenceCaps::DEFAULT);
        for bucket in [
            "results.violations",
            "results.passes",
            "results.incomplete",
            "results.inapplicable",
        ] {
            assert!(script.contains(bucket), "{bucket} read by the payload");
        }
    }

    #[test]
    fn the_run_script_serves_both_readback_paths() {
        // Title-polling runtimes read the global; CDP and Browser Run await
        // the returned string. One script has to do both.
        let script = axe_run_script(AxeEvidenceCaps::DEFAULT);
        assert!(script.contains("window.__SHK_AXE__ = payload"));
        assert!(script.contains("return JSON.stringify(payload)"));
        assert!(
            !script.trim_end().ends_with(';'),
            "the script stays an expression so an evaluating runtime gets its value"
        );
    }

    #[test]
    fn caps_are_interpolated_rather_than_hardcoded() {
        let script = axe_run_script(AxeEvidenceCaps {
            nodes: 2,
            target_parts: 3,
            selector_chars: 40,
            html_chars: 50,
            failure_summary_chars: 60,
        });
        assert!(script.contains("slice(0, 2)"));
        assert!(script.contains("slice(0, 40)"));
        assert!(script.contains("slice(0, 60)"));
    }

    #[test]
    fn the_observer_payload_publishes_where_the_readback_looks() {
        assert!(CWV_OBSERVER_SCRIPT.contains("__SHK_CWV__"));
        assert!(CWV_READ_SCRIPT.contains("___SHK_CWV___"));
        assert!(CWV_READ_SCRIPT.contains("return json"));
    }

    #[cfg(feature = "browser-payload")]
    #[test]
    fn the_bundled_axe_asset_matches_the_recorded_version() {
        assert!(
            AXE_CORE_SCRIPT.contains(AXE_CORE_VERSION),
            "the bundled axe-core asset must carry the version the profile reports"
        );
    }
}
