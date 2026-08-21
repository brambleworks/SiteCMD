//! Readers for the shared integration enrichment cache.
//! Stale rows return `None` until an adapter refreshes them.

use std::collections::HashMap;

use serde::Deserialize;

use crate::core::types_work_items::Enrichment;
use crate::db::Database;

const CACHE_TTL_MS: i64 = 5 * 60 * 1000;

#[derive(Default)]
pub struct EnrichmentCache {
    payloads: HashMap<(String, String), String>,
}

impl EnrichmentCache {
    pub fn load(db: &Database, project_id: i64) -> Result<Self, String> {
        let cutoff_ms = chrono::Utc::now().timestamp_millis() - CACHE_TTL_MS;
        let rows = db
            .get_fresh_enrichment_payloads(project_id, cutoff_ms)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            payloads: rows
                .into_iter()
                .map(|(integration, signal_key, payload)| ((integration, signal_key), payload))
                .collect(),
        })
    }

    fn read<T: for<'de> Deserialize<'de>>(
        &self,
        integration: &str,
        signal_key: &str,
    ) -> Result<Option<T>, String> {
        let Some(payload) = self
            .payloads
            .get(&(integration.to_string(), signal_key.to_string()))
        else {
            return Ok(None);
        };
        serde_json::from_str::<T>(payload)
            .map(Some)
            .map_err(|error| {
                format!("invalid {integration}/{signal_key} enrichment payload: {error}")
            })
    }
}

/// Write a raw JSON payload into the integration enrichment cache.
///
/// Called by each integration adapter's `poll` fetch flow.
pub fn write_cache_payload(
    db: &Database,
    project_id: i64,
    integration: &str,
    signal_key: &str,
    payload_json: &str,
) -> Result<(), String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    db.upsert_enrichment_cache_payload(project_id, integration, signal_key, payload_json, now_ms)
        .map_err(|e| e.to_string())
}

// The GSC fetch flow does not write these cache payloads yet.

#[derive(Deserialize)]
struct FieldMetricCache {
    p75_ms: u32,
    url: String,
}

pub fn gsc_field_lcp(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<FieldMetricCache> = cache.read("gsc", "field_lcp")?;
    Ok(cached.map(|c| Enrichment::FieldLcp {
        p75_ms: c.p75_ms,
        url: c.url,
        source: "gsc".into(),
    }))
}

#[derive(Deserialize)]
struct FieldClsCache {
    value: f32,
    url: String,
}

pub fn gsc_field_cls(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<FieldClsCache> = cache.read("gsc", "field_cls")?;
    Ok(cached.map(|c| Enrichment::FieldCls {
        value: c.value,
        url: c.url,
        source: "gsc".into(),
    }))
}

pub fn gsc_field_inp(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<FieldMetricCache> = cache.read("gsc", "field_inp")?;
    Ok(cached.map(|c| Enrichment::FieldInp {
        p75_ms: c.p75_ms,
        url: c.url,
        source: "gsc".into(),
    }))
}

#[derive(Deserialize)]
struct ImpressionsDropCache {
    from: u32,
    to: u32,
    days: u32,
}

pub fn gsc_search_impressions_drop(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<ImpressionsDropCache> = cache.read("gsc", "search_impressions_drop")?;
    Ok(cached.map(|c| Enrichment::SearchImpressionsDrop {
        from: c.from,
        to: c.to,
        days: c.days,
        source: "gsc".into(),
    }))
}

#[derive(Deserialize)]
struct CrawlErrorsCache {
    count: u32,
    days: u32,
}

pub fn gsc_recent_crawl_errors(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<CrawlErrorsCache> = cache.read("gsc", "recent_crawl_errors")?;
    Ok(cached.map(|c| Enrichment::RecentCrawlErrors {
        count: c.count,
        days: c.days,
        source: "gsc".into(),
    }))
}

// The UptimeRobot poll flow does not write these cache payloads yet.

#[derive(Deserialize)]
struct DowntimeCache {
    window_start: String,
    window_end: String,
}

pub fn uptime_recent_downtime(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<DowntimeCache> = cache.read("uptimerobot", "recent_downtime")?;
    Ok(cached.map(|c| Enrichment::RecentDowntime {
        window_start: c.window_start,
        window_end: c.window_end,
        source: "uptimerobot".into(),
    }))
}

#[derive(Deserialize)]
struct CertExpiresCache {
    days: i64,
}

pub fn uptime_cert_expires_in(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<CertExpiresCache> = cache.read("uptimerobot", "cert_expires_in")?;
    Ok(cached.map(|c| Enrichment::CertExpiresIn {
        days: c.days,
        source: "uptimerobot".into(),
    }))
}

