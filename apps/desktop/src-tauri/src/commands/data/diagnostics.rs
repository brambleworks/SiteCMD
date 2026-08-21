use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::LazyLock;
use tauri::{AppHandle, Manager};

use crate::commands::run_blocking;

static DIAGNOSTIC_URL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"https?://[^\s)]+").unwrap());
static DIAGNOSTIC_LOCAL_URL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b(?:localhost|127\.0\.0\.1)(?::\d+)?\b").unwrap());
static DIAGNOSTIC_UNIX_PATH_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"/(?:Users|home|var|tmp|private|Volumes)/[^\s)]+").unwrap()
});
static DIAGNOSTIC_WINDOWS_PATH_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[A-Za-z]:\\[^\s)]+").unwrap());
static DIAGNOSTIC_EMAIL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}").unwrap());
static DIAGNOSTIC_TOKEN_ASSIGNMENT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"(?i)\bauthorization\s*:\s*bearer\s+[^"',;\s]+|\b(?:api[_-]?key|bearer|token|secret|license[_-]?key|refresh[_-]?token|access[_-]?token)\s*[:=]\s*["']?[^"',;\s]+"#,
    )
    .unwrap()
});
static DIAGNOSTIC_SECRET_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\b(?:ghp|github_pat|sk|rk|pk|xox[baprs]|AIza)[A-Za-z0-9_:\-]{8,}\b")
        .unwrap()
});
const MAX_FRONTEND_LOG_CHARS: usize = 4_000;

pub(super) fn redact_diagnostic_text(value: &str) -> String {
    let value = DIAGNOSTIC_TOKEN_ASSIGNMENT_RE.replace_all(value, "[secret]");
    let value = DIAGNOSTIC_SECRET_RE.replace_all(&value, "[secret]");
    let value = DIAGNOSTIC_URL_RE.replace_all(&value, "[url]");
    let value = DIAGNOSTIC_LOCAL_URL_RE.replace_all(&value, "[local-url]");
    let value = DIAGNOSTIC_UNIX_PATH_RE.replace_all(&value, "[path]");
    let value = DIAGNOSTIC_WINDOWS_PATH_RE.replace_all(&value, "[path]");
    let value = DIAGNOSTIC_EMAIL_RE.replace_all(&value, "[email]");
    value.into_owned()
}

fn sanitize_frontend_log_text(value: &str) -> String {
    let redacted = redact_diagnostic_text(value).trim().to_string();
    if redacted.chars().count() <= MAX_FRONTEND_LOG_CHARS {
        return redacted;
    }

    let mut truncated = redacted
        .chars()
        .take(MAX_FRONTEND_LOG_CHARS)
        .collect::<String>();
    truncated.push_str("...[truncated]");
    truncated
}

fn read_file_tail(path: &Path, max_lines: usize) -> Result<String, String> {
    const CHUNK_SIZE: usize = 8 * 1024;
    const MAX_TAIL_BYTES: usize = 512 * 1024;

    let mut file = File::open(path).map_err(|e| format!("Failed to read log file: {}", e))?;
    let file_len = file
        .metadata()
        .map_err(|e| format!("Failed to read log metadata: {}", e))?
        .len();
    if file_len == 0 {
        return Ok(String::new());
    }

    let mut position = file_len;
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    let mut newline_count = 0usize;
    let mut bytes_collected = 0usize;

    while position > 0 && newline_count <= max_lines && bytes_collected < MAX_TAIL_BYTES {
        let read_size = CHUNK_SIZE.min(position as usize);
        position -= read_size as u64;
        file.seek(SeekFrom::Start(position))
            .map_err(|e| format!("Failed to seek log file: {}", e))?;

        let mut buffer = vec![0_u8; read_size];
        file.read_exact(&mut buffer)
            .map_err(|e| format!("Failed to read log file: {}", e))?;
        newline_count += buffer.iter().filter(|byte| **byte == b'\n').count();
        bytes_collected += buffer.len();
        chunks.push(buffer);
    }

    chunks.reverse();
    let mut combined = Vec::with_capacity(bytes_collected);
    for chunk in chunks {
        combined.extend_from_slice(&chunk);
    }

    Ok(String::from_utf8_lossy(&combined).into_owned())
}

