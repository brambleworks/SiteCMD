//! Point-impact and fix-applicability classification for CLI output.

use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};

/// Where a fix needs to happen - narrows the actionable surface for each issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applicability {
    /// The fix lives in source code (framework config, middleware, component).
    Code,
    /// The fix lives in server / hosting / infra configuration.
    Config,
    /// The fix requires changing copy, images, or page content.
    Content,
}

impl Applicability {
    /// Short bracketed tag for inline CLI output, e.g. `"{code}"`.
    pub fn tag(&self) -> &'static str {
        match self {
            Applicability::Code => "{code}",
            Applicability::Config => "{config}",
            Applicability::Content => "{content}",
        }
    }

    /// Plain label, e.g. `"code"`.
    pub fn label(&self) -> &'static str {
        match self {
            Applicability::Code => "code",
            Applicability::Config => "config",
            Applicability::Content => "content",
        }
    }
}

/// A `CheckResult` annotated with its rank, estimated score impact, and
/// applicability classification.
pub struct RankedIssue<'a> {
    pub issue: &'a CheckResult,
    /// 1-indexed rank (1 = highest impact).
    pub rank: usize,
    /// Estimated score points the overall score is losing due to this issue.
    pub estimated_points: u32,
    /// Where the fix needs to happen.
    pub applicability: Applicability,
}

/// Estimate how many overall score points this issue is costing the site.
///
/// Formula: `penalty × status_multiplier × category_weight`, rounded to the
/// nearest integer. Failing checks always return at least 1 point.
pub fn estimate_points(severity: &Severity, category: &ScanCategory, status: &CheckStatus) -> u32 {
    let penalty: f64 = match severity {
        Severity::Critical => 25.0,
        Severity::High => 12.0,
        Severity::Medium => 5.0,
        Severity::Low => 1.5,
    };

    let status_multiplier: f64 = match status {
        CheckStatus::Fail => 1.0,
        CheckStatus::Warn => 0.5,
        _ => return 0,
    };

    let category_weight: f64 = match category {
        ScanCategory::Security => 0.25,
        ScanCategory::Performance => 0.25,
        ScanCategory::Seo => 0.15,
        ScanCategory::Accessibility => 0.15,
        ScanCategory::Compliance => 0.10,
        ScanCategory::Polish => 0.10,
        ScanCategory::Config => 0.05,
    };

    let raw = penalty * status_multiplier * category_weight;
    let rounded = raw.round() as u32;

    // Failing/warning checks always contribute at least 1 point.
    rounded.max(1)
}

/// Frameworks that support file-based security header configuration.
const FRAMEWORK_CONFIG_HEADERS: &[&str] =
    &["Next.js", "Nuxt", "Astro", "Remix", "SvelteKit", "Gatsby"];

/// Locate the likely fix surface for a check and detected stack.
pub fn classify_applicability(
    check_id: &str,
    detected_stack: Option<&serde_json::Value>,
) -> Applicability {
    let stack_field = |field: &str| -> Option<&str> {
        detected_stack
            .and_then(|v| v.get(field))
            .and_then(|v| v.as_str())
    };

    if check_id.contains("security_header") || check_id.contains("headers.") {
        // If the project uses a framework that handles headers via config files,
        // the fix is a code/config-file change in that framework.
        let framework = stack_field("framework")
            .or_else(|| stack_field("js_framework"))
            .unwrap_or("");
        let has_framework_config = FRAMEWORK_CONFIG_HEADERS
            .iter()
            .any(|f| framework.contains(f));
        return if has_framework_config {
            Applicability::Code
        } else {
            Applicability::Config
        };
    }

    if check_id.starts_with("security.https")
        || check_id.starts_with("security.tls")
        || check_id.contains("https")
        || check_id.contains("tls")
        || check_id.contains("ssl")
        || check_id.contains("hsts")
    {
        return Applicability::Config;
    }

    if check_id.starts_with("security.") {
        return Applicability::Code;
    }

    if check_id == "performance.ttfb" || check_id.contains("ttfb") {
        return Applicability::Config;
    }

    if check_id.starts_with("performance.") {
        return Applicability::Code;
    }

    if check_id.starts_with("seo.") {
        return Applicability::Code;
    }

    if check_id.contains("alt_text")
        || check_id.contains("image_alt")
        || check_id.contains("alt-text")
    {
        return Applicability::Content;
    }

    if check_id.starts_with("accessibility.") || check_id.starts_with("accessibility.") {
        return Applicability::Code;
    }

    if check_id.contains("privacy")
        || check_id.contains("cookie")
        || check_id.contains("gdpr")
        || check_id.contains("ccpa")
    {
        return Applicability::Content;
    }

    if check_id.starts_with("compliance.") {
        return Applicability::Code;
    }

    if check_id.contains("copy")
        || check_id.contains("content")
        || check_id.contains("text")
        || check_id.starts_with("polish.copy_content")
    {
        return Applicability::Content;
    }

    if check_id.starts_with("polish.") {
        return Applicability::Code;
    }

    Applicability::Code
}

