//! Polls Cloudflare for blocked-threat alerts and cache enrichments.
//!
//! Zone-wide results are project-scoped, and date-stable alert IDs make polls
//! idempotent.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::db::alerts::AlertInput;
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use crate::integrations::cloudflare;

pub struct CloudflareAdapter {
    db: Arc<crate::db::Database>,
}

impl CloudflareAdapter {
    #[tracing::instrument(skip(db))]
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self { db }
    }
}

fn classify_cloudflare_error(error: String) -> AdapterError {
    if error.contains("401 Unauthorized")
        || error.contains("403 Forbidden")
        || error.contains("404 Not Found")
    {
        AdapterError::AuthFailed(error)
    } else {
        AdapterError::Transport(error)
    }
}

fn build_threat_alert(
    project_id: i64,
    env_url: &str,
    data: &cloudflare::CloudflareData,
    observed_at: i64,
    utc_day: &str,
) -> Option<AlertInput> {
    if data.threats_blocked == 0 {
        return None;
    }

    let severity = if data.threats_blocked > 10 {
        "critical"
    } else {
        "warn"
    };
    let request_label = if data.threats_blocked == 1 {
        "request"
    } else {
        "requests"
    };

    Some(AlertInput {
        project_id,
        env_url: Some(env_url.to_string()),
        source: "cloudflare".to_string(),
        // Stable per-day ID - matches source_id in refresh_cloudflare_events.
        alert_id: format!("cf_threats_7d_{utc_day}"),
        severity: severity.to_string(),
        title: format!(
            "Cloudflare blocked {} threat {}",
            data.threats_blocked, request_label
        ),
        description: format!(
            "{} {} Cloudflare classified as threats were blocked in the last 7 days. Review Security Events for source IPs, rules, and paths before changing firewall settings.",
            data.threats_blocked, request_label
        ),
        // Cloudflare WAF events have no corresponding in-app destination.
        detail_json: serde_json::to_string(&serde_json::json!({
            "alert_type": "cloudflare_threats_blocked",
            "threats": data.threats_blocked,
            "requests": data.requests_total,
            "cached": data.requests_cached,
            "period": "7d",
            "url": env_url,
        }))
        .ok(),
        occurred_at: observed_at,
        observed_at,
    })
}

#[async_trait]
impl IntegrationAdapter for CloudflareAdapter {
    fn source(&self) -> &'static str {
        "cloudflare"
    }

    fn cadence(&self) -> Duration {
        // allow-inline-duration: per-adapter polling cadence; lives with the
        // adapter so quota/cost characteristics stay co-located.
        Duration::from_secs(300) // 5 minutes
    }

    fn is_configured(&self, credentials: &crate::integrations::adapters::Credentials) -> bool {
        credentials.has_api_key() && credentials.has_site_id()
    }

    fn env_scoped(&self) -> bool {
        // Zone-level stats: the fetch is keyed by credentials alone, so a
        // per-environment fan-out would emit one identical alert per env.
        false
    }

    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        let api_key = ctx
            .credentials
            .api_key
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("cloudflare".to_string()))?;

        let zone_id = ctx
            .credentials
            .site_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("cloudflare (zone_id)".to_string()))?;

        let data = cloudflare::fetch_stats_with_period(api_key, zone_id, "7d")
            .await
            .map_err(classify_cloudflare_error)?;

        write_v3_enrichments(&self.db, ctx.project_id, &data);

        let now_ms = chrono::Utc::now().timestamp_millis();
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let alerts = build_threat_alert(ctx.project_id, &ctx.env_url, &data, now_ms, &today)
            .into_iter()
            .collect();

        Ok(PollOutput {
            work_items: vec![],
            alerts,
            partial: false,
            unobserved_signal_prefixes: Vec::new(),
        })
    }
}

