//! Google Search Console query-regression and enrichment signals.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::checks::Severity;
use crate::core::correlation::signal_mapping::resolve_check_id;
use crate::db::alerts::AlertInput;
use crate::db::work_items::{WorkItemInput, WorkItemMetadata};
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use crate::integrations::search_console::{IndexCoverageIssue, QueryRegression, SearchDailyPoint};

pub struct GscAdapter {
    db: Arc<crate::db::Database>,
}

impl GscAdapter {
    #[tracing::instrument(skip(db))]
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl IntegrationAdapter for GscAdapter {
    fn source(&self) -> &'static str {
        "gsc"
    }

    fn cadence(&self) -> Duration {
        // allow-inline-duration: per-adapter polling cadence.
        Duration::from_secs(3600) // 1 hour
    }

    fn is_configured(&self, credentials: &crate::integrations::adapters::Credentials) -> bool {
        credentials.has_oauth_token() && credentials.has_site_id()
    }

    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        let token = ctx
            .credentials
            .oauth_token
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("gsc".into()))?;
        let site_url = ctx
            .credentials
            .site_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("gsc (site_id)".into()))?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut work_items = Vec::new();
        let mut alerts = Vec::new();

        let urls = vec![ctx.env_url.clone()];
        let coverage_issues: Vec<IndexCoverageIssue> =
            match crate::integrations::search_console::fetch_index_coverage_issues(
                token, site_url, &urls,
            )
            .await
            {
                Ok(issues) => issues,
                // A failed coverage fetch is unobserved, not an empty result that
                // can resolve every open GSC finding.
                Err(e) => {
                    tracing::warn!("gsc_adapter: index coverage fetch failed: {}", e);
                    return Err(AdapterError::from_google_http_error(e));
                }
            };

        for issue in &coverage_issues {
            let category =
                if issue.reason == "touch-target-size" || issue.reason == "text-too-small" {
                    "accessibility".to_string()
                } else {
                    "seo".to_string()
                };
            work_items.push(WorkItemInput {
                project_id: ctx.project_id,
                env_url: ctx.env_url.clone(),
                source: "gsc".into(),
                signal_id: format!("gsc:{}:{}", issue.reason, issue.page_url),
                check_id: resolve_check_id("gsc", &issue.reason),
                category,
                severity: gsc_indexing_severity(&issue.reason),
                title: gsc_reason_title(&issue.reason),
                description: issue
                    .detail
                    .clone()
                    .unwrap_or_else(|| format!("Google reported {} for this page.", issue.reason)),
                detail_json: None,
                scan_ref: None,
                page_url: Some(issue.page_url.clone()),
                fix_prompt: None,
                manual_fix: None,
                why_it_matters: None,
                observed_at: now_ms,
                metadata: WorkItemMetadata::default(),
            });
        }

        match crate::integrations::search_console::fetch_query_comparison(
            token, site_url, 7, 0.20, 100,
        )
        .await
        {
            Ok(regressions) => {
                for q in regressions {
                    alerts.push(build_query_drop_alert(
                        ctx.project_id,
                        &ctx.env_url,
                        &q,
                        now_ms,
                    ));
                }
            }
            Err(e) => tracing::warn!("gsc_adapter: query comparison failed: {}", e),
        }

        match crate::integrations::search_console::fetch_analytics(token, site_url, 28).await {
            Ok(data) => {
                write_v3_enrichments(&self.db, ctx.project_id, &data.daily, &coverage_issues);
            }
            Err(e) => tracing::warn!("gsc_adapter: analytics fetch for enrichments failed: {}", e),
        }

        Ok(PollOutput {
            work_items,
            alerts,
            partial: false,
            unobserved_signal_prefixes: Vec::new(),
        })
    }
}

fn build_query_drop_alert(
    project_id: i64,
    env_url: &str,
    regression: &QueryRegression,
    observed_at: i64,
) -> AlertInput {
    let drop_percent = if regression.previous_impressions > 0 {
        100.0
            * (1.0 - regression.current_impressions as f64 / regression.previous_impressions as f64)
    } else {
        0.0
    };

    AlertInput {
        project_id,
        env_url: Some(env_url.to_string()),
        source: "gsc".into(),
        alert_id: format!(
            "query-drop:{}:{}",
            regression.query, regression.detected_at
        ),
        severity: "warn".into(),
        title: format!("Search impressions down: {}", regression.query),
        description: format!(
            "{} impressions vs {} in the previous 7-day window ({drop_percent:.0}% drop). Average position is {:.1} vs {:.1}; check rank changes, indexing, seasonality, and content changes before rewriting the page.",
            regression.current_impressions,
            regression.previous_impressions,
            regression.current_position,
            regression.previous_position,
        ),
        detail_json: Some(
            serde_json::json!({
                "alert_type": "gsc_query_impression_drop",
                "query": regression.query,
                "previous_impressions": regression.previous_impressions,
                "current_impressions": regression.current_impressions,
                "previous_clicks": regression.previous_clicks,
                "current_clicks": regression.current_clicks,
                "previous_ctr": regression.previous_ctr,
                "current_ctr": regression.current_ctr,
                "previous_position": regression.previous_position,
                "current_position": regression.current_position,
                "drop_percent": drop_percent,
                "url": env_url,
                "destination": "search-console"
            })
            .to_string(),
        ),
        occurred_at: regression.detected_at,
        observed_at,
    }
}

