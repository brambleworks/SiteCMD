//! Shared types returned by the database layer.

use crate::checks::Severity;
use crate::core::code_scan::{
    CodeIssueView, CodeScanDomain, CodeScanReportView, CodeScanSkippedScopes,
};
use crate::core::scanner::{ScanType, ScheduledScanType};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Summary of a scan for history display
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanSummary {
    pub id: i64,
    pub url: String,
    pub mode: String,
    pub scan_type: ScanType,
    pub overall_score: u32,
    pub issues_total: u32,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub issues_medium: u32,
    pub issues_low: u32,
    pub duration_ms: u64,
    pub timestamp: String,
    pub session_id: Option<i64>,
    pub page_url: Option<String>,
}

/// Scan session summary for multi-page scan history
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanSessionSummary {
    pub session_id: i64,
    pub total_pages: i64,
    pub completed_pages: i64,
    pub status: String,
    pub started_at: String,
    pub overall_score: Option<i64>,
    pub duration_ms: Option<i64>,
    pub page_scans: Vec<ScanSummary>,
}

/// Summary of a code scan for history display
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct CodeScanDomainSummary {
    pub domain: CodeScanDomain,
    pub issue_count: u32,
    pub critical_count: u32,
    pub high_count: u32,
    pub medium_count: u32,
    pub low_count: u32,
}

/// Summary of a code scan for history display
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct CodeScanSummary {
    pub id: i64,
    pub project_id: i64,
    pub environment_url: Option<String>,
    pub overall_score: u32,
    pub issue_count: u32,
    /// Unique canonical issue groups, or zero when views are unavailable.
    pub grouped_issue_count: u32,
    pub critical_count: u32,
    pub high_count: u32,
    pub duration_ms: u64,
    pub checked_at: String,
    pub framework: Option<String>,
    pub top_domain: Option<CodeScanDomain>,
    pub top_domain_count: u32,
    pub domain_summaries: Vec<CodeScanDomainSummary>,
}

/// Full persisted code scan details
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct CodeScanResult {
    pub id: i64,
    pub project_id: i64,
    pub environment_url: Option<String>,
    pub overall_score: u32,
    pub issue_count: u32,
    pub critical_count: u32,
    pub high_count: u32,
    pub medium_count: u32,
    pub low_count: u32,
    pub duration_ms: u64,
    pub checked_at: String,
    pub framework: Option<String>,
    pub domain_summaries: Vec<CodeScanDomainSummary>,
    pub issues: Vec<CodeIssueView>,
    /// Pruned-directory counts from a fresh scan; absent on persisted history.
    #[serde(default)]
    #[ts(optional)]
    pub skipped_scopes: Option<CodeScanSkippedScopes>,
}

