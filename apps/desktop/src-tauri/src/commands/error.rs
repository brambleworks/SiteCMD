//! The command error type: sanitized once, at serialization.

use std::fmt;

/// Error type for Tauri commands. It serializes as the sanitized message, so
/// the renderer still receives a string while sanitization stops being a
/// per-call-site `map_err(sanitize_error)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError(String);

/// Result alias for helpers that feed commands.
pub type CommandResult<T> = Result<T, CommandError>;

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// The unsanitized message, for logs and tests only.
    pub fn raw(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&super::sanitize_error(&self.0))
    }
}

impl serde::Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&super::sanitize_error(&self.0))
    }
}

impl From<CommandError> for String {
    fn from(error: CommandError) -> Self {
        super::sanitize_error(error.0)
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self(message.to_string())
    }
}

impl From<crate::db::DbError> for CommandError {
    fn from(error: crate::db::DbError) -> Self {
        Self(error.to_string())
    }
}

impl From<crate::http_client::BodyReadError> for CommandError {
    fn from(error: crate::http_client::BodyReadError) -> Self {
        Self(error.to_string())
    }
}

impl From<reqwest::Error> for CommandError {
    fn from(error: reqwest::Error) -> Self {
        Self(error.without_url().to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::CommandError;
    use crate::commands::sanitize_error;

    #[test]
    fn sanitize_error_strips_windows_paths() {
        assert_eq!(
            sanitize_error(
                r"Failed to open C:\Users\dev\AppData\Roaming\SiteCMD\sitecmd.db: denied"
            ),
            "Failed to open [internal path]: denied"
        );
        assert_eq!(
            sanitize_error(r"Cannot read \\fileserver\projects\site\index.html"),
            "Cannot read [internal path]"
        );
    }

    #[test]
    fn sanitize_error_preserves_urls() {
        assert_eq!(
            sanitize_error("Fetch https://example.com/blog/post/1 failed"),
            "Fetch https://example.com/blog/post/1 failed"
        );
        assert_eq!(
            sanitize_error(
                "wss://example.com/socket/v1 closed while reading /Users/dev/site/a.log"
            ),
            "wss://example.com/socket/v1 closed while reading [internal path]"
        );
    }

    #[test]
    fn sanitize_error_strips_unix_paths() {
        assert_eq!(
            sanitize_error("No such file /Users/dev/Projects/site/index.html"),
            "No such file [internal path]"
        );
    }

    #[test]
    fn command_error_serializes_as_the_sanitized_string() {
        let error = CommandError::new("open /Users/dev/x/y failed");
        assert_eq!(error.raw(), "open /Users/dev/x/y failed");
        assert_eq!(
            serde_json::to_string(&error).expect("serialize"),
            "\"open [internal path] failed\""
        );
        let as_string: String = error.into();
        assert_eq!(as_string, "open [internal path] failed");
    }

    #[test]
    fn command_error_converts_from_the_domain_errors() {
        let db: CommandError = crate::db::DbError::Other("x".into()).into();
        assert_eq!(db.raw(), "x");
        let text: CommandError = "plain".into();
        assert_eq!(text.raw(), "plain");
    }
}
