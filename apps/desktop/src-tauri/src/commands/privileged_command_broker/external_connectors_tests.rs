use super::*;
use crate::commands::privileged_command_broker::PrivilegedCommandTokenState;
use serde_json::json;

#[test]
fn missing_token_is_rejected_before_any_work() {
    let tokens = PrivilegedCommandTokenState::default();
    let result = tokens.consume(None, BROKER_COMMAND, "fetch_analytics", &json!({}));
    let error = result.expect_err("missing token must be rejected");
    assert!(
        error.contains("Missing privileged command token"),
        "unexpected error message: {error}"
    );
}

#[test]
fn stale_token_is_rejected_before_any_work() {
    let tokens = PrivilegedCommandTokenState::default();
    let result = tokens.consume(
        Some("0000000000000000000000000000000000000000000000000000000000000000"),
        BROKER_COMMAND,
        "fetch_analytics",
        &json!({}),
    );
    let error = result.expect_err("stale token must be rejected");
    assert!(
        error.contains("invalid or expired"),
        "unexpected error message: {error}"
    );
}

#[test]
fn unknown_command_returns_scope_labelled_error() {
    let unsupported = format!("Unsupported {SCOPE_LABEL} command.");
    assert_eq!(unsupported, "Unsupported external connector command.");
}

#[test]
fn public_allowlist_matches_domain_dispatchers() {
    let mut allowlist = EXTERNAL_CONNECTOR_COMMANDS.to_vec();
    let mut routed = dispatch::routed_commands();
    let routed_count = routed.len();
    allowlist.sort_unstable();
    routed.sort_unstable();
    routed.dedup();

    assert_eq!(
        routed_count,
        routed.len(),
        "dispatcher command names must be unique"
    );
    assert_eq!(
        allowlist, routed,
        "public allowlist and dispatchers must match"
    );
}

#[test]
fn broker_command_constant_matches_token_issue_name() {
    assert_eq!(BROKER_COMMAND, "run_external_connector_command");
}
