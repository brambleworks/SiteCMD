//! Exact persisted shapes for mutable active-issue observations.

use crate::checks::{CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::core::code_scan::CodeScanDomain;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Typed work-item metadata stored in columns rather than parsed from detail JSON.
/// Canonical `check_id` remains the lifecycle identity; producer fields retain
/// the source finding's exact values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemMetadata {
    pub confidence: Option<IssueConfidence>,
    pub domain: Option<CodeScanDomain>,
    pub relative_path: Option<String>,
    pub line: Option<u32>,
    pub check_status: Option<CheckStatus>,
    pub confidence_reason: Option<String>,
    pub producer_check_id: Option<String>,
    pub producer_fix_prompt: Option<String>,
    pub producer_category: Option<ScanCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemRow {
    pub id: i64,
    pub project_id: i64,
    pub env_url: String,
    pub source: String,
    pub signal_id: String,
    pub check_id: String,
    pub category: String,
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub detail_json: Option<String>,
    pub scan_ref: Option<i64>,
    pub page_url: Option<String>,
    pub fix_prompt: Option<String>,
    pub manual_fix: Option<String>,
    pub why_it_matters: Option<String>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub resolved_at: Option<i64>,
    #[serde(flatten)]
    pub metadata: WorkItemMetadata,
}

#[derive(Debug, Clone)]
pub struct WorkItemInput {
    pub project_id: i64,
    pub env_url: String,
    pub source: String,
    pub signal_id: String,
    pub check_id: String,
    pub category: String,
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub detail_json: Option<String>,
    pub scan_ref: Option<i64>,
    pub page_url: Option<String>,
    pub fix_prompt: Option<String>,
    pub manual_fix: Option<String>,
    pub why_it_matters: Option<String>,
    pub observed_at: i64,
    pub metadata: WorkItemMetadata,
}

/// Lifecycle "memory" for one issue `check_id`, aggregated across a project's
/// environments, for the dossier History rail. Timestamps are epoch ms (`None`
/// when the issue has no rows in that lifecycle state yet).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct IssueCheckMemory {
    pub first_seen: Option<i64>,
    pub last_failed: Option<i64>,
    pub last_verified: Option<i64>,
    pub affected_env_urls: Vec<String>,
}
