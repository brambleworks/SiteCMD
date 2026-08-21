use crate::db::Database;
use crate::report;
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::{confirm_sensitive_action, run_blocking, sanitize_error};

const MAX_REPORT_PERIOD_DAYS: u32 = 365;

fn sanitize_report_period_days(period_days: u32) -> u32 {
    period_days.clamp(1, MAX_REPORT_PERIOD_DAYS)
}

/// Aggregate report data (health, analytics, uptime, deploys) for the Report page preview.
#[tauri::command]
#[tracing::instrument(skip(app, db, branding, sections, site_url), fields(project_id, period_days, report_title = ?report_title))]
pub async fn generate_report_data(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    site_url: String,
    period_days: u32,
    branding: Option<report::ReportBranding>,
    report_title: Option<String>,
    sections: Option<report::SectionConfig>,
) -> Result<report::ReportData, String> {
    let period_days = sanitize_report_period_days(period_days);
    let brand = branding.unwrap_or_default();
    let title = report_title.unwrap_or_default();
    let secs = sections.unwrap_or_default();
    report::aggregate_report(
        &app,
        &db,
        project_id,
        &site_url,
        period_days,
        brand,
        title,
        secs,
    )
    .await
    .map_err(sanitize_error)
}

/// Generate a self-contained HTML report with branding, sections, and inline charts.
#[tauri::command]
#[tracing::instrument(skip(app, db, branding, sections, site_url), fields(project_id, period_days, report_title = ?report_title))]
pub async fn generate_report_html(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    site_url: String,
    period_days: u32,
    branding: Option<report::ReportBranding>,
    report_title: Option<String>,
    sections: Option<report::SectionConfig>,
) -> Result<String, String> {
    let period_days = sanitize_report_period_days(period_days);
    let brand = branding.unwrap_or_default();
    let title = report_title.unwrap_or_default();
    let secs = sections.unwrap_or_default();
    let data = report::aggregate_report(
        &app,
        &db,
        project_id,
        &site_url,
        period_days,
        brand,
        title,
        secs,
    )
    .await
    .map_err(sanitize_error)?;
    Ok(report::render_html(&data))
}

/// Render a self-contained HTML report from already-aggregated report data.
#[tauri::command]
#[tracing::instrument(skip(data))]
pub async fn render_report_html_from_data(data: report::ReportData) -> Result<String, String> {
    Ok(report::render_html(&data))
}

/// Save a report generation record to history (for the "Previous Reports" list).
#[tauri::command]
#[tracing::instrument(skip(db, branding_json, sections_json, report_summary_json, site_url), fields(project_id, period_days, report_title = %report_title, output_format = %output_format, branding_len = branding_json.len(), sections_len = sections_json.len(), has_summary = report_summary_json.is_some()))]
pub async fn save_report_history(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    site_url: String,
    period_days: u32,
    report_title: String,
    output_format: String,
    branding_json: String,
    sections_json: String,
    report_summary_json: Option<String>,
) -> Result<i64, String> {
    let period_days = sanitize_report_period_days(period_days);
    let db = (*db).clone();
    run_blocking(move || {
        db.save_report_history(
            project_id,
            &site_url,
            period_days,
            &report_title,
            &output_format,
            &branding_json,
            &sections_json,
            report_summary_json.as_deref(),
        )
    })
    .await?
    .map_err(sanitize_error)
}

#[cfg(test)]
mod tests {
    use super::sanitize_report_period_days;

    #[test]
    fn sanitize_report_period_days_clamps_to_reasonable_bounds() {
        assert_eq!(sanitize_report_period_days(0), 1);
        assert_eq!(sanitize_report_period_days(30), 30);
        assert_eq!(sanitize_report_period_days(5_000), 365);
    }
}

/// Get report generation history for a project.
#[tauri::command]
pub async fn get_report_history(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Vec<crate::db::ReportHistoryEntry>, String> {
    let db = (*db).clone();
    run_blocking(move || -> Result<_, String> {
        db.get_report_history(project_id).map_err(sanitize_error)
    })
    .await?
}

/// Delete a report history entry by ID.
#[tracing::instrument(skip(app, db), fields(id))]
pub async fn delete_report_history(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    confirm_sensitive_action(
        app,
        "Delete this saved report?",
        "This removes the selected report history entry.".to_string(),
        "Delete Report",
    )
    .await?;
    let db = (*db).clone();
    run_blocking(move || db.delete_report_history(id))
        .await?
        .map_err(sanitize_error)
}
