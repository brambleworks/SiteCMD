//! Typed database errors with conversion at string-returning command boundaries.

/// Errors produced by the database layer.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// A rusqlite operation failed (query, prepare, transaction, row parse).
    #[error("{0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Serializing/deserializing a JSON column failed.
    #[error("{0}")]
    Serde(#[from] serde_json::Error),
    /// The DB worker thread's channel is closed (send failed).
    #[error("DB worker thread is no longer accepting operations")]
    WorkerUnavailable,
    /// The operation did not complete within the allotted time.
    #[error("Database operation timed out after {secs}s (the database is busy or stalled)")]
    Timeout { secs: u64 },
    /// The worker disconnected before returning a result.
    #[error("DB worker thread terminated before producing a result")]
    WorkerTerminated,
    /// A hand-rolled validation or business error (no upstream error to wrap).
    #[error("{0}")]
    Other(String),
}

impl From<DbError> for String {
    fn from(e: DbError) -> Self {
        e.to_string()
    }
}

impl From<String> for DbError {
    fn from(s: String) -> Self {
        DbError::Other(s)
    }
}

impl From<&str> for DbError {
    fn from(s: &str) -> Self {
        DbError::Other(s.to_string())
    }
}