#[derive(Deserialize)]
struct CertChainCache {
    issues: Vec<String>,
}

pub fn uptime_cert_chain(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<CertChainCache> = cache.read("uptimerobot", "cert_chain")?;
    Ok(cached.map(|c| Enrichment::CertChain {
        issues: c.issues,
        source: "uptimerobot".into(),
    }))
}

#[derive(Deserialize)]
struct TtfbHistoryCache {
    p75_ms: u32,
    days: u32,
}

pub fn uptime_ttfb_history(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<TtfbHistoryCache> = cache.read("uptimerobot", "ttfb_history")?;
    Ok(cached.map(|c| Enrichment::TtfbHistory {
        p75_ms: c.p75_ms,
        days: c.days,
        source: "uptimerobot".into(),
    }))
}

// The Cloudflare fetch flow does not write these cache payloads yet.

#[derive(Deserialize)]
struct BotTrafficCache {
    value: f32,
}

pub fn cf_bot_traffic_pct(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<BotTrafficCache> = cache.read("cloudflare", "bot_traffic_pct")?;
    Ok(cached.map(|c| Enrichment::BotTrafficPct {
        value: c.value,
        source: "cloudflare".into(),
    }))
}

#[derive(Deserialize)]
struct CacheHitCache {
    value: f32,
}

pub fn cf_cache_hit_rate(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<CacheHitCache> = cache.read("cloudflare", "cache_hit_rate")?;
    Ok(cached.map(|c| Enrichment::CacheHitRate {
        value: c.value,
        source: "cloudflare".into(),
    }))
}

#[derive(Deserialize)]
struct FiveXxSpikeCache {
    rate: f32,
    started_at: String,
}

pub fn cf_recent_five_xx_spike(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<FiveXxSpikeCache> = cache.read("cloudflare", "recent_five_xx_spike")?;
    Ok(cached.map(|c| Enrichment::RecentFiveXxSpike {
        rate: c.rate,
        started_at: c.started_at,
        source: "cloudflare".into(),
    }))
}

#[derive(Deserialize)]
struct OriginErrorsCache {
    count: u32,
    days: u32,
}

pub fn cf_recent_origin_errors(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<OriginErrorsCache> = cache.read("cloudflare", "recent_origin_errors")?;
    Ok(cached.map(|c| Enrichment::RecentOriginErrors {
        count: c.count,
        days: c.days,
        source: "cloudflare".into(),
    }))
}

// The Plausible fetch flow does not write these cache payloads yet.

#[derive(Deserialize)]
struct TopFallingPageCache {
    url: String,
    pct_drop: f32,
}

pub fn plausible_top_falling_page(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<TopFallingPageCache> = cache.read("plausible", "top_falling_page")?;
    Ok(cached.map(|c| Enrichment::TopFallingPage {
        url: c.url,
        pct_drop: c.pct_drop,
        source: "plausible".into(),
    }))
}

#[derive(Deserialize)]
struct TopFallingFunnelCache {
    name: String,
    pct_drop: f32,
}