fn gsc_indexing_severity(reason: &str) -> Severity {
    match reason {
        "not-indexed" | "crawl-error" | "blocked-by-robots" => Severity::High,
        "canonical-mismatch" | "duplicate-no-canonical" => Severity::Medium,
        "mobile-viewport"
        | "text-too-small"
        | "touch-target-size"
        | "content-wider-than-screen" => Severity::Medium,
        _ => Severity::Low,
    }
}

fn gsc_reason_title(reason: &str) -> String {
    match reason {
        "not-indexed" => "Page not indexed by Google".into(),
        "crawl-error" => "Google can't crawl this page".into(),
        "blocked-by-robots" => "Blocked by robots.txt".into(),
        "canonical-mismatch" => "Google chose a different canonical URL".into(),
        "duplicate-no-canonical" => "Duplicate page without canonical".into(),
        "mobile-viewport" => "Missing or broken mobile viewport".into(),
        "touch-target-size" => "Touch targets too small".into(),
        "text-too-small" => "Text too small on mobile".into(),
        "content-wider-than-screen" => "Content overflows mobile viewport".into(),
        other => format!("GSC issue: {}", other),
    }
}

/// Compute a meaningful impressions drop from 14+ days of daily GSC data.
/// Compares two seven-day windows and excludes fewer than 50 prior impressions
/// or drops below 15%.
pub fn compute_impressions_drop(daily: &[SearchDailyPoint]) -> Option<(u64, u64, u32)> {
    if daily.len() < 14 {
        return None;
    }

    let mut sorted: Vec<&SearchDailyPoint> = daily.iter().collect();
    sorted.sort_by(|a, b| a.date.cmp(&b.date));

    let total = sorted.len();
    let recent_impressions: u64 = sorted[total - 7..].iter().map(|p| p.impressions).sum();
    let prior_impressions: u64 = sorted[total - 14..total - 7]
        .iter()
        .map(|p| p.impressions)
        .sum();

    if prior_impressions < 50 {
        return None;
    }

    if recent_impressions >= prior_impressions {
        return None;
    }

    let drop_pct = (prior_impressions - recent_impressions) as f64 / prior_impressions as f64;

    if drop_pct < 0.15 {
        return None;
    }

    Some((prior_impressions, recent_impressions, 7))
}