/// Cache enrichments supported by the current aggregate Cloudflare response.
/// Status-code and bot signals require additional GraphQL fields.
pub fn write_v3_enrichments(
    db: &crate::db::Database,
    project_id: i64,
    data: &cloudflare::CloudflareData,
) {
    use crate::core::correlation::enrichments::write_cache_payload;

    // Convert Cloudflare's percentage to the stored 0-1 fraction.
    if data.requests_total > 0 {
        let value = (data.cache_hit_rate / 100.0) as f32;
        let payload = serde_json::json!({ "value": value });
        if let Err(e) = write_cache_payload(
            db,
            project_id,
            "cloudflare",
            "cache_hit_rate",
            &payload.to_string(),
        ) {
            tracing::warn!("cloudflare: failed to write cache_hit_rate cache: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::{temp_db, temp_db_arc, temp_db_with_project};
    use crate::integrations::adapters::Credentials;

    fn test_adapter() -> CloudflareAdapter {
        let db = temp_db_arc();
        CloudflareAdapter::new(db.db.clone())
    }

    #[tokio::test]
    async fn returns_missing_credentials_when_creds_empty() {
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
    async fn returns_missing_credentials_when_zone_id_empty() {
        let adapter = test_adapter();
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials {
                api_key: Some("test-key".to_string()),
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
        assert_eq!(test_adapter().source(), "cloudflare");
        assert_eq!(test_adapter().cadence(), Duration::from_secs(300));
    }

    #[test]
    fn is_configured_requires_api_key_and_zone_id() {
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
            site_id: Some("zone-id".into()),
            github: None,
            github_unobservable: false,
        }));
    }

    #[test]
    fn cloudflare_auth_statuses_are_not_logged_as_transport_failures() {
        let err = classify_cloudflare_error("Cloudflare API returned 404 Not Found".to_string());
        assert!(matches!(err, AdapterError::AuthFailed(_)));

        let err = classify_cloudflare_error("Cloudflare REST error: timeout".to_string());
        assert!(matches!(err, AdapterError::Transport(_)));
    }

    #[test]
    fn threat_alert_describes_blocked_requests_as_review_context() {
        let data = cloudflare::CloudflareData {
            requests_total: 1_000,
            requests_cached: 400,
            cache_hit_rate: 40.0,
            bandwidth_total: 0,
            bandwidth_cached: 0,
            threats_blocked: 12,
            page_views: 0,
            unique_visitors: 0,
        };

        let alert = build_threat_alert(7, "https://example.com", &data, 1_000, "2026-05-16")
            .expect("threat alert");

        assert_eq!(alert.severity, "critical");
        assert_eq!(alert.title, "Cloudflare blocked 12 threat requests");
        assert!(alert.description.contains("Review Security Events"));
        let detail = alert.detail_json.unwrap();
        assert!(detail.contains("cloudflare_threats_blocked"));
        assert!(
            !detail.contains("destination"),
            "threat alerts must not deep-link to an unrelated in-app page: {detail}"
        );
    }

    #[test]
    fn write_v3_enrichments_writes_cache_hit_rate_as_fraction() {
        use crate::core::correlation::enrichments::{cf_cache_hit_rate, EnrichmentCache};
        use crate::core::types_work_items::Enrichment;

        let db = temp_db_with_project();
        let data = cloudflare::CloudflareData {
            requests_total: 1_000,
            requests_cached: 850,
            cache_hit_rate: 85.0, // stored as percentage in CloudflareData
            bandwidth_total: 0,
            bandwidth_cached: 0,
            threats_blocked: 0,
            page_views: 0,
            unique_visitors: 0,
        };

        write_v3_enrichments(&db, 1, &data);

        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let enr =
            cf_cache_hit_rate("performance.cache_headers", &cache).expect("read should not error");
        assert!(enr.is_some(), "cache_hit_rate enrichment should be written");
        if let Some(Enrichment::CacheHitRate { value, source }) = enr {
            // stored as fraction: 85.0 / 100.0 = 0.85
            assert!(
                (value - 0.85_f32).abs() < 0.01,
                "expected ~0.85, got {value}"
            );
            assert_eq!(source, "cloudflare");
        } else {
            panic!("unexpected enrichment variant");
        }
    }

    #[test]
    fn write_v3_enrichments_skips_cache_hit_rate_when_no_requests() {
        use crate::core::correlation::enrichments::{cf_cache_hit_rate, EnrichmentCache};

        let db = temp_db();
        let data = cloudflare::CloudflareData {
            requests_total: 0,
            requests_cached: 0,
            cache_hit_rate: 0.0,
            bandwidth_total: 0,
            bandwidth_cached: 0,
            threats_blocked: 0,
            page_views: 0,
            unique_visitors: 0,
        };

        write_v3_enrichments(&db, 1, &data);

        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let enr =
            cf_cache_hit_rate("performance.cache_headers", &cache).expect("read should not error");
        assert!(
            enr.is_none(),
            "no cache_hit_rate enrichment when requests_total is 0"
        );
    }
}
