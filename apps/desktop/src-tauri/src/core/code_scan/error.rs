//! Typed Code Scan cancellation and failure errors.

/// Error surfaced by `run_code_scan_internal` and its callers.
#[derive(Debug, thiserror::Error)]
pub enum CodeScanError {
    /// The text is matched by `scan-error.ts`.
    #[error("Code scan cancelled.")]
    Cancelled,
    /// Engine or infrastructure failure, sanitized before wrapping.
    #[error("{0}")]
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_display_matches_frontend_contract() {
        let rendered = CodeScanError::Cancelled.to_string();
        assert_eq!(
            rendered, "Code scan cancelled.",
            "wire text must stay byte-identical to the legacy literal"
        );
        assert!(
            rendered.to_lowercase().contains("cancelled"),
            "scan-error.ts keys on this substring: {rendered}"
        );
    }

    #[test]
    fn failed_display_is_message_passthrough() {
        let rendered = CodeScanError::Failed("Code scan task failed: boom".to_string()).to_string();
        assert_eq!(rendered, "Code scan task failed: boom");
    }
}