/// Ranks failing and warning issues by estimated score impact.
pub fn rank_issues<'a>(
    issues: &'a [CheckResult],
    detected_stack: Option<&serde_json::Value>,
) -> Vec<RankedIssue<'a>> {
    let mut ranked: Vec<(usize, u32, u8, &CheckResult)> = issues
        .iter()
        .filter(|r| matches!(r.status, CheckStatus::Fail | CheckStatus::Warn))
        .map(|r| {
            let pts = estimate_points(&r.severity, &r.category, &r.status);
            let sord = r.severity.sort_rank();
            (0, pts, sord, r)
        })
        .collect();

    // Sort by points descending, then severity ascending (Critical = 0 → highest)
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));

    ranked
        .into_iter()
        .enumerate()
        .map(|(i, (_, pts, _, r))| RankedIssue {
            issue: r,
            rank: i + 1,
            estimated_points: pts,
            applicability: classify_applicability(&r.check_id, detected_stack),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};

    fn make_result(
        check_id: &str,
        category: ScanCategory,
        severity: Severity,
        status: CheckStatus,
    ) -> CheckResult {
        CheckResult {
            check_id: check_id.to_string(),
            category,
            title: check_id.to_string(),
            description: String::new(),
            status,
            severity,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    #[test]
    fn critical_security_has_highest_impact() {
        let pts = estimate_points(
            &Severity::Critical,
            &ScanCategory::Security,
            &CheckStatus::Fail,
        );
        // 25.0 * 1.0 * 0.25 = 6.25 → rounds to 6
        assert!(pts >= 6, "expected >= 6, got {}", pts);
    }

    #[test]
    fn low_polish_has_lowest_impact() {
        let pts = estimate_points(&Severity::Low, &ScanCategory::Polish, &CheckStatus::Fail);
        // 1.5 * 1.0 * 0.10 = 0.15 → rounds to 0 → clamped to 1
        assert!(pts <= 2, "expected <= 2, got {}", pts);
    }

    #[test]
    fn warn_status_halves_impact() {
        let fail_pts =
            estimate_points(&Severity::High, &ScanCategory::Security, &CheckStatus::Fail);
        let warn_pts =
            estimate_points(&Severity::High, &ScanCategory::Security, &CheckStatus::Warn);
        assert!(
            warn_pts <= fail_pts,
            "warn ({}) should be <= fail ({})",
            warn_pts,
            fail_pts
        );
    }

    #[test]
    fn pass_has_zero_impact() {
        let pts = estimate_points(
            &Severity::Critical,
            &ScanCategory::Security,
            &CheckStatus::Pass,
        );
        assert_eq!(pts, 0);
    }

    #[test]
    fn applicability_security_headers_with_nextjs() {
        let stack = serde_json::json!({ "framework": "Next.js", "js_framework": "React" });
        let result = classify_applicability("security_headers.x_frame_options", Some(&stack));
        assert_eq!(result, Applicability::Code);
    }

    #[test]
    fn applicability_security_headers_without_framework() {
        let stack = serde_json::json!({ "server": "nginx" });
        let result = classify_applicability("security_headers.x_frame_options", Some(&stack));
        assert_eq!(result, Applicability::Config);
    }

    #[test]
    fn applicability_alt_text_is_content() {
        let result = classify_applicability("accessibility.alt_text", None);
        assert_eq!(result, Applicability::Content);
    }

    #[test]
    fn applicability_ttfb_is_config() {
        let result = classify_applicability("performance.ttfb", None);
        assert_eq!(result, Applicability::Config);
    }

    #[test]
    fn rank_issues_sorted_by_impact() {
        let issues = vec![
            make_result(
                "performance.ttfb",
                ScanCategory::Performance,
                Severity::Low,
                CheckStatus::Fail,
            ),
            make_result(
                "security.missing_csp",
                ScanCategory::Security,
                Severity::Critical,
                CheckStatus::Fail,
            ),
            make_result(
                "seo.meta_description",
                ScanCategory::Seo,
                Severity::Medium,
                CheckStatus::Warn,
            ),
        ];

        let ranked = rank_issues(&issues, None);

        assert_eq!(ranked.len(), 3);
        // Critical security should be rank 1 (highest impact)
        assert_eq!(ranked[0].issue.check_id, "security.missing_csp");
        assert_eq!(ranked[0].rank, 1);
        // Low performance (ttfb) should be lowest
        assert_eq!(ranked[2].issue.check_id, "performance.ttfb");
        assert_eq!(ranked[2].rank, 3);
        assert!(ranked[0].estimated_points >= ranked[1].estimated_points);
        assert!(ranked[1].estimated_points >= ranked[2].estimated_points);
    }
}
