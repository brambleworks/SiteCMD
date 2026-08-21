//! Named-column database row deserialization.

use rusqlite::{Row, Statement};

use super::helpers::{parse_optional_enum_required, parse_required_enum};
use super::issue_links::IssueLink;
use super::types::*;
use super::DbError;

pub fn i64_from_u64(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("Value {} is too large for SQLite INTEGER", value))
}

pub fn row_u64(row: &Row<'_>, idx: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(idx)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(idx, value))
}

pub fn row_u64_named(row: &Row<'_>, name: &str) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(name)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, value))
}

/// Deserialize a struct from a `rusqlite::Row` using named columns.
pub trait FromRow: Sized {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self>;
}

/// Execute a prepared statement and collect all rows into a `Vec<T>`.
#[tracing::instrument(skip(stmt, params))]
pub fn query_vec<T: FromRow>(
    stmt: &mut Statement<'_>,
    params: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<T>, DbError> {
    let rows = stmt.query_map(params, T::from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

impl FromRow for ScanSummary {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            url: row.get("url")?,
            mode: row.get("mode")?,
            scan_type: parse_required_enum(
                13,
                "scans.scan_type",
                &row.get::<_, String>("scan_type")?,
            )?,
            overall_score: row.get("overall_score")?,
            issues_total: row.get("issues_total")?,
            issues_critical: row.get("issues_critical")?,
            issues_high: row.get("issues_high")?,
            issues_medium: row.get("issues_medium")?,
            issues_low: row.get("issues_low")?,
            duration_ms: row_u64_named(row, "duration_ms")?,
            timestamp: row.get("timestamp")?,
            session_id: row.get("session_id")?,
            page_url: row.get("page_url")?,
        })
    }
}

impl FromRow for CodeScanSummary {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            environment_url: row.get("environment_url")?,
            overall_score: row.get("overall_score")?,
            issue_count: row.get("issue_count")?,
            grouped_issue_count: 0,
            critical_count: row.get("critical_count")?,
            high_count: row.get("high_count")?,
            duration_ms: row_u64_named(row, "duration_ms")?,
            checked_at: row.get("checked_at")?,
            framework: row.get("framework")?,
            top_domain: parse_optional_enum_required(
                10,
                "scan_runs.top_domain",
                row.get::<_, Option<String>>("top_domain")?,
            )?,
            top_domain_count: row.get("top_domain_count")?,
            domain_summaries: Vec::new(),
        })
    }
}

impl FromRow for CodeScanDomainSummary {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        // Both domain-summary queries alias the aggregate columns identically.
        // An unknown domain must fail instead of being relabeled Security.
        let domain =
            parse_required_enum(0, "scan_findings.domain", &row.get::<_, String>("domain")?)?;
        Ok(Self {
            domain,
            issue_count: row.get("issue_count")?,
            critical_count: row.get("critical_count")?,
            high_count: row.get("high_count")?,
            medium_count: row.get("medium_count")?,
            low_count: row.get("low_count")?,
        })
    }
}

impl FromRow for IssueLink {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            check_id: row.get("check_id")?,
            scan_id: row.get("scan_id")?,
            provider: row.get("provider")?,
            external_id: row.get("external_id")?,
            external_url: row.get("external_url")?,
            status: row.get("status")?,
            created_at: row.get("created_at")?,
            resolved_at: row.get("resolved_at")?,
        })
    }
}

impl FromRow for ProjectRecord {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            path: row.get("path")?,
            framework: row.get("framework")?,
            created_at: row.get("created_at")?,
            secret_namespace: row
                .get::<_, Option<String>>("secret_namespace")?
                .unwrap_or_default(),
            environments: Vec::new(), // loaded separately
        })
    }
}

impl FromRow for EnvironmentRecord {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            url: row.get("url")?,
            label: row.get("label")?,
            environment: row.get("environment")?,
            source: row.get("source")?,
            last_scanned_at: row.get("last_scanned_at")?,
            latest_score: row.get("latest_score")?,
        })
    }
}

impl FromRow for PageRecord {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            site_id: row.get("site_id")?,
            url: row.get("url")?,
            path: row.get("path")?,
            title: row.get("title")?,
            last_seen_at: row.get("last_seen_at")?,
            source: row.get("source")?,
        })
    }
}

impl FromRow for ScoreTrendPoint {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            overall: row.get("overall_score")?,
            security: row.get("security_score")?,
            performance: row.get("performance_score")?,
            seo: row.get("seo_score")?,
            accessibility: row.get("accessibility_score")?,
            compliance: row.get("compliance_score")?,
            config: row.get("config_score")?,
            polish: row.get("polish_score")?,
            timestamp: row.get("timestamp")?,
            issues: row.get("issues_total")?,
            scan_type: parse_required_enum(
                11,
                "scans.scan_type",
                &row.get::<_, String>("scan_type")?,
            )?,
        })
    }
}

impl FromRow for WebhookConfig {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            url: row.get("url")?,
            events: row.get("events")?,
            secret: row.get("secret")?,
            enabled: row.get("enabled")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl FromRow for ReportHistoryEntry {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            site_url: row.get("site_url")?,
            period_days: row.get("period_days")?,
            report_title: row.get("report_title")?,
            output_format: row.get("output_format")?,
            generated_at: row.get("generated_at")?,
            branding_json: row.get("branding_json")?,
            sections_json: row.get("sections_json")?,
            report_summary_json: row.get("report_summary_json")?,
        })
    }
}

impl FromRow for SiteEvent {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let event_type_str: String = row.get("event_type")?;
        let severity_str: String = row.get("severity")?;
        let source_str: String = row.get("source")?;
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            event_type: parse_required_enum(2, "events.event_type", &event_type_str)?,
            severity: parse_required_enum(3, "events.severity", &severity_str)?,
            occurred_at_ms: row.get("occurred_at_ms")?,
            title: row.get("title")?,
            summary: row.get("summary")?,
            detail: row.get("detail")?,
            source: parse_required_enum(8, "events.source", &source_str)?,
            source_id: row.get("source_id")?,
            metadata: row.get("metadata").unwrap_or(None),
            // Not hydrated on reads; use get_events_for_check_ids for junction lookups.
            affected_check_ids: None,
        })
    }
}

impl FromRow for ScanSchedule {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            environment_id: row.get("environment_id")?,
            frequency: row.get("frequency")?,
            time_of_day: row.get("time_of_day")?,
            day_of_week: row.get("day_of_week")?,
            scan_type: parse_required_enum(
                6,
                "scan_schedules.scan_type",
                &row.get::<_, String>("scan_type")?,
            )?,
            last_run_at: row.get("last_run_at")?,
            next_run_at: row.get("next_run_at")?,
        })
    }
}
