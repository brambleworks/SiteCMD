use crate::api_cache;
use crate::db::Database;
use crate::integrations::{self, IntegrationConfig, IntegrationType};
use std::net::IpAddr;
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::super::{period_to_days, sanitize_error};
use super::{resolve_google_token, usable_api_key};

pub(super) fn is_analytics_integration(integration_type: &IntegrationType) -> bool {
    matches!(
        integration_type,
        IntegrationType::Plausible
            | IntegrationType::Cloudflare
            | IntegrationType::UptimeRobot
            | IntegrationType::GoogleAnalytics
            | IntegrationType::GoogleSearchConsole
            | IntegrationType::BingWebmaster
    )
}

pub(super) fn take_enabled_analytics_configs(
    configs: Vec<IntegrationConfig>,
) -> Vec<IntegrationConfig> {
    configs
        .into_iter()
        .filter(|config| config.enabled && is_analytics_integration(&config.integration_type))
        .collect()
}

/// Planning result for a Google analytics fetch.
///
/// `Skip` is reserved for unconfigured integrations. Configured integrations
/// must use cached data or attempt a fetch so missing OAuth state is reported.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum GoogleFetchPlan {
    Skip,
    ServeCached,
    Attempt,
}

pub(super) fn plan_google_fetch(configured: bool, cache_hit: bool) -> GoogleFetchPlan {
    match (configured, cache_hit) {
        (false, _) => GoogleFetchPlan::Skip,
        (true, true) => GoogleFetchPlan::ServeCached,
        (true, false) => GoogleFetchPlan::Attempt,
    }
}

/// Resolve keyring placeholders for a set of integration configs.
/// Shared helper used by fetch_integration_data and fetch_analytics.
#[tracing::instrument(skip(app, db, configs), fields(project_id))]
pub(crate) fn resolve_keyring(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    configs: &mut [IntegrationConfig],
) {
    for config in configs.iter_mut() {
        crate::keyring::hydrate_integration_secrets(app, db, project_id, config);
    }
}

pub(super) fn normalize_plausible_site_id(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = url::Url::parse(trimmed) {
        return parsed
            .host_str()
            .map(|host| host.trim_end_matches('.').to_ascii_lowercase());
    }

    let with_scheme = format!("https://{}", trimmed.trim_start_matches("//"));
    if let Ok(parsed) = url::Url::parse(&with_scheme) {
        if let Some(host) = parsed.host_str() {
            return Some(host.trim_end_matches('.').to_ascii_lowercase());
        }
    }

    Some(trimmed.trim_end_matches('.').to_ascii_lowercase())
}

fn is_local_plausible_site_id(site_id: &str) -> bool {
    site_id == "localhost"
        || site_id.ends_with(".localhost")
        || site_id
            .parse::<IpAddr>()
            .map(|addr| addr.is_loopback())
            .unwrap_or(false)
}

fn push_unique_site_id(candidates: &mut Vec<String>, site_id: Option<String>) {
    let Some(site_id) = site_id else {
        return;
    };
    if !candidates.iter().any(|candidate| candidate == &site_id) {
        candidates.push(site_id);
    }
}

fn plausible_site_ids_are_related(left: &str, right: &str) -> bool {
    let left = left.strip_prefix("www.").unwrap_or(left);
    let right = right.strip_prefix("www.").unwrap_or(right);
    left == right
        || left
            .strip_suffix(right)
            .map(|prefix| prefix.ends_with('.'))
            .unwrap_or(false)
        || right
            .strip_suffix(left)
            .map(|prefix| prefix.ends_with('.'))
            .unwrap_or(false)
}

pub(super) fn plausible_site_candidates(
    configured_site_id: Option<&str>,
    environment_url: Option<&str>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let environment_site_id = environment_url
        .and_then(normalize_plausible_site_id)
        .filter(|site_id| !is_local_plausible_site_id(site_id));
    let configured_site_id = configured_site_id
        .and_then(normalize_plausible_site_id)
        .filter(|site_id| !is_local_plausible_site_id(site_id));
    match (environment_site_id, configured_site_id) {
        (Some(environment), Some(configured))
            if plausible_site_ids_are_related(&environment, &configured) =>
        {
            push_unique_site_id(&mut candidates, Some(configured));
            push_unique_site_id(&mut candidates, Some(environment));
        }
        (Some(environment), Some(configured)) => {
            push_unique_site_id(&mut candidates, Some(environment));
            push_unique_site_id(&mut candidates, Some(configured));
        }
        (Some(environment), None) => push_unique_site_id(&mut candidates, Some(environment)),
        (None, Some(configured)) => push_unique_site_id(&mut candidates, Some(configured)),
        (None, None) => {}
    }
    candidates
}

