use crate::checks::Severity;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Complete report data payload for PDF generation
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ReportData {
    pub site_url: String,
    pub project_name: String,
    pub report_title: String,
    pub sections: SectionConfig,
    pub period_label: String,
    pub period_start: String,
    pub period_end: String,
    pub generated_at: String,

    pub site_score: SiteScoreSummary,
    pub health: HealthSummary,
    pub categories: Vec<CategorySummary>,
    pub top_issues: Vec<ReportIssue>,
    pub resolved_count: u32,
    pub latest_scan_date: Option<String>,
    pub code_scan: Option<CodeScanSummary>,

    pub analytics: Option<AnalyticsSummary>,
    pub uptime: Option<UptimeSummary>,
    pub deploys: Option<DeploysSummary>,

    pub branding: ReportBranding,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SiteScoreSummary {
    pub current_score: u32,
    pub issues_total: u32,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub issues_medium: u32,
    pub issues_low: u32,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct HealthSummary {
    pub current_score: u32,
    pub previous_score: Option<u32>,
    pub trend: String, // "up", "down", "stable"
    pub trend_points: Vec<ScorePoint>,
    pub issues_total: u32,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub issues_medium: u32,
    pub issues_low: u32,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScorePoint {
    pub date: String,
    pub score: u32,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct CategorySummary {
    pub name: String,
    pub score: u32,
    pub previous_score: Option<u32>,
    pub issue_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ReportIssue {
    pub title: String,
    pub category: String,
    pub severity: Severity,
    pub description: String,
}

// Renamed on the TS side to avoid colliding with db::types::CodeScanSummary in
// the single bindings bundle; this is the report-payload shape, distinct from
// the scan-history row.
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts", rename = "ReportCodeScanSummary")]
pub struct CodeScanSummary {
    pub current_score: u32,
    pub previous_score: Option<u32>,
    pub trend: String,
    pub issue_count: u32,
    pub critical_count: u32,
    pub high_count: u32,
    pub medium_count: u32,
    pub low_count: u32,
    pub checked_at: String,
    pub framework: Option<String>,
    pub top_domain: Option<String>,
    pub top_domain_count: u32,
    pub domain_trend: Option<String>,
    pub domains: Vec<CodeScanDomainSummary>,
    pub top_issues: Vec<ReportIssue>,
}

// Renamed on the TS side to avoid colliding with db::types::CodeScanDomainSummary.
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts", rename = "ReportCodeScanDomainSummary")]
pub struct CodeScanDomainSummary {
    pub name: String,
    pub issue_count: u32,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct AnalyticsSummary {
    pub visitors: u64,
    pub pageviews: u64,
    pub bounce_rate: f64,
    pub visit_duration: f64,
    pub prev_visitors: Option<u64>,
    pub prev_pageviews: Option<u64>,
    pub prev_bounce_rate: Option<f64>,
    pub prev_visit_duration: Option<f64>,
    pub top_pages: Vec<TopPageEntry>,
    pub top_sources: Vec<TopSourceEntry>,
    pub daily_visitors: Vec<DailyPoint>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DailyPoint {
    pub date: String,
    pub value: u64,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct TopPageEntry {
    pub page: String,
    pub visitors: u64,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct TopSourceEntry {
    pub source: String,
    pub visitors: u64,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct UptimeSummary {
    pub uptime_pct: f64,
    pub incidents: u32,
    pub avg_response_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DeploysSummary {
    pub count: u32,
    pub recent: Vec<DeployEntry>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DeployEntry {
    pub date: String,
    pub message: String,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ReportBranding {
    pub company_name: String,
    pub logo_path: Option<String>,
    pub logo_data_url: Option<String>,
    pub logo_name: Option<String>,
    pub primary_color: String,
    pub footer_text: String,
    pub client_name: Option<String>,
    pub hide_attribution: bool,
}

impl Default for ReportBranding {
    fn default() -> Self {
        Self {
            company_name: "SiteCMD".into(),
            logo_path: None,
            logo_data_url: None,
            logo_name: None,
            primary_color: "#2563eb".into(),
            footer_text: "Confidential".into(),
            client_name: None,
            hide_attribution: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SectionConfig {
    #[serde(default = "default_true")]
    pub executive_summary: bool,
    #[serde(default = "default_true")]
    pub category_breakdown: bool,
    #[serde(default = "default_true")]
    pub top_issues: bool,
    #[serde(default = "default_true")]
    pub recommendations: bool,
    #[serde(default = "default_true")]
    pub code_scan: bool,
    #[serde(default = "default_true")]
    pub analytics: bool,
    #[serde(default = "default_true")]
    pub uptime: bool,
    #[serde(default = "default_true")]
    pub deploys: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            executive_summary: true,
            category_breakdown: true,
            top_issues: true,
            recommendations: true,
            code_scan: true,
            analytics: true,
            uptime: true,
            deploys: true,
        }
    }
}
