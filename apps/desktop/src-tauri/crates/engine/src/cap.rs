//! Shared eligibility rules for the exploitable-issue score cap.

use crate::identity::code_rule_id;
use crate::vocab::Severity;

/// Code-rule classes eligible for the score cap when Critical and explicitly
/// High or Confirmed confidence. Static NeedsReview findings never qualify.
pub const SCORE_CAP_CANDIDATE_CODE_RULES: &[&str] = &[
    // Same-call taint and sink candidates require runtime confirmation.
    "js-command-injection",
    "php-code-execution",
    "php-dynamic-command",
    "php-file-inclusion",
    "php-object-injection",
    "python-code-execution",
    "python-command-injection",
    "python-sql-injection",
    "python-template-injection",
    "python-unsafe-deserialization",
];

/// Web checks with strong enough evidence to qualify for the score cap.
/// Ordinary source regex matches remain NeedsReview and do not qualify.
pub const SCORE_CAP_CANDIDATE_WEB_CHECKS: &[&str] = &["security.exposed_files.env"];

/// Whether a code rule belongs to a class that can qualify for the score cap.
pub fn is_code_score_cap_candidate_rule(rule: &str) -> bool {
    SCORE_CAP_CANDIDATE_CODE_RULES.contains(&rule)
}

/// Return whether a Critical check can qualify for the score cap.
/// Callers must also require explicit High or Confirmed confidence.
pub fn is_score_cap_candidate_check(check_id: &str, severity: Severity) -> bool {
    if severity != Severity::Critical {
        return false;
    }
    if let Some(rule) = code_rule_id(check_id) {
        return is_code_score_cap_candidate_rule(rule);
    }
    SCORE_CAP_CANDIDATE_WEB_CHECKS.contains(&check_id)
}
