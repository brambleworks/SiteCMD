//! Top-level scan intent, admission, and retry identity.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::core::scanner::ScanType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum ScanExecutionMode {
    Full,
    Web,
    Code,
}

impl ScanExecutionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Web => "web",
            Self::Code => "code",
        }
    }
}

impl fmt::Display for ScanExecutionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ScanExecutionMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "full" => Ok(Self::Full),
            "web" => Ok(Self::Web),
            "code" => Ok(Self::Code),
            other => Err(format!("unknown scan execution mode: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum ScanTrigger {
    Manual,
    Tray,
    Scheduled,
    Verification,
    Background,
    Migration,
}

impl ScanTrigger {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Tray => "tray",
            Self::Scheduled => "scheduled",
            Self::Verification => "verification",
            Self::Background => "background",
            Self::Migration => "migration",
        }
    }
}

impl FromStr for ScanTrigger {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "manual" => Ok(Self::Manual),
            "tray" => Ok(Self::Tray),
            "scheduled" => Ok(Self::Scheduled),
            "verification" => Ok(Self::Verification),
            "background" => Ok(Self::Background),
            "migration" => Ok(Self::Migration),
            other => Err(format!("unknown scan trigger: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum ScanAdmissionClass {
    GeneralScan,
    BoundedVerification,
    SystemExempt,
}

impl ScanAdmissionClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GeneralScan => "general_scan",
            Self::BoundedVerification => "bounded_verification",
            Self::SystemExempt => "system_exempt",
        }
    }
}

impl FromStr for ScanAdmissionClass {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "general_scan" => Ok(Self::GeneralScan),
            "bounded_verification" => Ok(Self::BoundedVerification),
            "system_exempt" => Ok(Self::SystemExempt),
            other => Err(format!("unknown scan admission class: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum ScanExecutionStatus {
    Planned,
    Running,
    Complete,
    Partial,
    Failed,
    Cancelled,
}

impl ScanExecutionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Partial | Self::Failed | Self::Cancelled
        )
    }
}

impl FromStr for ScanExecutionStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "planned" => Ok(Self::Planned),
            "running" => Ok(Self::Running),
            "complete" => Ok(Self::Complete),
            "partial" => Ok(Self::Partial),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown scan execution status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum ScanComponentStatus {
    Planned,
    Running,
    Complete,
    Failed,
    Cancelled,
    Skipped,
}

impl ScanComponentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }

    pub const fn is_unsettled(self) -> bool {
        matches!(self, Self::Planned | Self::Running)
    }
}

impl FromStr for ScanComponentStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "planned" => Ok(Self::Planned),
            "running" => Ok(Self::Running),
            "complete" => Ok(Self::Complete),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            other => Err(format!("unknown scan component status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanComponent {
    Web,
    Code,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanExecutionRecord {
    pub id: i64,
    pub project_id: Option<i64>,
    pub environment_id: Option<i64>,
    pub environment_url: Option<String>,
    pub environment_scope_key: String,
    pub requested_mode: ScanExecutionMode,
    pub web_focus: Option<ScanType>,
    pub trigger: ScanTrigger,
    pub admission_class: ScanAdmissionClass,
    pub status: ScanExecutionStatus,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub score_snapshot_id: Option<i64>,
    pub failure_summary: Option<String>,
    pub web_status: Option<ScanComponentStatus>,
    pub web_detail: Option<String>,
    pub code_status: Option<ScanComponentStatus>,
    pub code_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanExecutionSummary {
    pub id: i64,
    pub project_id: Option<i64>,
    pub environment_id: Option<i64>,
    pub environment_url: Option<String>,
    pub requested_mode: ScanExecutionMode,
    pub web_focus: Option<ScanType>,
    pub trigger: ScanTrigger,
    pub status: ScanExecutionStatus,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub score: Option<f64>,
    pub critical_count: Option<u32>,
    pub high_count: Option<u32>,
    pub medium_count: Option<u32>,
    pub low_count: Option<u32>,
    pub web_status: Option<ScanComponentStatus>,
    pub web_detail: Option<String>,
    pub code_status: Option<ScanComponentStatus>,
    pub code_detail: Option<String>,
    pub web_scan_id: Option<i64>,
    pub web_session_id: Option<i64>,
    pub web_page_count: u32,
    pub code_scan_id: Option<i64>,
    pub runs: Vec<ScanRunSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanRunSummary {
    pub id: i64,
    pub parent_run_id: Option<i64>,
    pub source: crate::core::normalized_scan::ScanEvidenceSource,
    pub run_kind: crate::core::normalized_scan::ScanRunKind,
    pub status: crate::core::normalized_scan::ScanRunStatus,
    pub timestamp: String,
    pub raw_score: Option<u32>,
    pub duration_ms: u64,
    pub issues_total: u32,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub issues_medium: u32,
    pub issues_low: u32,
    pub diagnostics: crate::core::normalized_scan::NormalizedRunDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanRunDetail {
    pub id: i64,
    pub parent_run_id: Option<i64>,
    pub source: crate::core::normalized_scan::ScanEvidenceSource,
    pub run_kind: crate::core::normalized_scan::ScanRunKind,
    pub status: crate::core::normalized_scan::ScanRunStatus,
    pub timestamp: String,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub raw_score: Option<u32>,
    pub duration_ms: u64,
    pub coverage: crate::core::normalized_scan::ScanCoverageManifest,
    pub diagnostics: crate::core::normalized_scan::NormalizedRunDiagnostics,
    pub status_detail: Option<String>,
    pub detail_state: String,
    pub findings: Vec<crate::core::normalized_scan::NormalizedFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanExecutionDetail {
    pub summary: ScanExecutionSummary,
    pub runs: Vec<ScanRunDetail>,
}

#[derive(Debug, Clone)]
pub struct NewScanExecution {
    pub project_id: Option<i64>,
    pub environment_id: Option<i64>,
    pub environment_url: Option<String>,
    pub environment_scope_key: String,
    pub requested_mode: ScanExecutionMode,
    pub web_focus: Option<ScanType>,
    pub trigger: ScanTrigger,
    pub admission_class: ScanAdmissionClass,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub now_ms: i64,
    pub web_status: Option<ScanComponentStatus>,
    pub web_detail: Option<String>,
    pub code_status: Option<ScanComponentStatus>,
    pub code_detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScanAdmissionOutcome {
    pub execution: ScanExecutionRecord,
    pub reused: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScanAdmissionError {
    #[error("scan action key was already used for a different request")]
    IdempotencyConflict,
    #[error("scan action key is outside the supported retry window; start a new action")]
    IdempotencyStale,
    #[error("invalid scan execution request: {0}")]
    InvalidRequest(String),
    #[error("scan admission storage failed: {0}")]
    Storage(String),
}

/// Typed failures for scan admission and execution.
#[derive(Debug, thiserror::Error)]
pub enum ScanExecutionError {
    #[error(transparent)]
    Admission(#[from] ScanAdmissionError),
    #[error("{0}")]
    Failed(String),
}

impl From<String> for ScanExecutionError {
    fn from(message: String) -> Self {
        ScanExecutionError::Failed(message)
    }
}