/// Non-persisted code scan report payload returned by direct code audit commands.
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct CodeScanReportPayload {
    pub checked_at: String,
    pub framework: Option<String>,
    pub issue_count: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub issues: Vec<CodeIssueView>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SearchRegressionSignal {
    pub source: String,
    pub delta_pct: i32,
    pub focus: Option<String>,
    pub item_id: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectMonitoringSignals {
    pub enabled_integrations: Vec<String>,
    pub integration_failure_count: u32,
    pub stale_integration_count: u32,
    pub search_regression: Option<SearchRegressionSignal>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectAttentionTargets {
    pub security_issue_id: Option<String>,
    pub security_focus: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum WorkItemKind {
    Web,
    Code,
    Launch,
    Update,
}

impl std::fmt::Display for WorkItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Web => write!(f, "web"),
            Self::Code => write!(f, "code"),
            Self::Launch => write!(f, "launch"),
            Self::Update => write!(f, "update"),
        }
    }
}

impl std::str::FromStr for WorkItemKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "web" => Ok(Self::Web),
            "code" => Ok(Self::Code),
            "launch" => Ok(Self::Launch),
            "update" => Ok(Self::Update),
            other => Err(format!("unknown WorkItemKind: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum WorkItemStatus {
    New,
    Working,
    Snoozed,
    Verified,
    Regressed,
    Ignored,
    Blocked,
}

impl std::fmt::Display for WorkItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New => write!(f, "new"),
            Self::Working => write!(f, "working"),
            Self::Snoozed => write!(f, "snoozed"),
            Self::Verified => write!(f, "verified"),
            Self::Regressed => write!(f, "regressed"),
            Self::Ignored => write!(f, "ignored"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

impl std::str::FromStr for WorkItemStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "new" => Ok(Self::New),
            "working" => Ok(Self::Working),
            "snoozed" => Ok(Self::Snoozed),
            "verified" => Ok(Self::Verified),
            "regressed" => Ok(Self::Regressed),
            "ignored" => Ok(Self::Ignored),
            "blocked" => Ok(Self::Blocked),
            other => Err(format!("unknown WorkItemStatus: {}", other)),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct WorkItemTarget {
    pub page: String,
    pub project_id: Option<i64>,
    pub url: Option<String>,
    pub scan_id: Option<i64>,
    pub session_id: Option<i64>,
    pub scan_kind: Option<String>,
    pub focus: Option<String>,
    pub item_id: Option<String>,
    pub prompt_id: Option<String>,
    pub lane: Option<String>,
    pub reason: Option<String>,
    pub file_path: Option<String>,
    pub restore_scan: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectWorkItem {
    pub stable_key: String,
    pub project_id: i64,
    pub environment_url: Option<String>,
    pub kind: WorkItemKind,
    pub status: WorkItemStatus,
    pub severity: Option<Severity>,
    pub title: String,
    pub summary: String,
    pub category: Option<String>,
    pub domain: Option<String>,
    pub package_name: Option<String>,
    pub target: WorkItemTarget,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub last_verified_at: Option<String>,
    pub last_status_changed_at: String,
    pub snooze_until: Option<i64>,
    pub block_reason: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectWorkQueue {
    pub resume_now: Vec<ProjectWorkItem>,
    pub verify_now: Vec<ProjectWorkItem>,
    pub fix_next: Vec<ProjectWorkItem>,
    pub maintenance: Vec<ProjectWorkItem>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectWorkSummary {
    /// Active canonical issue groups, excluding update and launch queue items.
    /// Code-only groups are counted separately from all other sources.
    #[serde(default)]
    pub issue_count: u32,
    #[serde(default)]
    pub issue_web_count: u32,
    #[serde(default)]
    pub issue_code_count: u32,
    #[serde(default)]
    pub issue_critical_count: u32,
    #[serde(default)]
    pub issue_high_count: u32,
    #[serde(default)]
    pub issue_medium_count: u32,
    #[serde(default)]
    pub issue_low_count: u32,
    pub unresolved_count: u32,
    pub new_count: u32,
    pub working_count: u32,
    pub regressed_count: u32,
    pub ignored_count: u32,
    pub blocked_count: u32,
    pub launch_blocker_count: u32,
    pub maintenance_count: u32,
    pub primary_action: Option<ProjectWorkItem>,
    pub regressed_action: Option<ProjectWorkItem>,
    pub working_action: Option<ProjectWorkItem>,
    pub blocked_action: Option<ProjectWorkItem>,
    pub ignored_action: Option<ProjectWorkItem>,
    pub launch_blocker_action: Option<ProjectWorkItem>,
    pub weekly_summary: Option<ProjectWorkItem>,
}

#[derive(Debug, Clone)]
pub struct ProjectSignalSnapshotRecord {
    pub project_id: i64,
    pub environment_url: Option<String>,
    pub monitoring_json: Option<String>,
    pub monitoring_refreshed_at: Option<String>,
    pub updates_json: Option<String>,
    pub updates_refreshed_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectSignalSnapshot {
    pub project_id: i64,
    pub environment_url: Option<String>,
    pub first_scan_banner_dismissed: bool,
    pub code_scan_summary: Option<CodeScanSummary>,
    pub previous_code_scan_summary: Option<CodeScanSummary>,
    pub code_scan_detail: Option<CodeScanResult>,
    pub monitoring: ProjectMonitoringSignals,
    pub monitoring_refreshed_at: Option<String>,
    pub updates: Option<crate::updates::types::UpdateReport>,
    pub updates_refreshed_at: Option<String>,
    pub targets: ProjectAttentionTargets,
    pub work_summary: ProjectWorkSummary,
}

impl From<CodeScanReportView> for CodeScanReportPayload {
    fn from(report: CodeScanReportView) -> Self {
        Self {
            checked_at: report.checked_at,
            framework: report.framework,
            issue_count: report.issue_count,
            critical_count: report.critical_count,
            high_count: report.high_count,
            medium_count: report.medium_count,
            low_count: report.low_count,
            issues: report.issues,
        }
    }
}

/// Exact producer observation retained beneath a consolidated finding.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConsolidatedIssueInstance {
    pub page_url: Option<String>,
    pub category: crate::checks::ScanCategory,
    pub check_id: String,
    pub severity: Severity,
    pub status: crate::checks::CheckStatus,
    pub title: String,
    pub description: String,
    pub fix_prompt: Option<String>,
    pub manual_fix: Option<String>,
    #[ts(type = "unknown")]
    pub raw_data: Option<serde_json::Value>,
    pub confidence: crate::checks::IssueConfidence,
    pub confidence_reason: Option<String>,
    pub why_it_matters: Option<String>,
}

/// Issue consolidated across multiple page scans.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConsolidatedIssue {
    pub category: crate::checks::ScanCategory,
    pub check_id: String,
    pub severity: Severity,
    pub status: crate::checks::CheckStatus,
    pub title: String,
    pub description: String,
    pub fix_prompt: Option<String>,
    pub manual_fix: Option<String>,
    pub confidence: crate::checks::IssueConfidence,
    pub confidence_reason: Option<String>,
    pub why_it_matters: Option<String>,
    pub pages: Vec<String>,
    pub page_count: usize,
    pub instances: Vec<ConsolidatedIssueInstance>,
}

/// Event type for the timeline calendar
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum EventType {
    Scan,
    Verification,
    Search,
    Update,
    Launch,
    Deploy,
    Uptime,
    Analytics,
    Security,
    Performance,
    Accessibility,
    Compliance,
    Anomaly,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scan => write!(f, "scan"),
            Self::Verification => write!(f, "verification"),
            Self::Search => write!(f, "search"),
            Self::Update => write!(f, "update"),
            Self::Launch => write!(f, "launch"),
            Self::Deploy => write!(f, "deploy"),
            Self::Uptime => write!(f, "uptime"),
            Self::Analytics => write!(f, "analytics"),
            Self::Security => write!(f, "security"),
            Self::Performance => write!(f, "performance"),
            Self::Accessibility => write!(f, "accessibility"),
            Self::Compliance => write!(f, "compliance"),
            Self::Anomaly => write!(f, "anomaly"),
        }
    }
}

impl std::str::FromStr for EventType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "scan" => Ok(Self::Scan),
            "verification" => Ok(Self::Verification),
            "search" => Ok(Self::Search),
            "update" => Ok(Self::Update),
            "launch" => Ok(Self::Launch),
            "deploy" => Ok(Self::Deploy),
            "uptime" => Ok(Self::Uptime),
            "analytics" => Ok(Self::Analytics),
            "security" => Ok(Self::Security),
            "performance" => Ok(Self::Performance),
            "accessibility" => Ok(Self::Accessibility),
            "compliance" => Ok(Self::Compliance),
            "anomaly" => Ok(Self::Anomaly),
            _ => Err(format!("unknown EventType: {}", s)),
        }
    }
}

