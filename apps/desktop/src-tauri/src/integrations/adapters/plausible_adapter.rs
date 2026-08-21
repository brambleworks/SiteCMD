//! Polls Plausible for traffic anomalies and correlation enrichments.
//!
//! Date-stable alert IDs make repeated polls idempotent.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::db::alerts::AlertInput;
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use crate::integrations::plausible::{TimeseriesPoint, TopPage};

pub struct PlausibleAdapter {
    db: Arc<crate::db::Database>,
}

impl PlausibleAdapter {
    #[tracing::instrument(skip(db))]
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self { db }
    }
}

fn classify_plausible_error(error: String) -> AdapterError {
    if error.contains("401 Unauthorized") || error.contains("403 Forbidden") {
        AdapterError::AuthFailed(
            "Plausible credentials were rejected; reconnect or update the API key".into(),
        )
    } else {
        AdapterError::Transport(error)
    }
}

#[async_trait]
impl IntegrationAdapter for PlausibleAdapter {
    fn source(&self) -> &'static str {
        "plausible"
    }

    fn cadence(&self) -> Duration {
        // allow-inline-duration: per-adapter polling cadence.
        Duration::from_secs(300) // 5 min
    }

    fn is_configured(&self, credentials: &crate::integrations::adapters::Credentials) -> bool {
        credentials.has_api_key() && credentials.has_site_id()
    }

    fn env_scoped(&self) -> bool {
        // Site-level analytics: the fetch is keyed by credentials alone, so a
        // per-environment fan-out would emit one identical alert per env.
        false
    }

    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        let api_key = ctx
            .credentials
            .api_key
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("plausible".into()))?;

        let site_id = ctx
            .credentials
            .site_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("plausible (site_id)".into()))?;

        let data = crate::integrations::plausible::fetch_analytics(api_key, site_id, "30d")
            .await
            .map_err(classify_plausible_error)?;

        write_v3_enrichments(&self.db, ctx.project_id, api_key, site_id).await;

        // Suppress anomaly detection for short or low-traffic series where ratios
        // are too noisy to be useful.
        let mut alerts: Vec<AlertInput> = Vec::new();

        if data.points.len() >= 7 {
            let avg: f64 = data.points.iter().map(|p| p.visitors as f64).sum::<f64>()
                / data.points.len() as f64;

            if avg > 10.0 {
                let now_ms = chrono::Utc::now().timestamp_millis();

                for point in &data.points {
                    if let Some(alert) = build_plausible_point_alert(
                        ctx.project_id,
                        &ctx.env_url,
                        point,
                        avg,
                        now_ms,
                    ) {
                        alerts.push(alert);
                    }
                }
            }
        }

        Ok(PollOutput {
            work_items: vec![],
            alerts,
            partial: false,
            unobserved_signal_prefixes: Vec::new(),
        })
    }
}

fn build_plausible_point_alert(
    project_id: i64,
    env_url: &str,
    point: &TimeseriesPoint,
    avg_visitors: f64,
    observed_at: i64,
) -> Option<AlertInput> {
    let ratio = point.visitors as f64 / avg_visitors;
    let (kind, severity, direction, pct) = if ratio > 1.5 {
        ("spike", "info", "above average", (ratio - 1.0) * 100.0)
    } else if ratio < 0.5 {
        ("drop", "warn", "below average", (1.0 - ratio) * 100.0)
    } else {
        return None;
    };
    let occurred_at = chrono::DateTime::parse_from_rfc3339(&format!("{}T12:00:00Z", point.date))
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(observed_at);

    Some(AlertInput {
        project_id,
        env_url: Some(env_url.to_string()),
        source: "plausible".to_string(),
        alert_id: format!("{kind}:{}", point.date),
        severity: severity.to_string(),
        title: format!(
            "Traffic {kind}: {} visitors ({pct:.0}% {direction})",
            point.visitors
        ),
        description: format!(
            "Plausible saw {} visitors on {}, compared with a {:.0}/day 30-day average. Check deploys, campaigns, tracking changes, and outage history before assuming this is normal.",
            point.visitors, point.date, avg_visitors
        ),
        detail_json: serde_json::to_string(&serde_json::json!({
            "alert_type": format!("plausible_traffic_{kind}"),
            "visitors": point.visitors,
            "pageviews": point.pageviews,
            "avg_visitors": avg_visitors,
            "ratio": ratio,
            "date": point.date,
            "url": env_url,
            "destination": "analytics"
        }))
        .ok(),
        occurred_at,
        observed_at,
    })
}