pub fn plausible_top_falling_funnel(
    _check_id: &str,
    cache: &EnrichmentCache,
) -> Result<Option<Enrichment>, String> {
    let cached: Option<TopFallingFunnelCache> = cache.read("plausible", "top_falling_funnel")?;
    Ok(cached.map(|c| Enrichment::TopFallingFunnel {
        name: c.name,
        pct_drop: c.pct_drop,
        source: "plausible".into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db_with_project;
    use rusqlite::params;

    fn seed_cache(
        db: &crate::db::Database,
        project_id: i64,
        integration: &str,
        signal_key: &str,
        payload_json: &str,
        age_ms: i64,
    ) {
        let refreshed_at = chrono::Utc::now().timestamp_millis() - age_ms;
        let integration_owned = integration.to_string();
        let signal_owned = signal_key.to_string();
        let payload_owned = payload_json.to_string();
        db.execute(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO integration_enrichment_cache
                   (project_id, integration, signal_key, payload_json, refreshed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project_id,
                    integration_owned,
                    signal_owned,
                    payload_owned,
                    refreshed_at
                ],
            )
            .unwrap();
        })
        .unwrap();
    }

    #[test]
    fn gsc_field_lcp_returns_enrichment_when_fresh() {
        let db = temp_db_with_project();
        let project_id: i64 = 1;
        seed_cache(
            &db,
            project_id,
            "gsc",
            "field_lcp",
            r#"{"p75_ms":2500,"url":"https://example.com/"}"#,
            0, // fresh
        );
        let cache = EnrichmentCache::load(&db, project_id).expect("load cache");
        let result = gsc_field_lcp("performance.lcp", &cache).expect("should not error");
        assert!(result.is_some(), "should return Some for fresh row");
        if let Some(Enrichment::FieldLcp {
            p75_ms,
            url,
            source,
        }) = result
        {
            assert_eq!(p75_ms, 2500);
            assert_eq!(url, "https://example.com/");
            assert_eq!(source, "gsc");
        } else {
            panic!("wrong enrichment variant");
        }
    }

    #[test]
    fn stale_row_returns_none() {
        let db = temp_db_with_project();
        let project_id: i64 = 1;
        // Age = 6 minutes (> 5-minute TTL)
        seed_cache(
            &db,
            project_id,
            "gsc",
            "field_lcp",
            r#"{"p75_ms":2500,"url":"https://example.com/"}"#,
            6 * 60 * 1000,
        );
        let cache = EnrichmentCache::load(&db, project_id).expect("load cache");
        let result = gsc_field_lcp("performance.lcp", &cache).expect("should not error");
        assert!(result.is_none(), "stale row should return None");
    }

    #[test]
    fn missing_row_returns_none() {
        let db = temp_db_with_project();
        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let result = gsc_field_lcp("performance.lcp", &cache).expect("should not error");
        assert!(result.is_none(), "missing row should return None");
    }

    #[test]
    fn malformed_fresh_payload_is_an_error_not_a_missing_enrichment() {
        let db = temp_db_with_project();
        seed_cache(&db, 1, "gsc", "field_lcp", r#"{"p75_ms":"fast"}"#, 0);
        let cache = EnrichmentCache::load(&db, 1).expect("load cache");

        let error = gsc_field_lcp("performance.lcp", &cache)
            .expect_err("malformed cached evidence must fail closed");

        assert!(error.contains("gsc/field_lcp enrichment payload"));
    }

    #[test]
    fn uptime_cert_expires_in_returns_enrichment_when_fresh() {
        let db = temp_db_with_project();
        let project_id: i64 = 1;
        seed_cache(
            &db,
            project_id,
            "uptimerobot",
            "cert_expires_in",
            r#"{"days":14}"#,
            0,
        );
        let cache = EnrichmentCache::load(&db, project_id).expect("load cache");
        let result = uptime_cert_expires_in("infrastructure.ssl-expiring", &cache)
            .expect("should not error");
        assert!(result.is_some());
        if let Some(Enrichment::CertExpiresIn { days, source }) = result {
            assert_eq!(days, 14);
            assert_eq!(source, "uptimerobot");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn cf_cache_hit_rate_returns_enrichment_when_fresh() {
        let db = temp_db_with_project();
        let project_id: i64 = 1;
        seed_cache(
            &db,
            project_id,
            "cloudflare",
            "cache_hit_rate",
            r#"{"value":0.82}"#,
            0,
        );
        let cache = EnrichmentCache::load(&db, project_id).expect("load cache");
        let result =
            cf_cache_hit_rate("performance.cache_headers", &cache).expect("should not error");
        assert!(result.is_some());
        if let Some(Enrichment::CacheHitRate { value, source }) = result {
            assert!((value - 0.82_f32).abs() < 0.001);
            assert_eq!(source, "cloudflare");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn plausible_top_falling_funnel_returns_enrichment_when_fresh() {
        let db = temp_db_with_project();
        let project_id: i64 = 1;
        seed_cache(
            &db,
            project_id,
            "plausible",
            "top_falling_funnel",
            r#"{"name":"Checkout","pct_drop":0.35}"#,
            0,
        );
        let cache = EnrichmentCache::load(&db, project_id).expect("load cache");
        let result = plausible_top_falling_funnel("analytics.conversion-drop", &cache)
            .expect("should not error");
        assert!(result.is_some());
        if let Some(Enrichment::TopFallingFunnel {
            name,
            pct_drop,
            source,
        }) = result
        {
            assert_eq!(name, "Checkout");
            assert!((pct_drop - 0.35_f32).abs() < 0.001);
            assert_eq!(source, "plausible");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn write_cache_payload_then_read_round_trips() {
        let db = temp_db_with_project();
        let project_id: i64 = 1;
        write_cache_payload(
            &db,
            project_id,
            "gsc",
            "field_cls",
            r#"{"value":0.05,"url":"https://example.com/about"}"#,
        )
        .expect("write should succeed");
        let cache = EnrichmentCache::load(&db, project_id).expect("load cache");
        let result = gsc_field_cls("performance.cls", &cache).expect("should not error");
        assert!(result.is_some(), "written payload should be readable back");
        if let Some(Enrichment::FieldCls { value, url, source }) = result {
            assert!((value - 0.05_f32).abs() < 0.001);
            assert_eq!(url, "https://example.com/about");
            assert_eq!(source, "gsc");
        } else {
            panic!("wrong variant");
        }
    }
}