/// Event severity level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum EventSeverity {
    Info,
    Warning,
    Critical,
}

impl EventSeverity {
    pub fn from_scan_score(score: u32) -> Self {
        if score < 50 {
            Self::Critical
        } else if score < 80 {
            Self::Warning
        } else {
            Self::Info
        }
    }

    pub fn from_issue_counts(critical_count: usize, high_count: usize) -> Self {
        if critical_count > 0 {
            Self::Critical
        } else if high_count > 0 {
            Self::Warning
        } else {
            Self::Info
        }
    }
}

impl std::fmt::Display for EventSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

#[cfg(test)]
mod event_severity_tests {
    use super::EventSeverity;

    #[test]
    fn maps_scan_scores_to_event_severity_bands() {
        assert_eq!(EventSeverity::from_scan_score(49), EventSeverity::Critical);
        assert_eq!(EventSeverity::from_scan_score(50), EventSeverity::Warning);
        assert_eq!(EventSeverity::from_scan_score(79), EventSeverity::Warning);
        assert_eq!(EventSeverity::from_scan_score(80), EventSeverity::Info);
    }

    #[test]
    fn maps_issue_counts_to_event_severity() {
        assert_eq!(
            EventSeverity::from_issue_counts(1, 0),
            EventSeverity::Critical
        );
        assert_eq!(
            EventSeverity::from_issue_counts(0, 1),
            EventSeverity::Warning
        );
        assert_eq!(EventSeverity::from_issue_counts(0, 0), EventSeverity::Info);
    }
}

impl std::str::FromStr for EventSeverity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "info" => Ok(Self::Info),
            "warning" => Ok(Self::Warning),
            "critical" => Ok(Self::Critical),
            _ => Err(format!("unknown EventSeverity: {}", s)),
        }
    }
}

/// Event source - where the event originated
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum EventSource {
    Internal,
    Git,
    #[serde(rename = "uptimerobot")]
    UptimeRobot,
    Plausible,
    Cloudflare,
}