/// Cache the largest page-level visitor drop between adjacent seven-day windows.
pub async fn write_v3_enrichments(
    db: &crate::db::Database,
    project_id: i64,
    api_key: &str,
    site_id: &str,
) {
    use crate::integrations::plausible::fetch_top_pages_for_window;

    let (current_result, prior_result) = tokio::join!(
        fetch_top_pages_for_window(api_key, site_id, 7, 0),
        fetch_top_pages_for_window(api_key, site_id, 7, 7),
    );

    let current = match current_result {
        Ok(pages) => pages,
        Err(e) => {
            tracing::debug!(
                "plausible: skipping top_falling_page, current fetch failed: {}",
                e
            );
            return;
        }
    };
    let prior = match prior_result {
        Ok(pages) => pages,
        Err(e) => {
            tracing::debug!(
                "plausible: skipping top_falling_page, prior fetch failed: {}",
                e
            );
            return;
        }
    };

    if let Some((url, pct_drop)) = compute_top_falling_page(&current, &prior) {
        let payload = serde_json::json!({ "url": url, "pct_drop": pct_drop });
        if let Err(e) = crate::core::correlation::enrichments::write_cache_payload(
            db,
            project_id,
            "plausible",
            "top_falling_page",
            &payload.to_string(),
        ) {
            tracing::warn!("plausible: failed to write top_falling_page cache: {}", e);
        }
    }
}

