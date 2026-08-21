use crate::db::Database;
use crate::integrations::{IntegrationConfig, IntegrationType};

use super::SectionConfig;

#[cfg(feature = "desktop")]
pub(super) fn should_include_report_code_scan(sections: &SectionConfig) -> bool {
    sections.code_scan
}

#[cfg(feature = "desktop")]
pub(super) fn should_load_detailed_web_issues(sections: &SectionConfig) -> bool {
    sections.top_issues || sections.recommendations
}

#[cfg(feature = "desktop")]
pub(super) fn required_report_integration_types(sections: &SectionConfig) -> Vec<IntegrationType> {
    let mut types = Vec::new();
    if sections.analytics {
        types.push(IntegrationType::Plausible);
    }
    if sections.uptime {
        types.push(IntegrationType::UptimeRobot);
    }
    types
}

#[cfg(feature = "desktop")]
pub(super) fn load_report_integrations(
    app: &tauri::AppHandle,
    db: &Database,
    project_id: i64,
    sections: &SectionConfig,
) -> Result<Vec<IntegrationConfig>, String> {
    let required_types = required_report_integration_types(sections);
    if required_types.is_empty() {
        return Ok(Vec::new());
    }

    let mut configs: Vec<_> = db
        .get_integrations(project_id)?
        .into_iter()
        .filter(|config| config.enabled && required_types.contains(&config.integration_type))
        .collect();

    for config in &mut configs {
        crate::keyring::hydrate_integration_secrets(app, db, project_id, config);
    }

    Ok(configs)
}
