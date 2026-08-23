//! Liveness file the MCP server reads before promising that verification,
//! fix attempts, or scans will happen soon.

use std::path::{Path, PathBuf};

pub const HEARTBEAT_FILE_NAME: &str = "desktop-heartbeat.json";

#[derive(serde::Serialize)]
struct Heartbeat {
    pid: u32,
    version: &'static str,
    updated_at_ms: i64,
}

pub fn heartbeat_path() -> Option<PathBuf> {
    crate::app_identity::default_storage_dir().map(|dir| dir.join(HEARTBEAT_FILE_NAME))
}

pub fn write_heartbeat(now_ms: i64) -> Result<(), String> {
    let path = heartbeat_path()
        .ok_or_else(|| "could not resolve the SiteCMD data directory".to_string())?;
    write_heartbeat_to(&path, now_ms)
}

pub fn write_heartbeat_to(path: &Path, now_ms: i64) -> Result<(), String> {
    let body = serde_json::to_vec(&Heartbeat {
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION"),
        updated_at_ms: now_ms,
    })
    .map_err(|error| error.to_string())?;
    crate::app_identity::write_private_file(path, &body).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_file_carries_pid_version_and_time_and_the_poll_beats_the_stale_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(HEARTBEAT_FILE_NAME);
        write_heartbeat_to(&path, 1_234).expect("write");
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(parsed["updated_at_ms"], 1_234);
        assert_eq!(parsed["pid"], std::process::id());
        assert_eq!(parsed["version"], env!("CARGO_PKG_VERSION"));
        assert!(
            crate::constants::AGENT_REQUEST_POLL_INTERVAL.as_millis() as i64 * 3
                <= crate::constants::DESKTOP_HEARTBEAT_STALE_MS,
            "three missed beats must fit inside the stale window the MCP server uses"
        );
    }
}