impl std::fmt::Display for EventSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal => write!(f, "internal"),
            Self::Git => write!(f, "git"),
            Self::UptimeRobot => write!(f, "uptimerobot"),
            Self::Plausible => write!(f, "plausible"),
            Self::Cloudflare => write!(f, "cloudflare"),
        }
    }
}

impl std::str::FromStr for EventSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "internal" => Ok(Self::Internal),
            "git" => Ok(Self::Git),
            "uptimerobot" => Ok(Self::UptimeRobot),
            "plausible" => Ok(Self::Plausible),
            "cloudflare" => Ok(Self::Cloudflare),
            _ => Err(format!("unknown EventSource: {}", s)),
        }
    }
}

/// Parses legacy timestamp text into epoch milliseconds. Accepts RFC 3339,
/// naive ISO, and SQLite `datetime('now')` formats; naive values use UTC.
pub fn timestamp_text_to_ms(text: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(dt.timestamp_millis());
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(text, format) {
            return Some(naive.and_utc().timestamp_millis());
        }
    }
    None
}

/// Unified site event for the timeline calendar.
///
/// Stored in the `events` table with `UNIQUE(project_id, source, source_id)` for dedup.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SiteEvent {
    pub id: i64,
    pub project_id: i64,
    pub event_type: EventType,
    pub severity: EventSeverity,
    /// Epoch milliseconds (UTC) when the event occurred. Numeric so ordering
    /// is correct across source timezones (git author dates carry offsets).
    pub occurred_at_ms: i64,
    pub title: String,
    pub summary: String,
    pub detail: Option<String>,
    pub source: EventSource,
    pub source_id: Option<String>,
    /// Structured payload stored verbatim (e.g. serialized AnomalyScore JSON).
    #[serde(default)]
    pub metadata: Option<String>,
    /// In-memory only. Persisted to `site_event_check_ids` junction table on insert.
    /// Not hydrated on reads - use `get_events_for_check_ids` for lookup.
    #[serde(default)]
    pub affected_check_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ReportHistoryEntry {
    pub id: i64,
    pub project_id: i64,
    pub site_url: String,
    pub period_days: u32,
    pub report_title: String,
    pub output_format: String,
    pub generated_at: String,
    pub branding_json: Option<String>,
    pub sections_json: Option<String>,
    pub report_summary_json: Option<String>,
}

/// Project record with environments
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectRecord {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub framework: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing, skip_deserializing, default)]
    #[ts(skip)]
    pub secret_namespace: String,
    pub environments: Vec<EnvironmentRecord>,
}

/// Environment (URL) belonging to a project
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct EnvironmentRecord {
    pub id: i64,
    pub url: String,
    pub label: String,
    pub environment: String,
    pub source: Option<String>,
    pub last_scanned_at: Option<String>,
    pub latest_score: Option<u32>,
}

/// Page record from sitemap discovery
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct PageRecord {
    pub id: i64,
    pub site_id: i64,
    pub url: String,
    pub path: String,
    pub title: Option<String>,
    pub last_seen_at: String,
    pub source: String,
}

/// Webhook configuration for scan notifications
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct WebhookConfig {
    pub id: i64,
    pub project_id: i64,
    pub url: String,
    pub events: String,
    pub secret: Option<String>,
    pub enabled: bool,
    pub created_at: String,
}

/// Scan schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanSchedule {
    pub id: Option<i64>,
    pub project_id: i64,
    pub environment_id: i64,
    pub frequency: String,        // "off", "daily", "weekly"
    pub time_of_day: String,      // "HH:MM"
    pub day_of_week: Option<i32>, // 0=Sun..6=Sat (for weekly)
    pub scan_type: ScheduledScanType,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
}

/// One persisted live-score history row (score_snapshots table): the
/// headline SiteCMD score at the moment it changed. Written on change by the
/// live-score compute sites, read by `get_score_snapshot_history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScoreSnapshotPoint {
    pub overall: f64,
    pub critical_count: u32,
    pub high_count: u32,
    pub medium_count: u32,
    pub low_count: u32,
    pub exploitable_capped: bool,
    pub computed_at: i64,
}

/// Score trend data point for charts
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScoreTrendPoint {
    pub overall: u32,
    pub security: Option<u32>,
    pub performance: Option<u32>,
    pub seo: Option<u32>,
    pub accessibility: Option<u32>,
    pub compliance: Option<u32>,
    pub config: Option<u32>,
    pub polish: Option<u32>,
    pub timestamp: String,
    pub issues: u32,
    pub scan_type: ScanType,
}
