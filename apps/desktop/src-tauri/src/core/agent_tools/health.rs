use std::process::{Command, Stdio};
use std::time::Instant;

use serde::Deserialize;

use super::{drain_pipe, recv_drained, McpServerSpec};
use crate::constants::{MCP_HEALTH_CHECK_POLL_INTERVAL, MCP_HEALTH_CHECK_TIMEOUT};

const HEALTH_MARKER: &str = "SITECMD_MCP_HEALTH_V1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthPayload {
    marker: String,
    ok: bool,
    error_code: Option<String>,
    database_version: Option<u64>,
    supported_min: Option<u64>,
    supported_max: Option<u64>,
}

fn generic_health_error() -> String {
    "The SiteCMD MCP server did not return a valid health response; restart SiteCMD and try again"
        .to_string()
}

fn health_failure_message(payload: &HealthPayload) -> String {
    match payload.error_code.as_deref() {
        Some("schema_too_new") => match (payload.database_version, payload.supported_max) {
            (Some(database), Some(maximum)) => format!(
                "This SiteCMD build has database version {database}, but its bundled MCP server supports through version {maximum}. Install an updated SiteCMD build, then reconnect your agent"
            ),
            _ => "This SiteCMD build contains an MCP server that is older than its database. Install an updated SiteCMD build, then reconnect your agent".to_string(),
        },
        Some("schema_too_old") => match (payload.database_version, payload.supported_min) {
            (Some(database), Some(minimum)) => format!(
                "SiteCMD's database is version {database}, but the bundled MCP server requires version {minimum} or newer. Restart SiteCMD so it can finish updating, then reconnect your agent"
            ),
            _ => "The SiteCMD database needs to be updated. Restart SiteCMD, then reconnect your agent".to_string(),
        },
        Some("schema_version_missing") => {
            "The SiteCMD database has not finished initializing. Restart SiteCMD, then reconnect your agent".to_string()
        }
        Some("database_not_found") => {
            "SiteCMD could not find its local database. Restart SiteCMD, then reconnect your agent"
                .to_string()
        }
        Some("invalid_database") => {
            "The MCP connection does not point to a valid SiteCMD database. Repair the connection and try again"
                .to_string()
        }
        Some("database_unavailable") => {
            "The SiteCMD MCP server could not read its database. Restart SiteCMD and try again"
                .to_string()
        }
        _ => generic_health_error(),
    }
}

fn health_outcome(status_success: bool, stdout: &str) -> Result<(), String> {
    let payload =
        serde_json::from_str::<HealthPayload>(stdout.trim()).map_err(|_| generic_health_error())?;
    if payload.marker != HEALTH_MARKER {
        return Err(generic_health_error());
    }
    if status_success && payload.ok {
        return Ok(());
    }
    if !payload.ok {
        return Err(health_failure_message(&payload));
    }
    Err(generic_health_error())
}

pub(super) fn run_server_health_check(spec: &McpServerSpec) -> Result<(), String> {
    let mut child = Command::new(&spec.command)
        .args(&spec.args)
        .arg("--sitecmd-health-check")
        .envs(&spec.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "The SiteCMD MCP server could not start".to_string())?;
    let stdout_rx = drain_pipe(child.stdout.take());
    let stderr_rx = drain_pipe(child.stderr.take());
    let started_at = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started_at.elapsed() < MCP_HEALTH_CHECK_TIMEOUT => {
                std::thread::sleep(MCP_HEALTH_CHECK_POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("The SiteCMD MCP server health check timed out".to_string());
            }
            Err(_) => return Err("The SiteCMD MCP server health check failed".to_string()),
        }
    };
    let stdout = recv_drained(&stdout_rx);
    let stderr = recv_drained(&stderr_rx);
    let result = health_outcome(status.success(), &stdout);
    if result.is_err() {
        tracing::warn!(
            stderr = %crate::log_sanitizer::bounded_issue_evidence(&stderr),
            "SiteCMD MCP health check failed"
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_health_requires_the_marker_and_exit_status() {
        let success = r#"{"marker":"SITECMD_MCP_HEALTH_V1","ok":true}"#;
        assert_eq!(health_outcome(true, success), Ok(()));
        assert!(health_outcome(false, success)
            .unwrap_err()
            .contains("valid health response"));
    }

    #[test]
    fn schema_mismatch_names_both_versions() {
        let failure = r#"{"marker":"SITECMD_MCP_HEALTH_V1","ok":false,"errorCode":"schema_too_new","databaseVersion":29,"supportedMin":26,"supportedMax":28}"#;
        let error = health_outcome(false, failure).expect_err("version mismatch");
        assert!(error.contains("database version 29"), "{error}");
        assert!(error.contains("supports through version 28"), "{error}");
    }

    #[test]
    fn older_schema_directs_the_user_to_finish_updating() {
        let failure = r#"{"marker":"SITECMD_MCP_HEALTH_V1","ok":false,"errorCode":"schema_too_old","databaseVersion":25,"supportedMin":26,"supportedMax":28}"#;
        let error = health_outcome(false, failure).expect_err("version mismatch");
        assert!(error.contains("requires version 26 or newer"), "{error}");
        assert!(error.contains("finish updating"), "{error}");
    }

    #[test]
    fn database_failures_have_distinct_recovery_messages() {
        let missing =
            r#"{"marker":"SITECMD_MCP_HEALTH_V1","ok":false,"errorCode":"database_not_found"}"#;
        let invalid =
            r#"{"marker":"SITECMD_MCP_HEALTH_V1","ok":false,"errorCode":"invalid_database"}"#;
        assert!(health_outcome(false, missing)
            .unwrap_err()
            .contains("could not find"));
        assert!(health_outcome(false, invalid)
            .unwrap_err()
            .contains("valid SiteCMD database"));
    }
}
