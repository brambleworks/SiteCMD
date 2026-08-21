//! Timeline event filtering and integration refresh dispatch.
//! Integration adapters perform ingestion; refresh requests fan out through the scheduler.

use crate::db::Database;
use crate::integrations::{IntegrationConfig, IntegrationType};
use std::sync::Arc;

fn is_event_refresh_integration(integration_type: &IntegrationType) -> bool {
    matches!(
        integration_type,
        IntegrationType::UptimeRobot
            | IntegrationType::Plausible
            | IntegrationType::Cloudflare
            | IntegrationType::GoogleAnalytics
            | IntegrationType::GoogleSearchConsole
    )
}

fn take_event_refresh_configs(configs: Vec<IntegrationConfig>) -> Vec<IntegrationConfig> {
    configs
        .into_iter()
        .filter(|config| config.enabled && is_event_refresh_integration(&config.integration_type))
        .collect()
}

/// Map an integration type to the scheduler adapter source name used in
/// `request_immediate_poll`. Returns `None` for integration types that have no
/// event-generating adapter (e.g. GitHub, Jira).
fn source_for_integration(integration_type: &IntegrationType) -> Option<&'static str> {
    match integration_type {
        IntegrationType::UptimeRobot => Some("uptimerobot"),
        IntegrationType::Plausible => Some("plausible"),
        IntegrationType::Cloudflare => Some("cloudflare"),
        IntegrationType::GoogleAnalytics => Some("ga4"),
        IntegrationType::GoogleSearchConsole => Some("gsc"),
        _ => None,
    }
}

fn built_in_event_refresh_sources() -> &'static [&'static str] {
    &["updates"]
}

#[cfg(feature = "desktop")]
/// Queue immediate polls for enabled, entitled integrations.
#[tracing::instrument(skip(_app, db), fields(project_id))]
pub async fn refresh_integration_events(
    _app: &tauri::AppHandle,
    db: &Arc<Database>,
    project_id: i64,
) -> Result<(), String> {
    let configs = take_event_refresh_configs(db.get_integrations(project_id)?);

    for config in &configs {
        if let Some(source) = source_for_integration(&config.integration_type) {
            if let Err(e) =
                crate::core::integration_scheduler::request_immediate_poll(source, project_id, None)
                    .await
            {
                tracing::warn!(
                    "refresh_integration_events: immediate poll request for '{}' failed: {}",
                    source,
                    e
                );
            } else {
                tracing::info!(
                    "refresh_integration_events: queued immediate poll for '{}' project {}",
                    source,
                    project_id
                );
            }
        }
    }

    for source in built_in_event_refresh_sources() {
        if let Err(e) =
            crate::core::integration_scheduler::request_immediate_poll(source, project_id, None)
                .await
        {
            tracing::warn!(
                "refresh_integration_events: immediate poll request for built-in '{}' failed: {}",
                source,
                e
            );
        } else {
            tracing::info!(
                "refresh_integration_events: queued immediate poll for built-in '{}' project {}",
                source,
                project_id
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::take_event_refresh_configs;
    use crate::integrations::{IntegrationConfig, IntegrationType};

    fn config(integration_type: IntegrationType, enabled: bool) -> IntegrationConfig {
        IntegrationConfig {
            integration_type,
            api_key: None,
            site_id: Some("site".to_string()),
            extra: Some(serde_json::json!({
                "tokens": { "access_token": "should-not-matter" }
            })),
            enabled,
        }
    }

    #[test]
    fn event_refresh_includes_all_event_emitting_integrations() {
        let configs = vec![
            config(IntegrationType::Plausible, true),
            config(IntegrationType::UptimeRobot, true),
            config(IntegrationType::Cloudflare, true),
            config(IntegrationType::GoogleAnalytics, true),
            config(IntegrationType::GoogleSearchConsole, true),
            config(IntegrationType::GitHub, true),
            config(IntegrationType::Jira, true),
            config(IntegrationType::Plausible, false),
        ];

        let expected = vec![
            IntegrationType::Plausible,
            IntegrationType::UptimeRobot,
            IntegrationType::Cloudflare,
            IntegrationType::GoogleAnalytics,
            IntegrationType::GoogleSearchConsole,
        ];

        let refreshed = take_event_refresh_configs(configs);

        assert_eq!(
            refreshed
                .iter()
                .map(|config| config.integration_type.clone())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn source_for_integration_maps_scheduler_sources() {
        use super::{built_in_event_refresh_sources, source_for_integration};
        assert_eq!(
            source_for_integration(&IntegrationType::UptimeRobot),
            Some("uptimerobot")
        );
        assert_eq!(
            source_for_integration(&IntegrationType::Plausible),
            Some("plausible")
        );
        assert_eq!(
            source_for_integration(&IntegrationType::Cloudflare),
            Some("cloudflare")
        );
        assert_eq!(
            source_for_integration(&IntegrationType::GoogleAnalytics),
            Some("ga4")
        );
        assert_eq!(
            source_for_integration(&IntegrationType::GoogleSearchConsole),
            Some("gsc")
        );
        assert_eq!(source_for_integration(&IntegrationType::GitHub), None);
        assert_eq!(source_for_integration(&IntegrationType::Jira), None);
        assert_eq!(built_in_event_refresh_sources(), &["updates"]);
    }
}