/// Log a message from the frontend into the unified log file.
/// Levels: "error", "warn", "info", "debug".
#[tauri::command]
#[tracing::instrument(skip(message, context), fields(level = %level, message_len = message.len(), has_context = context.as_ref().is_some_and(|value| !value.is_empty())))]
pub async fn log_frontend(level: String, message: String, context: Option<String>) {
    let message = sanitize_frontend_log_text(&message);
    let ctx = context
        .as_deref()
        .map(sanitize_frontend_log_text)
        .unwrap_or_default();
    let msg = if ctx.is_empty() {
        format!("[frontend] {}", message)
    } else {
        format!("[frontend] {} | {}", message, ctx)
    };
    match level.as_str() {
        "error" => tracing::error!("{}", msg),
        "warn" => tracing::warn!("{}", msg),
        "debug" => tracing::debug!("{}", msg),
        _ => tracing::info!("{}", msg),
    }
}

/// Get the log directory path so the frontend can read/export logs.
#[tracing::instrument(skip(app))]
pub async fn get_log_path(app: AppHandle) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to resolve log dir: {}", e))?;
    let log_file = log_dir.join("sitecmd.log");
    Ok(log_file.to_string_lossy().to_string())
}

/// Read the most recent log entries (tail of log file). Returns up to `lines` lines.
#[tracing::instrument(skip(app), fields(lines))]
pub async fn read_recent_logs(app: AppHandle, lines: Option<usize>) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("Failed to resolve log dir: {}", e))?;
    let log_file = log_dir.join("sitecmd.log");

    let max_lines = lines.unwrap_or(500);
    // 512 KB tail read is blocking File::seek + read_exact in a loop. Offload
    // so unrelated IPC commands (e.g. scan progress events) don't stall while
    // the diagnostic panel pulls logs.
    let tail_content = run_blocking(move || read_file_tail(&log_file, max_lines)).await??;
    let tail: Vec<&str> = tail_content.lines().rev().take(max_lines).collect();
    let result: Vec<&str> = tail.into_iter().rev().collect();

    // Prepend app version and OS info for diagnostics
    let header = format!(
        "--- SiteCMD Diagnostic Log ---\nVersion: {}\nOS: {} {}\nArch: {}\nTimestamp: {}\n---\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::FAMILY,
        std::env::consts::ARCH,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z"),
    );

    let mut output = header;
    output.push_str(&redact_diagnostic_text(&result.join("\n")));
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_file_tail_returns_recent_lines_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sitecmd.log");
        std::fs::write(&path, "line-1\nline-2\nline-3\nline-4\n").expect("write log");

        let tail = read_file_tail(&path, 2).expect("read tail");

        assert!(tail.ends_with("line-3\nline-4\n") || tail.ends_with("line-3\nline-4"));
        assert!(tail.contains("line-3"));
        assert!(tail.contains("line-4"));
    }

    #[test]
    fn redact_diagnostic_text_removes_common_user_secrets_and_paths() {
        let raw = concat!(
            "site https://example.com/path?token=abc user user@example.com ",
            "path /Users/dev/Projects/Web/SiteCMD/.env ",
            "win C:\\Users\\Dev\\secret.txt ",
            "Authorization: Bearer abcdefghijklmnop ",
            "license_key=sitecmd-dev-core ",
            "ghp_abcdefghijklmnopqrstuvwxyz"
        );

        let redacted = redact_diagnostic_text(raw);

        assert!(redacted.contains("[url]"));
        assert!(redacted.contains("[email]"));
        assert!(redacted.contains("[path]"));
        assert!(redacted.contains("[secret]"));
        assert!(!redacted.contains("example.com"));
        assert!(!redacted.contains("user@example.com"));
        assert!(!redacted.contains("/Users/dev"));
        assert!(!redacted.contains("sitecmd-dev-core"));
        assert!(!redacted.contains("abcdefghijklmnop"));
        assert!(!redacted.contains("ghp_"));
    }

    #[test]
    fn sanitize_frontend_log_text_redacts_before_persistent_logging() {
        let raw = concat!(
            "Unhandled promise rejection https://example.com/callback?token=abc ",
            "admin@example.com /Users/dev/Projects/Web/SiteCMD/.env ",
            "Authorization: Bearer abcdefghijklmnop"
        );

        let redacted = sanitize_frontend_log_text(raw);

        assert!(redacted.contains("[url]"));
        assert!(redacted.contains("[email]"));
        assert!(redacted.contains("[path]"));
        assert!(redacted.contains("[secret]"));
        assert!(!redacted.contains("example.com"));
        assert!(!redacted.contains("admin@example.com"));
        assert!(!redacted.contains("/Users/dev"));
        assert!(!redacted.contains("abcdefghijklmnop"));
    }

    #[test]
    fn sanitize_frontend_log_text_truncates_before_persistent_logging() {
        let redacted = sanitize_frontend_log_text(&"x".repeat(5_000));

        assert_eq!(redacted.chars().count(), 4_014);
        assert!(redacted.ends_with("...[truncated]"));
    }
}
