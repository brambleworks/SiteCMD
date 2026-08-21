use crate::db::Database;

use super::{DeployEntry, DeploysSummary, SectionConfig};

#[cfg(feature = "desktop")]
pub(super) fn build_deploys_summary(
    db: &Database,
    project_id: i64,
    period_start: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
    sections: &SectionConfig,
) -> Result<Option<DeploysSummary>, String> {
    if !sections.deploys {
        return Ok(None);
    }

    let deploy_types = vec!["deploy".to_string()];
    let deploy_events = db.get_events(
        project_id,
        period_start.timestamp_millis(),
        now.timestamp_millis(),
        Some(&deploy_types),
        None,
        None,
        None,
    )?;

    if deploy_events.is_empty() {
        return Ok(None);
    }

    Ok(Some(DeploysSummary {
        count: deploy_events.len() as u32,
        recent: deploy_events
            .iter()
            .take(20)
            .map(|event| {
                let detail: serde_json::Value = event
                    .detail
                    .as_deref()
                    .and_then(|d| serde_json::from_str(d).ok())
                    .unwrap_or_default();
                DeployEntry {
                    date: chrono::DateTime::from_timestamp_millis(event.occurred_at_ms)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default(),
                    message: event.title.clone(),
                    author: detail
                        .get("author")
                        .and_then(|a| a.as_str())
                        .unwrap_or("")
                        .into(),
                }
            })
            .collect(),
    }))
}
