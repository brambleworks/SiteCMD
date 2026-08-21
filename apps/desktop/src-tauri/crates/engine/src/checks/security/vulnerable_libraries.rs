//! Detects known client libraries and grades runtime-supplied advisory results.
//!
//! An unavailable advisory corpus cannot produce a clean verification result.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

const CHECK_ID: &str = "security.vulnerable_libraries";

/// Libraries whose filename == npm package name, safe to report from a bare
/// versioned filename. CDN-path detections (cdnjs/unpkg/jsdelivr) trust the
/// path's package segment instead and are not limited to this list.
const KNOWN_LIBS: &[&str] = &[
    "jquery",
    "jquery-ui",
    "bootstrap",
    "react",
    "react-dom",
    "vue",
    "angular",
    "lodash",
    "moment",
    "d3",
    "backbone",
    "handlebars",
    "mustache",
    "knockout",
    "axios",
    "swiper",
    "chart.js",
    "three",
    "underscore",
];

/// cdnjs paths: /ajax/libs/<name>/<version>/...
static CDNJS_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"cdnjs\.cloudflare\.com/ajax/libs/([A-Za-z0-9._-]+)/(\d+\.\d+\.\d+[0-9A-Za-z.-]*)/",
    )
    .unwrap()
});

/// npm-style CDN paths: unpkg.com/<pkg>@<version>/, cdn.jsdelivr.net/npm/<pkg>@<version>/
static NPM_CDN_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?:unpkg\.com|cdn\.jsdelivr\.net/npm|esm\.sh|cdn\.skypack\.dev)/((?:@[A-Za-z0-9._-]+/)?[A-Za-z0-9._-]+)@(\d+\.\d+\.\d+[0-9A-Za-z.-]*)").unwrap()
});

/// Versioned filenames: jquery-1.12.4.min.js, bootstrap-4.0.0.js
static FILENAME_VERSION_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"([A-Za-z][A-Za-z0-9._-]*?)[-.](\d+\.\d+\.\d+)(?:[.-][A-Za-z0-9.]*)?\.js(?:\?|$)",
    )
    .unwrap()
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedLibrary {
    pub name: String,
    pub version: String,
    pub source_url: String,
}

/// One advisory the runtime's vulnerability database returned for an exact
/// (package, version) pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryAdvisory {
    pub package_name: String,
    pub current_version: String,
    pub advisory_id: String,
    pub severity: String,
    pub advisory_url: Option<String>,
    pub fixed_version: Option<String>,
}

/// The outcome of the runtime's advisory-database lookup. `Answered` with an
/// empty list is a real clean result; `Unavailable` means no claim can be
/// made either way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "advisories", rename_all = "snake_case")]
pub enum AdvisoryLookup {
    Answered(Vec<LibraryAdvisory>),
    Unavailable,
}

fn library_list(detected: &[DetectedLibrary]) -> String {
    detected
        .iter()
        .map(|l| format!("{} {}", l.name, l.version))
        .collect::<Vec<_>>()
        .join(", ")
}

fn library_count_phrase(count: usize) -> String {
    if count == 1 {
        "1 recognizable library with a pinned version".to_string()
    } else {
        format!("{} recognizable libraries with pinned versions", count)
    }
}

fn libraries_raw_data(detected: &[DetectedLibrary]) -> serde_json::Value {
    serde_json::json!({
        "libraries": detected.iter().map(|l| serde_json::json!({
            "name": l.name, "version": l.version, "src": l.source_url,
        })).collect::<Vec<_>>(),
    })
}

/// Pass result: the detected versions were checked against the advisory
/// database and came back clean. Reachable only from `Answered`.
fn clean_pass_result(detected: &[DetectedLibrary]) -> CheckResult {
    CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Security,
        title: "Front-end library versions".into(),
        description: format!(
            "{} detected ({}); OSV.dev reports no known advisories for those exact versions.",
            library_count_phrase(detected.len()),
            library_list(detected),
        ),
        status: CheckStatus::Pass,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(libraries_raw_data(detected)),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Advisory failure cannot prove absence; return `Skipped`.
fn osv_unreachable_result(detected: &[DetectedLibrary]) -> CheckResult {
    CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Security,
        title: "Front-end library versions not checked".into(),
        description: format!(
            "{} detected ({}), but OSV.dev could not be reached, so those versions were not checked for known advisories. Re-run the scan with network access to verify them.",
            library_count_phrase(detected.len()),
            library_list(detected),
        ),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "libraries": detected.iter().map(|l| serde_json::json!({
                "name": l.name, "version": l.version, "src": l.source_url,
            })).collect::<Vec<_>>(),
            "osv_unreachable": true,
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "The OSV.dev advisory lookup failed (offline or blocked network); the detected versions are unverified.".into(),
        ),
        why_it_matters: None,
    }
}

