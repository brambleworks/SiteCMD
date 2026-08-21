//! Google Analytics traffic alerts using the Plausible baseline guard.

use async_trait::async_trait;
use std::time::Duration;

use crate::db::alerts::AlertInput;
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use crate::integrations::google_analytics::{GA4DailyPoint, GA4Data};

pub struct Ga4Adapter;

impl Default for Ga4Adapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Ga4Adapter {
    #[tracing::instrument]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl IntegrationAdapter for Ga4Adapter {
    fn source(&self) -> &'static str {
        "ga4"
    }

    fn cadence(&self) -> Duration {
        // allow-inline-duration: per-adapter polling cadence.
        Duration::from_secs(300)
    }

    fn is_configured(&self, credentials: &crate::integrations::adapters::Credentials) -> bool {
        credentials.has_oauth_token() && credentials.has_site_id()
    }

    fn env_scoped(&self) -> bool {
        // Property-level analytics: the fetch is keyed by credentials alone,
        // so a per-environment fan-out would emit one identical alert per env.
        false
    }

    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        let access_token = ctx
            .credentials
            .oauth_token
            .as_deref()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("ga4 oauth token".into()))?;

        let property_id = ctx
            .credentials
            .site_id
            .as_deref()
            .filter(|property| !property.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("ga4 property id".into()))?;

        let data =
            crate::integrations::google_analytics::fetch_analytics(access_token, property_id, 30)
                .await
                .map_err(classify_ga4_error)?;

        Ok(PollOutput {
            work_items: vec![],
            alerts: build_ga4_alerts(ctx.project_id, &ctx.env_url, property_id, &data),
            partial: false,
            unobserved_signal_prefixes: Vec::new(),
        })
    }
}

fn build_ga4_alerts(
    project_id: i64,
    env_url: &str,
    property_id: &str,
    data: &GA4Data,
) -> Vec<AlertInput> {
    if data.daily.len() < 7 {
        return Vec::new();
    }

    let avg: f64 = data
        .daily
        .iter()
        .map(|point| point.users as f64)
        .sum::<f64>()
        / data.daily.len() as f64;
    if avg <= 10.0 {
        return Vec::new();
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    data.daily
        .iter()
        .filter_map(|point| {
            build_ga4_point_alert(project_id, env_url, property_id, point, avg, now_ms)
        })
        .collect()
}

fn build_ga4_point_alert(
    project_id: i64,
    env_url: &str,
    property_id: &str,
    point: &GA4DailyPoint,
    avg_users: f64,
    observed_at: i64,
) -> Option<AlertInput> {
    let users = point.users as f64;
    let ratio = users / avg_users;
    let (kind, severity, direction, pct) = if ratio > 1.5 {
        ("spike", "info", "above average", (ratio - 1.0) * 100.0)
    } else if ratio < 0.5 {
        ("drop", "warn", "below average", (1.0 - ratio) * 100.0)
    } else {
        return None;
    };
    let title = format!(
        "GA traffic {kind}: {} users ({pct:.0}% {direction})",
        point.users
    );
    let description = format!(
        "GA4 saw {} active users on {}, compared with a {:.0}/day 30-day average. Check deploys, campaigns, tracking changes, and outage history before assuming this is normal.",
        point.users, point.date, avg_users
    );

    let occurred_at = chrono::DateTime::parse_from_rfc3339(&format!("{}T12:00:00Z", point.date))
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(observed_at);

    Some(AlertInput {
        project_id,
        env_url: Some(env_url.to_string()),
        source: "ga4".to_string(),
        alert_id: format!("{kind}:{}", point.date),
        severity: severity.to_string(),
        title,
        description,
        detail_json: Some(
            serde_json::json!({
                "alert_type": format!("ga4_traffic_{kind}"),
                "property_id": property_id,
                "users": point.users,
                "sessions": point.sessions,
                "pageviews": point.pageviews,
                "avg_users": avg_users,
                "ratio": ratio,
                "date": point.date,
                "url": env_url,
                "destination": "analytics"
            })
            .to_string(),
        ),
        occurred_at,
        observed_at,
    })
}

fn classify_ga4_error(error: String) -> AdapterError {
    if error.contains("401 Unauthorized")
        || error.contains("403 Forbidden")
        || error.contains("UNAUTHENTICATED")
        || error.contains("PERMISSION_DENIED")
    {
        AdapterError::AuthFailed(
            "Google Analytics credentials were rejected; reconnect Google Analytics".into(),
        )
    } else {
        AdapterError::Transport(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::adapters::Credentials;
    use crate::integrations::google_analytics::GA4Data;

    fn data_with_daily(users: &[u64]) -> GA4Data {
        GA4Data {
            active_users: users.iter().sum(),
            sessions: 0,
            pageviews: 0,
            bounce_rate: 0.0,
            avg_session_duration: 0.0,
            top_pages: Vec::new(),
            top_sources: Vec::new(),
            top_countries: Vec::new(),
            daily: users
                .iter()
                .enumerate()
                .map(|(index, users)| GA4DailyPoint {
                    date: format!("2026-05-{:02}", index + 1),
                    users: *users,
                    sessions: *users,
                    pageviews: *users * 2,
                })
                .collect(),
        }
    }

    #[test]
    fn source_and_cadence() {
        assert_eq!(Ga4Adapter.source(), "ga4");
        assert_eq!(Ga4Adapter.cadence(), Duration::from_secs(300));
    }

    #[test]
    fn is_configured_requires_oauth_and_property() {
        assert!(!Ga4Adapter.is_configured(&Credentials::empty()));
        assert!(!Ga4Adapter.is_configured(&Credentials {
            api_key: None,
            oauth_token: Some("token".into()),
            site_id: None,
            github: None,
            github_unobservable: false,
        }));
        assert!(Ga4Adapter.is_configured(&Credentials {
            api_key: None,
            oauth_token: Some("token".into()),
            site_id: Some("123456".into()),
            github: None,
            github_unobservable: false,
        }));
    }

    #[test]
    fn traffic_drop_emits_alert_after_baseline_guard() {
        let data = data_with_daily(&[30, 32, 31, 29, 35, 33, 5]);
        let alerts = build_ga4_alerts(7, "https://example.com", "123456", &data);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].source, "ga4");
        assert_eq!(alerts[0].severity, "warn");
        assert!(alerts[0].alert_id.starts_with("drop:"));
        assert!(alerts[0].title.contains("below average"));
        assert!(alerts[0].description.contains("tracking changes"));
    }

    #[test]
    fn low_traffic_sites_do_not_emit_alerts() {
        let data = data_with_daily(&[1, 2, 3, 2, 1, 2, 20]);
        let alerts = build_ga4_alerts(7, "https://example.com", "123456", &data);

        assert!(alerts.is_empty());
    }

    #[test]
    fn classify_ga4_error_marks_rejected_credentials() {
        let error = classify_ga4_error("GA4 API returned 403 Forbidden".into());
        assert!(matches!(error, AdapterError::AuthFailed(_)));
    }
}