fn normalize_public_integration_url(environment_url: &str) -> Option<String> {
    url::Url::parse(environment_url).ok().and_then(|parsed| {
        if crate::core::localhost::is_localhost(&parsed) {
            return None;
        }
        parsed.host_str()?;
        Some(parsed.to_string())
    })
}

pub(super) fn choose_analytics_integration_url(
    environment_url: Option<&str>,
    project_environment_urls: &[String],
) -> Option<String> {
    environment_url
        .and_then(normalize_public_integration_url)
        .or_else(|| {
            project_environment_urls
                .iter()
                .find_map(|url| normalize_public_integration_url(url))
        })
}

fn plausible_cache_key(project_id: i64, period: &str, site_id: &str) -> String {
    api_cache::cache_key(project_id, "plausible", &format!("{period}:{site_id}"))
}

fn public_site_ref_from_url(value: Option<&str>) -> Option<String> {
    value
        .and_then(normalize_plausible_site_id)
        .filter(|site_ref| !is_local_plausible_site_id(site_ref))
}

pub(super) fn cloudflare_zone_candidates(
    configured_zone_ref: Option<&str>,
    environment_url: Option<&str>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(zone_ref) = configured_zone_ref {
        let trimmed = zone_ref.trim();
        if crate::integrations::cloudflare::looks_like_cloudflare_zone_id(trimmed) {
            candidates.push(trimmed.to_string());
            return candidates;
        }
        push_unique_site_id(&mut candidates, public_site_ref_from_url(Some(trimmed)));
    }
    push_unique_site_id(&mut candidates, public_site_ref_from_url(environment_url));
    candidates
}