/// Extract (library, version) pairs from the page's script URLs.
pub fn detect_libraries(body: &str) -> Vec<DetectedLibrary> {
    let mut found: Vec<DetectedLibrary> = Vec::new();
    let mut push = |name: String, version: String, url: &str| {
        let name = name.to_ascii_lowercase();
        if !found.iter().any(|l| l.name == name && l.version == version) {
            found.push(DetectedLibrary {
                name,
                version,
                source_url: crate::log_sanitizer::evidence_safe_url_reference(url),
            });
        }
    };

    let lower = body.to_ascii_lowercase();
    for tag in crate::checks::html_attrs::tag_slices(body, &lower, "script") {
        let Some(src) = crate::checks::html_attrs::attr_value(tag, "src")
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        let src = src.trim();

        if let Some(c) = CDNJS_RE.captures(src) {
            // cdnjs library names mostly match npm; jquery/bootstrap/etc do.
            push(c[1].to_string(), c[2].to_string(), src);
            continue;
        }
        if let Some(c) = NPM_CDN_RE.captures(src) {
            push(c[1].to_string(), c[2].to_string(), src);
            continue;
        }
        if let Some(c) = FILENAME_VERSION_RE.captures(src) {
            let name = c[1].to_ascii_lowercase();
            // Bare filenames are only trustworthy for household names;
            // "app-2.1.3.min.js" must not be queried as the npm "app" package.
            if KNOWN_LIBS.contains(&name.as_str()) {
                push(name, c[2].to_string(), src);
            }
        }
    }

    found
}

/// Grade the detected libraries against the runtime's advisory lookup.
/// Emits no rows when nothing detectable was found: staying silent is
/// honest, a clean bill would not be.
pub fn evaluate_vulnerable_libraries(
    detected: &[DetectedLibrary],
    lookup: AdvisoryLookup,
) -> Vec<CheckResult> {
    if detected.is_empty() {
        return vec![];
    }

    let advisories = match lookup {
        AdvisoryLookup::Answered(advisories) => advisories,
        AdvisoryLookup::Unavailable => return vec![osv_unreachable_result(detected)],
    };
    if advisories.is_empty() {
        return vec![clean_pass_result(detected)];
    }

    // One aggregated finding: list each vulnerable library with its
    // advisories. High when any advisory reads high/critical.
    let mut lines: Vec<String> = Vec::new();
    let mut worst_is_high = false;
    for lib in detected {
        let lib_advisories: Vec<&LibraryAdvisory> = advisories
            .iter()
            .filter(|v| v.package_name == lib.name && v.current_version == lib.version)
            .collect();
        if lib_advisories.is_empty() {
            continue;
        }
        if lib_advisories.iter().any(|v| {
            matches!(
                v.severity.to_ascii_lowercase().as_str(),
                "high" | "critical"
            )
        }) {
            worst_is_high = true;
        }
        let ids: Vec<&str> = lib_advisories
            .iter()
            .take(4)
            .map(|v| v.advisory_id.as_str())
            .collect();
        lines.push(format!(
            "{} {} ({} advisor{}: {}{})",
            lib.name,
            lib.version,
            lib_advisories.len(),
            if lib_advisories.len() == 1 {
                "y"
            } else {
                "ies"
            },
            ids.join(", "),
            if lib_advisories.len() > ids.len() {
                ", ..."
            } else {
                ""
            },
        ));
    }

    // Every advisory named a package/version pair this page does not
    // actually carry, so there is nothing to report against this page.
    if lines.is_empty() {
        return vec![clean_pass_result(detected)];
    }

    let severity = if worst_is_high {
        Severity::High
    } else {
        Severity::Medium
    };

    vec![CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Security,
        title: format!(
            "{} front-end librar{} with known vulnerabilities",
            lines.len(),
            if lines.len() == 1 { "y" } else { "ies" }
        ),
        description: format!(
            "Script URLs identify library versions for which OSV.dev returned published security advisories: {}. The version/advisory match does not establish that an affected function is used, reachable, or exploitable in this deployment; review the linked advisory conditions and fixed versions.",
            lines.join("; "),
        ),
        status: CheckStatus::Fail,
        severity,
        fix_prompt: None,
        manual_fix: Some(
            "Open each first-party advisory/OSV record and confirm the affected version range, vulnerable feature, prerequisites, and fixed version. Upgrade to a supported fixed release, run the application's browser/unit tests, and remove the old asset from caches/CDNs. If no supported fix exists, disable the affected feature or replace the library based on the advisory's mitigation guidance.".into(),
        ),
        raw_data: Some(serde_json::json!({
            "vulnerable": advisories.iter().map(|v| serde_json::json!({
                "package": v.package_name,
                "version": v.current_version,
                "advisory": v.advisory_id,
                "severity": v.severity,
                "url": v.advisory_url,
                "fixed_version": v.fixed_version,
            })).collect::<Vec<_>>(),
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: Some(
            "A published advisory means the detected version falls in a known affected range. Real impact depends on whether this site invokes the vulnerable behavior and whether the advisory's prerequisites are present, but an unnecessary affected version increases avoidable exposure.".into(),
        ),
    }]
}

#[cfg(test)]
#[path = "vulnerable_libraries_tests.rs"]
mod tests;