/// Cache significant seven-day impression drops and recent GSC crawl errors.
/// Field Core Web Vitals require CrUX and are not available from this adapter.
pub fn write_v3_enrichments(
    db: &crate::db::Database,
    project_id: i64,
    daily: &[SearchDailyPoint],
    coverage_issues: &[IndexCoverageIssue],
) {
    use crate::core::correlation::enrichments::write_cache_payload;

    if let Some((from, to, days)) = compute_impressions_drop(daily) {
        let payload = serde_json::json!({ "from": from, "to": to, "days": days });
        if let Err(e) = write_cache_payload(
            db,
            project_id,
            "gsc",
            "search_impressions_drop",
            &payload.to_string(),
        ) {
            tracing::warn!("gsc: failed to write search_impressions_drop cache: {}", e);
        }
    }

    // Keep a count shape so future sitemap batching does not change the cache schema.
    let crawl_error_count = coverage_issues
        .iter()
        .filter(|issue| issue.reason.contains("crawl-error"))
        .count() as u32;

    if crawl_error_count > 0 {
        // 28 days is GSC's default lookback for coverage reports.
        let payload = serde_json::json!({ "count": crawl_error_count, "days": 28 });
        if let Err(e) = write_cache_payload(
            db,
            project_id,
            "gsc",
            "recent_crawl_errors",
            &payload.to_string(),
        ) {
            tracing::warn!("gsc: failed to write recent_crawl_errors cache: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db_arc;
    use crate::integrations::adapters::Credentials;

    fn test_adapter() -> GscAdapter {
        let db = temp_db_arc();
        GscAdapter::new(db.db.clone())
    }

    #[test]
    fn source_is_gsc() {
        assert_eq!(test_adapter().source(), "gsc");
    }

    #[test]
    fn cadence_is_1_hour() {
        assert_eq!(test_adapter().cadence(), Duration::from_secs(3600));
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
    async fn returns_missing_credentials_when_oauth_token_empty() {
        let adapter = test_adapter();
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials {
                api_key: None,
                oauth_token: Some("".to_string()),
                site_id: None,
                github: None,
                github_unobservable: false,
            },
        };
        let err = adapter.poll(&ctx).await.unwrap_err();
        assert!(matches!(err, AdapterError::MissingCredentials(_)));
    }

    #[tokio::test]
    async fn returns_missing_credentials_when_site_id_absent() {
        let adapter = test_adapter();
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials {
                api_key: None,
                oauth_token: Some("ya29.valid-token".to_string()),
                site_id: None,
                github: None,
                github_unobservable: false,
            },
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
                api_key: None,
                oauth_token: Some("ya29.valid-token".to_string()),
                site_id: Some("".to_string()),
                github: None,
                github_unobservable: false,
            },
        };
        let err = adapter.poll(&ctx).await.unwrap_err();
        assert!(matches!(err, AdapterError::MissingCredentials(_)));
    }

    #[test]
    fn is_configured_requires_oauth_token_and_site_id() {
        let adapter = test_adapter();
        assert!(!adapter.is_configured(&Credentials::empty()));
        assert!(!adapter.is_configured(&Credentials {
            api_key: None,
            oauth_token: Some("ya29.valid-token".into()),
            site_id: None,
            github: None,
            github_unobservable: false,
        }));
        assert!(adapter.is_configured(&Credentials {
            api_key: None,
            oauth_token: Some("ya29.valid-token".into()),
            site_id: Some("https://example.com/".into()),
            github: None,
            github_unobservable: false,
        }));
    }

    #[test]
    fn query_drop_alert_preserves_comparison_context() {
        let regression = QueryRegression {
            query: "sitecmd scanner".into(),
            previous_impressions: 200,
            current_impressions: 120,
            previous_clicks: 12,
            current_clicks: 4,
            previous_ctr: 0.06,
            current_ctr: 0.03,
            previous_position: 4.2,
            current_position: 7.8,
            detected_at: 1_000,
        };

        let alert = build_query_drop_alert(7, "https://example.com", &regression, 2_000);

        assert_eq!(alert.title, "Search impressions down: sitecmd scanner");
        assert!(alert.description.contains("40% drop"));
        assert!(alert
            .description
            .contains("rank changes, indexing, seasonality"));
        assert!(alert
            .detail_json
            .unwrap()
            .contains("gsc_query_impression_drop"));
    }

    fn make_daily(impressions_per_day: &[u64]) -> Vec<SearchDailyPoint> {
        impressions_per_day
            .iter()
            .enumerate()
            .map(|(i, &imp)| SearchDailyPoint {
                date: format!("2026-01-{:02}", i + 1),
                clicks: 0,
                impressions: imp,
                ctr: 0.0,
                position: 0.0,
            })
            .collect()
    }

    #[test]
    fn impressions_drop_detected_at_15_percent() {
        // Prior 7 days: 100/day = 700 total. Recent 7 days: 50/day = 350 total. 50% drop.
        let mut impressions = vec![100u64; 7];
        impressions.extend_from_slice(&[50u64; 7]);
        let daily = make_daily(&impressions);

        let result = compute_impressions_drop(&daily);
        assert!(result.is_some(), "50% drop should be detected");
        let (from, to, days) = result.unwrap();
        assert_eq!(from, 700);
        assert_eq!(to, 350);
        assert_eq!(days, 7);
    }

    #[test]
    fn impressions_drop_not_detected_below_threshold() {
        // Prior: 100/day = 700. Recent: 96/day = 672. Drop ~3.9% -- below 15%.
        let mut impressions = vec![100u64; 7];
        impressions.extend_from_slice(&[96u64; 7]);
        let daily = make_daily(&impressions);

        let result = compute_impressions_drop(&daily);
        assert!(result.is_none(), "3.9% drop is below 15% threshold");
    }

    #[test]
    fn impressions_drop_not_detected_at_exactly_14_percent() {
        // Prior: 100/day = 700. Recent: 86/day = 602. Drop ~14% -- just below threshold.
        let mut impressions = vec![100u64; 7];
        impressions.extend_from_slice(&[86u64; 7]);
        let daily = make_daily(&impressions);

        let result = compute_impressions_drop(&daily);
        assert!(result.is_none(), "14% drop should not cross 15% threshold");
    }

    #[test]
    fn impressions_drop_requires_meaningful_prior() {
        // Prior week: 4/day = 28 total (< 50 floor). Drop 100%. Still skip.
        let mut impressions = vec![4u64; 7];
        impressions.extend_from_slice(&[0u64; 7]);
        let daily = make_daily(&impressions);

        let result = compute_impressions_drop(&daily);
        assert!(result.is_none(), "prior < 50 impressions means no signal");
    }

    #[test]
    fn impressions_drop_requires_14_days() {
        // Only 13 data points.
        let daily = make_daily(&[100u64; 13]);
        let result = compute_impressions_drop(&daily);
        assert!(result.is_none(), "fewer than 14 points returns None");
    }

    #[test]
    fn impressions_drop_not_emitted_when_recent_is_higher() {
        // Impressions grew: no drop.
        let mut impressions = vec![50u64; 7];
        impressions.extend_from_slice(&[100u64; 7]);
        let daily = make_daily(&impressions);

        let result = compute_impressions_drop(&daily);
        assert!(result.is_none(), "no drop when recent > prior");
    }

    #[test]
    fn impressions_drop_uses_last_14_of_longer_series() {
        // 21 days: first 7 have 200/day (ignored), last 14 have 100/prior 50/recent.
        let mut impressions = vec![200u64; 7]; // oldest 7 -- should be ignored
        impressions.extend_from_slice(&[100u64; 7]); // prior window
        impressions.extend_from_slice(&[50u64; 7]); // recent window
        let daily = make_daily(&impressions);

        let result = compute_impressions_drop(&daily);
        assert!(result.is_some());
        let (from, to, _) = result.unwrap();
        assert_eq!(from, 700); // 7 * 100
        assert_eq!(to, 350); // 7 * 50
    }

    #[test]
    fn write_v3_enrichments_writes_impressions_drop_and_crawl_errors() {
        use crate::core::correlation::enrichments::{
            gsc_recent_crawl_errors, gsc_search_impressions_drop, EnrichmentCache,
        };
        use crate::db::test_helpers::temp_db_with_project;

        let db = temp_db_with_project();

        let mut impressions = vec![100u64; 7];
        impressions.extend_from_slice(&[50u64; 7]);
        let daily = make_daily(&impressions);

        let coverage_issues = vec![IndexCoverageIssue {
            page_url: "https://example.com/".into(),
            reason: "crawl-error".into(),
            detail: Some("DNS error".into()),
        }];

        write_v3_enrichments(&db, 1, &daily, &coverage_issues);

        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let drop_enrichment =
            gsc_search_impressions_drop("seo.impressions-drop", &cache).expect("no error");
        assert!(
            drop_enrichment.is_some(),
            "search_impressions_drop should be written"
        );

        let crawl_enrichment =
            gsc_recent_crawl_errors("seo.crawl-errors", &cache).expect("no error");
        assert!(
            crawl_enrichment.is_some(),
            "recent_crawl_errors should be written"
        );
    }

    #[test]
    fn write_v3_enrichments_skips_drop_when_below_threshold() {
        use crate::core::correlation::enrichments::{gsc_search_impressions_drop, EnrichmentCache};
        use crate::db::test_helpers::temp_db;

        let db = temp_db();

        // Only 5% drop -- should not write.
        let mut impressions = vec![100u64; 7];
        impressions.extend_from_slice(&[95u64; 7]);
        let daily = make_daily(&impressions);

        write_v3_enrichments(&db, 1, &daily, &[]);

        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let result = gsc_search_impressions_drop("seo.impressions-drop", &cache).expect("no error");
        assert!(
            result.is_none(),
            "5% drop is below threshold, should not write"
        );
    }

    #[test]
    fn write_v3_enrichments_skips_crawl_errors_when_none() {
        use crate::core::correlation::enrichments::{gsc_recent_crawl_errors, EnrichmentCache};
        use crate::db::test_helpers::temp_db;

        let db = temp_db();
        let daily = make_daily(&[100u64; 7]); // only 7 days, impressions drop skipped too

        // No crawl errors.
        write_v3_enrichments(&db, 1, &daily, &[]);

        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let result = gsc_recent_crawl_errors("seo.crawl-errors", &cache).expect("no error");
        assert!(result.is_none(), "no crawl errors means no enrichment row");
    }
}