/// Invalidate the in-memory analytics cache for a project (forces fresh API calls).
#[tracing::instrument(skip(db), fields(project_id))]
pub async fn invalidate_analytics_cache(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<(), String> {
    let db = (*db).clone();
    crate::commands::run_blocking(move || -> Result<(), String> {
        api_cache::invalidate_project(project_id);
        db.invalidate_project_signal_snapshots(project_id, None)
            .map_err(sanitize_error)?;
        Ok(())
    })
    .await?
}

#[tracing::instrument(skip(app, db), fields(project_id, period = %period))]
pub(crate) async fn fetch_analytics_internal(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    period: &str,
    environment_url: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut configs =
        take_enabled_analytics_configs(db.get_integrations(project_id).map_err(sanitize_error)?);
    resolve_keyring(app, db, project_id, &mut configs);

    let mut result = serde_json::Map::new();
    let project_environment_urls = db.list_project_envs(project_id).unwrap_or_default();
    let integration_environment_url =
        choose_analytics_integration_url(environment_url, &project_environment_urls);

    let mut plausible_plan_error: Option<String> = None;
    let plausible_fetch: Option<(String, Vec<String>)> = configs
        .iter()
        .find(|c| c.integration_type == IntegrationType::Plausible)
        .and_then(|config| {
            let Some(api_key) = usable_api_key(config) else {
                plausible_plan_error = Some("No Plausible API key configured".to_string());
                return None;
            };
            let candidates = plausible_site_candidates(
                config.site_id.as_deref(),
                integration_environment_url.as_deref(),
            );
            if candidates.is_empty() {
                plausible_plan_error = Some(
                    "No Plausible site configured. Set the Plausible site domain, or add a public project environment URL.".to_string(),
                );
                return None;
            }

            for site_id in &candidates {
                let ck = plausible_cache_key(project_id, period, site_id);
                if let Some(cached) = api_cache::get(&ck) {
                    result.insert("plausible".into(), cached);
                    return None;
                }
            }

            Some((api_key.to_owned(), candidates))
        });

    let mut cloudflare_plan_error: Option<String> = None;
    let cloudflare_fetch: Option<(String, String, String)> = configs
        .iter()
        .find(|c| c.integration_type == IntegrationType::Cloudflare)
        .and_then(|config| {
            let Some(api_key) = usable_api_key(config) else {
                cloudflare_plan_error = Some("No Cloudflare API token configured".to_string());
                return None;
            };
            let candidates = cloudflare_zone_candidates(
                config.site_id.as_deref(),
                integration_environment_url.as_deref(),
            );
            let Some(zone_ref) = candidates.into_iter().next() else {
                cloudflare_plan_error = Some(
                    "No Cloudflare zone configured. Paste the Zone ID/domain, or add a public project environment URL.".to_string(),
                );
                return None;
            };
            let ck = api_cache::cache_key(project_id, "cloudflare", &format!("{period}:{zone_ref}"));
            if let Some(cached) = api_cache::get(&ck) {
                result.insert("cloudflare".into(), cached);
                return None;
            }
            Some((ck, api_key.to_owned(), zone_ref))
        });

    let uptimerobot_fetch: Option<(String, String, Option<String>)> = configs
        .iter()
        .find(|c| c.integration_type == IntegrationType::UptimeRobot)
        .and_then(|config| {
            let ck = api_cache::cache_key(project_id, "uptimerobot", period);
            if let Some(cached) = api_cache::get(&ck) {
                result.insert("uptimerobot".into(), cached);
                None
            } else {
                usable_api_key(config)
                    .map(|k| (ck, k.to_owned(), integration_environment_url.clone()))
            }
        });

    // Attempt configured Google integrations even without an OAuth link so token
    // resolution can distinguish expired sign-in from an unconfigured service.
    let ga4_fetch: Option<(String, IntegrationConfig)> = {
        let config = configs
            .iter()
            .find(|c| c.integration_type == IntegrationType::GoogleAnalytics);
        let ck = api_cache::cache_key(project_id, "ga4", period);
        let cached = api_cache::get(&ck);
        match plan_google_fetch(config.is_some(), cached.is_some()) {
            GoogleFetchPlan::ServeCached => {
                if let Some(cached) = cached {
                    result.insert("google_analytics".into(), cached);
                }
                None
            }
            GoogleFetchPlan::Attempt => config.map(|config| (ck, config.clone())),
            GoogleFetchPlan::Skip => None,
        }
    };

    let gsc_fetch: Option<(String, IntegrationConfig)> = {
        let config = configs
            .iter()
            .find(|c| c.integration_type == IntegrationType::GoogleSearchConsole);
        let ck = api_cache::cache_key(project_id, "gsc", period);
        let cached = api_cache::get(&ck);
        match plan_google_fetch(config.is_some(), cached.is_some()) {
            GoogleFetchPlan::ServeCached => {
                if let Some(cached) = cached {
                    result.insert("search_console".into(), cached);
                }
                None
            }
            GoogleFetchPlan::Attempt => config.map(|config| (ck, config.clone())),
            GoogleFetchPlan::Skip => None,
        }
    };

    let bing_fetch: Option<(String, String, String)> = configs
        .iter()
        .find(|c| c.integration_type == IntegrationType::BingWebmaster)
        .and_then(|config| {
            let ck = api_cache::cache_key(project_id, "bing", period);
            if let Some(cached) = api_cache::get(&ck) {
                result.insert("bing".into(), cached);
                None
            } else {
                usable_api_key(config).map(|k| {
                    let site_url = config.site_id.as_deref().unwrap_or("").to_owned();
                    (ck, k.to_owned(), site_url)
                })
            }
        });

    let period_clone = period.to_string();

    let plausible_fut = async {
        if let Some(error) = plausible_plan_error {
            Some(("plausible", Err(error)))
        } else if let Some((api_key, site_ids)) = plausible_fetch {
            let mut last_error = None;
            for site_id in site_ids {
                match integrations::plausible::fetch_analytics(&api_key, &site_id, &period_clone)
                    .await
                {
                    Ok(data) => {
                        let val = serde_json::to_value(&data).unwrap_or_default();
                        let ck = plausible_cache_key(project_id, &period_clone, &site_id);
                        api_cache::set(&ck, val.clone());
                        return Some(("plausible", Ok(val)));
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Plausible analytics fetch failed for site_id '{}': {}",
                            site_id,
                            e
                        );
                        last_error = Some(e);
                    }
                }
            }
            Some((
                "plausible",
                Err(last_error.unwrap_or_else(|| "No Plausible site ID configured".to_string())),
            ))
        } else {
            None
        }
    };

    let period_cf = period.to_string();
    let cloudflare_fut = async {
        if let Some(error) = cloudflare_plan_error {
            Some(("cloudflare", Err(error)))
        } else if let Some((ck, api_key, zone_id)) = cloudflare_fetch {
            match integrations::cloudflare::fetch_stats_with_period(&api_key, &zone_id, &period_cf)
                .await
            {
                Ok(data) => {
                    let val = serde_json::to_value(&data).unwrap_or_default();
                    api_cache::set(&ck, val.clone());
                    Some(("cloudflare", Ok(val)))
                }
                Err(e) => Some(("cloudflare", Err(e))),
            }
        } else {
            None
        }
    };

    let uptimerobot_fut = async {
        if let Some((ck, api_key, url_filter)) = uptimerobot_fetch {
            match integrations::uptimerobot::fetch_stats(&api_key, url_filter.as_deref()).await {
                Ok(data) => {
                    let val = serde_json::to_value(&data).unwrap_or_default();
                    api_cache::set(&ck, val.clone());
                    Some(("uptimerobot", Ok(val)))
                }
                Err(e) => Some(("uptimerobot", Err(e))),
            }
        } else {
            None
        }
    };

    let period_ga = period.to_string();
    let ga4_fut = async {
        // Same contract as Search Console: a configured integration always reports
        // data or an error, so an expired Google sign-in surfaces as "reconnect"
        // rather than silently looking like it was never connected.
        let (ck, config) = ga4_fetch?;
        let access_token = match resolve_google_token(app, &config, db, project_id).await {
            Ok(token) => token,
            Err(e) => {
                tracing::warn!("Google Analytics token resolve failed: {}", e);
                return Some((
                    "google_analytics",
                    Err(
                        "Google sign-in expired. Reconnect Analytics to refresh the data."
                            .to_string(),
                    ),
                ));
            }
        };
        let Some(property_id) = config.site_id.as_deref() else {
            return Some((
                "google_analytics",
                Err("No Google Analytics property ID configured.".to_string()),
            ));
        };
        let days = period_to_days(&period_ga);
        match integrations::google_analytics::fetch_analytics(&access_token, property_id, days)
            .await
        {
            Ok(data) => {
                let val = serde_json::to_value(&data).unwrap_or_default();
                api_cache::set(&ck, val.clone());
                Some(("google_analytics", Ok(val)))
            }
            Err(e) => Some(("google_analytics", Err(e))),
        }
    };

    let period_gsc = period.to_string();
    let gsc_fut = async {
        // Preserve configured-but-expired as an error instead of omitting the source.
        let (ck, config) = gsc_fetch?;
        let access_token = match resolve_google_token(app, &config, db, project_id).await {
            Ok(token) => token,
            Err(e) => {
                tracing::warn!("Search Console token resolve failed: {}", e);
                return Some((
                    "search_console",
                    Err(
                        "Google sign-in expired. Reconnect Search Console to refresh the data."
                            .to_string(),
                    ),
                ));
            }
        };
        let Some(site_url_gsc) = config.site_id.as_deref() else {
            return Some((
                "search_console",
                Err("No Search Console site URL configured.".to_string()),
            ));
        };
        let days = period_to_days(&period_gsc);
        match integrations::search_console::fetch_analytics(&access_token, site_url_gsc, days).await
        {
            Ok(data) => {
                let val = serde_json::to_value(&data).unwrap_or_default();
                api_cache::set(&ck, val.clone());
                Some(("search_console", Ok(val)))
            }
            Err(e) => Some(("search_console", Err(e))),
        }
    };

    let bing_fut = async {
        if let Some((ck, api_key, site_url)) = bing_fetch {
            match integrations::bing::fetch_search_stats(&api_key, &site_url).await {
                Ok(data) => {
                    let val = serde_json::to_value(&data).unwrap_or_default();
                    api_cache::set(&ck, val.clone());
                    Some(("bing", Ok(val)))
                }
                Err(e) => Some(("bing", Err(e))),
            }
        } else {
            None
        }
    };

    let (r_plausible, r_cloudflare, r_uptimerobot, r_ga4, r_gsc, r_bing) = tokio::join!(
        plausible_fut,
        cloudflare_fut,
        uptimerobot_fut,
        ga4_fut,
        gsc_fut,
        bing_fut
    );

    for (name, outcome) in [
        r_plausible,
        r_cloudflare,
        r_uptimerobot,
        r_ga4,
        r_gsc,
        r_bing,
    ]
    .into_iter()
    .flatten()
    {
        match outcome {
            Ok(val) => {
                result.insert(name.into(), val);
            }
            Err(e) => {
                result.insert(format!("{}_error", name), serde_json::Value::String(e));
            }
        }
    }
    Ok(serde_json::Value::Object(result))
}

/// Fetch analytics from all configured integrations for the Analytics page.
/// Uses an in-memory cache per (project, integration, period) to avoid redundant API calls.
/// Returns a JSON object keyed by integration name.
#[tracing::instrument(skip(app, db), fields(project_id, period = %period))]
pub async fn fetch_analytics(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    period: String,
    site_url: Option<String>,
) -> Result<serde_json::Value, String> {
    fetch_analytics_internal(&app, &db, project_id, &period, site_url.as_deref()).await
}
