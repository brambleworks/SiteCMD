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

/// Staging file for the atomic replace; same directory, so the rename cannot
/// cross a mount point.
fn staging_path(path: &Path) -> PathBuf {
    let mut staging = path.as_os_str().to_os_string();
    staging.push(".tmp");
    PathBuf::from(staging)
}

pub fn write_heartbeat_to(path: &Path, now_ms: i64) -> Result<(), String> {
    let body = serde_json::to_vec(&Heartbeat {
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION"),
        updated_at_ms: now_ms,
    })
    .map_err(|error| error.to_string())?;
    // A reader treats an unparsable heartbeat as "the app is not running", so
    // the final path must never be observed truncated or half written.
    let staging = staging_path(path);
    crate::app_identity::write_private_file(&staging, &body).map_err(|error| error.to_string())?;
    std::fs::rename(&staging, path).map_err(|error| {
        let _ = std::fs::remove_file(&staging);
        error.to_string()
    })
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

    #[test]
    fn rewriting_the_heartbeat_never_leaves_a_partial_file_behind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(HEARTBEAT_FILE_NAME);
        let staging = staging_path(&path);
        for beat in 1..=5 {
            write_heartbeat_to(&path, beat).expect("write");
            assert!(
                !staging.exists(),
                "staging file survived beat {beat}; a reader could take it for the heartbeat"
            );
            let parsed: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("read"))
                    .expect("every observable heartbeat parses");
            assert_eq!(parsed["updated_at_ms"], beat);
        }
        assert_eq!(
            std::fs::read_dir(dir.path())
                .expect("read dir")
                .filter_map(Result::ok)
                .count(),
            1,
            "only the heartbeat file itself should remain"
        );
    }
}
