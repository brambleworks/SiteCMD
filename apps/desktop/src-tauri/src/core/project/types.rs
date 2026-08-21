use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A detected URL environment from project config files
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DetectedUrl {
    pub url: String,
    pub environment: String, // "local", "development", "staging", "production"
    pub source: String,      // which config file it came from
}

/// Project detection result
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub urls: Vec<DetectedUrl>,
    pub framework: Option<String>,
}
