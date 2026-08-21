//! Desktop canonicalization for portable Code Scan producer IDs.

use sitecmd_engine::identity::code_producer_rule_id;

/// Canonical lifecycle identity for a Code Scan issue occurrence.
pub fn canonical_code_check_id(issue_id: &str) -> String {
    crate::core::correlation::resolve_check_id("code_scan", code_producer_rule_id(issue_id))
}