/// Return the largest page-level visitor drop, excluding pages below the
/// 100-visitor noise floor. A page absent from the current window drops 100%.
pub fn compute_top_falling_page(current: &[TopPage], prior: &[TopPage]) -> Option<(String, f32)> {
    let prior_map: std::collections::HashMap<&str, u64> = prior
        .iter()
        .map(|tp| (tp.page.as_str(), tp.visitors))
        .collect();

    let current_map: std::collections::HashMap<&str, u64> = current
        .iter()
        .map(|tp| (tp.page.as_str(), tp.visitors))
        .collect();

    let mut worst: Option<(String, f32)> = None;

    for tp in current {
        let prior_v = prior_map.get(tp.page.as_str()).copied().unwrap_or(0);
        if prior_v < 100 {
            continue;
        }
        if tp.visitors >= prior_v {
            continue;
        }
        let drop = ((prior_v - tp.visitors) as f32 / prior_v as f32) * 100.0;
        if worst.as_ref().map(|(_, d)| drop > *d).unwrap_or(true) {
            worst = Some((tp.page.clone(), drop));
        }
    }

    for (page, prior_v) in &prior_map {
        if *prior_v < 100 {
            continue;
        }
        if current_map.contains_key(page) {
            continue;
        }
        let drop = 100.0_f32;
        if worst.as_ref().map(|(_, d)| drop > *d).unwrap_or(true) {
            worst = Some((page.to_string(), drop));
        }
    }

    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::{temp_db_arc, temp_db_with_project};
    use crate::integrations::adapters::Credentials;

    fn test_adapter() -> PlausibleAdapter {
        let db = temp_db_arc();
        PlausibleAdapter::new(db.db.clone())
    }

    #[tokio::test]
    async fn returns_missing_credentials_when_api_key_empty() {
        let adapter = test_adapter();
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials::empty(),
        };
        let err = adapter.poll(&ctx).await.unwrap_err();
        assert!(matches!(err, AdapterError::MissingCredentials(_)));
    }

    #[tokio::test]
    async fn returns_missing_credentials_when_site_id_empty() {
        let adapter = test_adapter();
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials {
                api_key: Some("test-key".into()),
                oauth_token: None,
                site_id: None,
                github: None,
                github_unobservable: false,
            },
        };
        let err = adapter.poll(&ctx).await.unwrap_err();
        assert!(matches!(err, AdapterError::MissingCredentials(_)));
    }

    #[test]
    fn source_and_cadence() {
        assert_eq!(test_adapter().source(), "plausible");
        assert_eq!(test_adapter().cadence(), Duration::from_secs(300));
    }

    #[test]
    fn is_configured_requires_api_key_and_site_id() {
        let adapter = test_adapter();
        assert!(!adapter.is_configured(&Credentials::empty()));
        assert!(!adapter.is_configured(&Credentials {
            api_key: Some("test-key".into()),
            oauth_token: None,
            site_id: None,
            github: None,
            github_unobservable: false,
        }));
        assert!(adapter.is_configured(&Credentials {
            api_key: Some("test-key".into()),
            oauth_token: None,
            site_id: Some("example.com".into()),
            github: None,
            github_unobservable: false,
        }));
    }

    #[test]
    fn classify_plausible_error_marks_rejected_credentials() {
        let error = classify_plausible_error("Plausible API returned 401 Unauthorized".into());
        assert!(matches!(error, AdapterError::AuthFailed(_)));
    }

    #[test]
    fn classify_plausible_error_keeps_network_errors_as_transport() {
        let error = classify_plausible_error("connection timed out".into());
        assert!(matches!(error, AdapterError::Transport(_)));
    }

    #[test]
    fn traffic_drop_alert_names_the_baseline_direction() {
        let point = TimeseriesPoint {
            date: "2026-05-16".into(),
            visitors: 5,
            pageviews: 8,
            bounce_rate: 0.0,
            visit_duration: 0.0,
        };

        let alert = build_plausible_point_alert(7, "https://example.com", &point, 25.0, 1_000)
            .expect("traffic drop alert");

        assert_eq!(alert.severity, "warn");
        assert_eq!(alert.title, "Traffic drop: 5 visitors (80% below average)");
        assert!(alert
            .description
            .contains("deploys, campaigns, tracking changes"));
        assert!(alert
            .detail_json
            .unwrap()
            .contains("plausible_traffic_drop"));
    }

    fn pages(data: &[(&str, u64)]) -> Vec<TopPage> {
        data.iter()
            .map(|(page, visitors)| TopPage {
                page: page.to_string(),
                visitors: *visitors,
            })
            .collect()
    }

    #[test]
    fn compute_top_falling_page_picks_worst_drop() {
        let prior = pages(&[("/home", 500), ("/pricing", 300), ("/blog", 200)]);
        let current = pages(&[("/home", 450), ("/pricing", 150), ("/blog", 190)]);
        let result = compute_top_falling_page(&current, &prior);
        assert!(result.is_some(), "should find a falling page");
        let (url, pct_drop) = result.unwrap();
        assert_eq!(url, "/pricing");
        assert!(
            (pct_drop - 50.0_f32).abs() < 0.1,
            "expected ~50% drop, got {pct_drop}"
        );
    }

    #[test]
    fn compute_top_falling_page_handles_disappeared_page() {
        let prior = pages(&[("/home", 500), ("/docs", 200)]);
        let current = pages(&[("/home", 490)]);
        let result = compute_top_falling_page(&current, &prior);
        assert!(result.is_some());
        let (url, pct_drop) = result.unwrap();
        assert_eq!(url, "/docs");
        assert!((pct_drop - 100.0_f32).abs() < 0.1);
    }

    #[test]
    fn compute_top_falling_page_returns_none_when_no_drops() {
        let prior = pages(&[("/home", 500), ("/pricing", 300)]);
        let current = pages(&[("/home", 600), ("/pricing", 350)]);
        let result = compute_top_falling_page(&current, &prior);
        assert!(result.is_none(), "no drops means None");
    }

    #[test]
    fn compute_top_falling_page_ignores_low_traffic_pages() {
        let prior = pages(&[("/obscure", 50), ("/home", 500)]);
        let current = pages(&[("/home", 490)]);
        let result = compute_top_falling_page(&current, &prior);
        let (url, _) = result.expect("should find /home drop");
        assert_eq!(url, "/home");
    }

    #[test]
    fn compute_top_falling_page_returns_none_on_empty_inputs() {
        assert!(compute_top_falling_page(&[], &[]).is_none());
        let prior = pages(&[("/home", 500)]);
        let result = compute_top_falling_page(&[], &prior);
        assert!(result.is_some());
    }

    #[test]
    fn write_v3_enrichments_round_trips_via_db() {
        use crate::core::correlation::enrichments::{
            plausible_top_falling_page, write_cache_payload, EnrichmentCache,
        };

        let db = temp_db_with_project();
        let project_id: i64 = 1;

        write_cache_payload(
            &db,
            project_id,
            "plausible",
            "top_falling_page",
            r#"{"url":"/checkout","pct_drop":42.5}"#,
        )
        .expect("write should succeed");

        let cache = EnrichmentCache::load(&db, project_id).expect("load cache");
        let result =
            plausible_top_falling_page("analytics.traffic-drop", &cache).expect("no error");
        assert!(result.is_some(), "written payload should be readable back");
        if let Some(crate::core::types_work_items::Enrichment::TopFallingPage {
            url,
            pct_drop,
            source,
        }) = result
        {
            assert_eq!(url, "/checkout");
            assert!((pct_drop - 42.5_f32).abs() < 0.01);
            assert_eq!(source, "plausible");
        } else {
            panic!("wrong enrichment variant");
        }
    }
}
