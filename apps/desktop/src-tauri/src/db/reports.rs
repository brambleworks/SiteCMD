//! Report history CRUD.

use super::DbError;
use rusqlite::named_params;
use serde_json::Value;

use super::from_row;
use super::types::ReportHistoryEntry;
use super::Database;

fn sanitize_history_branding_json(input: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(input) else {
        return input.to_string();
    };

    if let Some(object) = value.as_object_mut() {
        object.insert("logo_path".to_string(), Value::Null);
        object.insert("logo_data_url".to_string(), Value::Null);
    }

    serde_json::to_string(&value).unwrap_or_else(|_| input.to_string())
}

impl Database {
    /// Save a report generation record. Returns the new row ID.
    #[tracing::instrument(skip(self, branding_json, sections_json, report_summary_json, site_url), fields(project_id, period_days, report_title = %report_title, output_format = %output_format, branding_len = branding_json.len(), sections_len = sections_json.len(), has_summary = report_summary_json.is_some()))]
    pub fn save_report_history(
        &self,
        project_id: i64,
        site_url: &str,
        period_days: u32,
        report_title: &str,
        output_format: &str,
        branding_json: &str,
        sections_json: &str,
        report_summary_json: Option<&str>,
    ) -> Result<i64, DbError> {
        let site_url = site_url.to_string();
        let report_title = report_title.to_string();
        let output_format = output_format.to_string();
        let branding_json = sanitize_history_branding_json(branding_json);
        let sections_json = sections_json.to_string();
        let report_summary_json = report_summary_json.map(|value| value.to_string());
        self.execute(move |conn| {
            conn.execute(
                "INSERT INTO report_history (project_id, site_url, period_days, report_title, output_format, branding_json, sections_json, report_summary_json)
                 VALUES (:project_id, :site_url, :period_days, :report_title, :output_format, :branding_json, :sections_json, :report_summary_json)",
                named_params! {
                    ":project_id": project_id,
                    ":site_url": site_url,
                    ":period_days": period_days,
                    ":report_title": report_title,
                    ":output_format": output_format,
                    ":branding_json": branding_json,
                    ":sections_json": sections_json,
                    ":report_summary_json": report_summary_json,
                },
            )?;
            Ok(conn.last_insert_rowid())
        })?
    }

    /// Get report generation history for a project, most recent first (max 50).
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_report_history(&self, project_id: i64) -> Result<Vec<ReportHistoryEntry>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, site_url, period_days, report_title, output_format, generated_at, branding_json, sections_json, report_summary_json
                 FROM report_history WHERE project_id = ?1 ORDER BY generated_at DESC LIMIT 50"
            )?;
            let mut entries = from_row::query_vec::<ReportHistoryEntry>(&mut stmt, &[&project_id])?;
            for entry in &mut entries {
                entry.branding_json = entry
                    .branding_json
                    .as_deref()
                    .map(sanitize_history_branding_json);
            }
            Ok(entries)
        })?
    }

    /// Delete a report history entry by ID.
    #[tracing::instrument(skip(self), fields(id))]
    pub fn delete_report_history(&self, id: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute("DELETE FROM report_history WHERE id = ?1", [id])?;
            Ok(())
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_history_branding_json;

    #[test]
    fn sanitize_history_branding_json_removes_inline_logo_payloads() {
        let sanitized = sanitize_history_branding_json(
            r#"{"company_name":"SiteCMD","logo_path":"/Users/dev/logo.png","logo_data_url":"data:image/png;base64,AAAA","logo_name":"logo.png"}"#,
        );

        assert!(!sanitized.contains("data:image/png"));
        assert!(sanitized.contains(r#""logo_path":null"#));
        assert!(sanitized.contains(r#""logo_data_url":null"#));
        assert!(sanitized.contains(r#""logo_name":"logo.png""#));
    }
}
