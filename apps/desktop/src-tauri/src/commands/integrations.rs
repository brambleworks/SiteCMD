use crate::api_cache;
use crate::db::Database;
use crate::integrations::{self, IntegrationConfig, IntegrationData, IntegrationType};
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::{confirm_sensitive_action, emit_event, run_blocking, sanitize_error};

/// Notify the renderer that project-wide integration badges must refresh.
/// `source: "integration"` selects the forced refresh path.
fn emit_integration_signals_changed(app: &AppHandle, project_id: i64) {
    emit_event(
        app,
        "project-signals-changed",
        serde_json::json!({
            "projectId": project_id,
            "url": null,
            "source": "integration",
        }),
    );
}

mod analytics;
#[cfg(test)]
mod tests;

pub(crate) use analytics::fetch_analytics_internal;
pub use analytics::*;

/// Build an IntegrationData response from a fetch result, reducing match-arm boilerplate.
fn integration_result(
    itype: IntegrationType,
    result: Result<impl serde::Serialize, String>,
) -> IntegrationData {
    let now = chrono::Utc::now().to_rfc3339();
    match result {
        Ok(data) => IntegrationData {
            integration_type: itype,
            data: serde_json::to_value(data).unwrap_or_default(),
            fetched_at: now,
            error: None,
        },
        Err(e) => IntegrationData {
            integration_type: itype,
            data: serde_json::Value::Null,
            fetched_at: now,
            error: Some(e),
        },
    }
}

fn parse_integration_type(integration_type: &str) -> Result<IntegrationType, String> {
    integration_type.parse().map_err(sanitize_error)
}

fn take_enabled_integration_config(
    configs: Vec<IntegrationConfig>,
    integration_type: &IntegrationType,
) -> Result<IntegrationConfig, String> {
    configs
        .into_iter()
        .find(|config| config.enabled && &config.integration_type == integration_type)
        .ok_or_else(|| "Integration not configured".to_string())
}

pub(crate) fn usable_api_key(config: &IntegrationConfig) -> Option<&str> {
    config
        .api_key
        .as_deref()
        .filter(|key| !key.trim().is_empty() && *key != crate::keyring::KEYRING_PLACEHOLDER)
}

/// Returns the configured API key, distinguishing an unreadable keychain entry
/// from a key that was never set so callers can give the correct recovery path.
pub(crate) fn require_api_key(config: &IntegrationConfig) -> Result<&str, String> {
    if let Some(key) = usable_api_key(config) {
        return Ok(key);
    }
    if config.api_key.as_deref() == Some(crate::keyring::KEYRING_PLACEHOLDER) {
        Err("Saved API key couldn't be read from your keychain. Reconnect the integration to re-store it.".to_string())
    } else {
        Err("No API key configured".to_string())
    }
}

fn load_enabled_integration_config(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    integration_type: &IntegrationType,
) -> Result<IntegrationConfig, String> {
    let configs = db.get_integrations(project_id).map_err(sanitize_error)?;
    let mut config = take_enabled_integration_config(configs, integration_type)?;
    crate::keyring::hydrate_integration_secrets(app, db, project_id, &mut config);
    Ok(config)
}

/// The project's GitHub token when one is configured and readable. Connected
/// CI setup uses it only to resolve the immutable id of a private repository;
/// public repositories need no credential.
pub(crate) fn github_access_token_for_project(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
) -> Result<Option<String>, String> {
    let configs = db.get_integrations(project_id).map_err(sanitize_error)?;
    let Some(mut config) = configs
        .into_iter()
        .find(|config| config.enabled && config.integration_type == IntegrationType::GitHub)
    else {
        return Ok(None);
    };
    crate::keyring::hydrate_integration_secrets(app, db, project_id, &mut config);
    if let Some(key) = usable_api_key(&config) {
        return Ok(Some(key.to_string()));
    }
    Ok(config
        .extra
        .as_ref()
        .and_then(|extra| extra.get("tokens"))
        .and_then(|tokens| tokens.get("access_token"))
        .and_then(serde_json::Value::as_str)
        .filter(|token| !token.trim().is_empty())
        .map(str::to_string))
}

/// Resolve and persist a refreshed Google access token when needed.
async fn resolve_google_token(
    app: &AppHandle,
    config: &IntegrationConfig,
    db: &Database,
    project_id: i64,
) -> Result<String, String> {
    integrations::google_oauth::resolve_valid_google_token_for_config(app, db, project_id, config)
        .await
        .map_err(sanitize_error)
}

#[tracing::instrument(skip(app, db, config), fields(project_id))]
pub(crate) fn persist_integration_config_securely(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    config: &IntegrationConfig,
) -> Result<(), String> {
    let config = integrations::validation::validate_and_normalize_config(config)?;
    let config_for_db = crate::keyring::store_integration_secrets(app, db, project_id, &config)
        .map_err(sanitize_error)?;
    api_cache::invalidate_project(project_id);
    db.save_integration(project_id, &config_for_db)
        .map_err(sanitize_error)?;
    db.invalidate_project_signal_snapshots(project_id, None)
        .map_err(sanitize_error)?;
    emit_integration_signals_changed(app, project_id);
    Ok(())
}

#[tracing::instrument(skip(app, db, url_filter), fields(project_id, integration_type = %integration_type))]
pub(crate) async fn fetch_integration_data_internal(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    integration_type: &str,
    url_filter: Option<&str>,
) -> Result<IntegrationData, String> {
    let requested_type = parse_integration_type(integration_type)?;
    let config = load_enabled_integration_config(app, db, project_id, &requested_type)?;
    let site_id = config.site_id.as_deref().unwrap_or("");
    let project_environment_urls = db.list_project_envs(project_id).unwrap_or_default();
    let public_environment_url =
        analytics::choose_analytics_integration_url(url_filter, &project_environment_urls);
    let effective_url_filter = public_environment_url.as_deref().or(url_filter);

    match requested_type {
        IntegrationType::Plausible => {
            let api_key = require_api_key(&config)?;
            let candidates = analytics::plausible_site_candidates(
                config.site_id.as_deref(),
                effective_url_filter,
            );
            let mut last_error = None;
            for plausible_site_id in candidates {
                match integrations::plausible::fetch_stats(api_key, &plausible_site_id).await {
                    Ok(data) => {
                        return Ok(integration_result(IntegrationType::Plausible, Ok(data)));
                    }
                    Err(error) => {
                        tracing::warn!(
                            "Plausible live-data fetch failed for site_id '{}': {}",
                            plausible_site_id,
                            error
                        );
                        last_error = Some(error);
                    }
                }
            }
            Ok(integration_result(
                IntegrationType::Plausible,
                Err::<integrations::plausible::PlausibleData, String>(
                    last_error.unwrap_or_else(|| "No Plausible site ID configured".to_string()),
                ),
            ))
        }
        IntegrationType::Cloudflare => {
            let api_key = require_api_key(&config)?;
            let zone_ref = analytics::cloudflare_zone_candidates(
                config.site_id.as_deref(),
                effective_url_filter,
            )
            .into_iter()
            .next()
            .ok_or_else(|| {
                "No Cloudflare zone configured. Paste the Zone ID/domain, or add a public project environment URL.".to_string()
            })?;
            Ok(integration_result(
                IntegrationType::Cloudflare,
                integrations::cloudflare::fetch_stats(api_key, &zone_ref).await,
            ))
        }
        IntegrationType::UptimeRobot => {
            let api_key = require_api_key(&config)?;
            Ok(integration_result(
                IntegrationType::UptimeRobot,
                integrations::uptimerobot::fetch_stats(api_key, url_filter).await,
            ))
        }
        IntegrationType::BingWebmaster => {
            let api_key = require_api_key(&config)?;
            let site_url = url_filter.unwrap_or(site_id);
            Ok(integration_result(
                IntegrationType::BingWebmaster,
                integrations::bing::fetch_search_stats(api_key, site_url).await,
            ))
        }
        _ => {
            let access_token = resolve_google_token(app, &config, db, project_id).await?;
            match config.integration_type {
                IntegrationType::GoogleAnalytics => {
                    let property_id = config
                        .site_id
                        .as_deref()
                        .ok_or("No GA4 property ID configured")?;
                    Ok(integration_result(
                        IntegrationType::GoogleAnalytics,
                        integrations::google_analytics::fetch_analytics(
                            &access_token,
                            property_id,
                            30,
                        )
                        .await,
                    ))
                }
                IntegrationType::GoogleSearchConsole => {
                    let site_url_gsc = config
                        .site_id
                        .as_deref()
                        .ok_or("No Search Console site URL configured")?;
                    Ok(integration_result(
                        IntegrationType::GoogleSearchConsole,
                        integrations::search_console::fetch_analytics(
                            &access_token,
                            site_url_gsc,
                            28,
                        )
                        .await,
                    ))
                }
                _ => Err("This integration doesn't support direct data fetch".into()),
            }
        }
    }
}

#[tracing::instrument(skip(app, db, url_filter), fields(project_id, integration_type = %integration_type, cache_scope = %cache_scope))]
pub(crate) async fn fetch_cached_integration_data_internal(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    integration_type: &str,
    url_filter: Option<&str>,
    cache_scope: &str,
) -> Result<IntegrationData, String> {
    let cache_key = api_cache::cache_key(project_id, integration_type, cache_scope);
    if let Some(cached) = api_cache::get(&cache_key) {
        if let Ok(data) = serde_json::from_value::<IntegrationData>(cached) {
            return Ok(data);
        }
    }

    let data =
        fetch_integration_data_internal(app, db, project_id, integration_type, url_filter).await?;
    if data.error.is_some() {
        return Ok(data);
    }

    let serialized = serde_json::to_value(&data)
        .map_err(|e| sanitize_error(format!("Failed to cache integration data: {}", e)))?;
    api_cache::set(&cache_key, serialized);
    Ok(data)
}

#[tracing::instrument(skip(app, db), fields(project_id))]
pub(crate) async fn fetch_github_data_internal(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
) -> Result<integrations::github::GitHubData, String> {
    let gh_config = load_enabled_integration_config(app, db, project_id, &IntegrationType::GitHub)?;

    let token = if let Some(key) = usable_api_key(&gh_config) {
        key.to_string()
    } else if let Some(extra) = &gh_config.extra {
        extra["tokens"]["access_token"]
            .as_str()
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };
    let repo = gh_config
        .site_id
        .as_deref()
        .ok_or("GitHub repo not configured. Set the repo as 'owner/repo' in site_id.")?;

    let cache_key = api_cache::cache_key(project_id, "github", "latest");
    if let Some(cached) = api_cache::get(&cache_key) {
        return serde_json::from_value(cached)
            .map_err(|e| format!("Failed to deserialize cached GitHub data: {}", e));
    }

    let data = integrations::github::fetch_github_data(&token, repo).await?;
    let val = serde_json::to_value(&data).map_err(|e| format!("Serialize error: {}", e))?;
    api_cache::set(&cache_key, val);
    Ok(data)
}

/// Save an integration config (Plausible, Cloudflare, etc). Stores API keys in keychain when available.
#[tracing::instrument(skip(app, db, config), fields(project_id))]
pub async fn save_integration(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    config: IntegrationConfig,
) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || persist_integration_config_securely(&app, &db, project_id, &config))
        .await?
}

/// Get all integration configs for a project, redacted for safe renderer use.
#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id))]
pub async fn get_integrations(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Vec<IntegrationConfig>, String> {
    let db = (*db).clone();
    run_blocking(move || -> Result<Vec<IntegrationConfig>, String> {
        let mut configs = db.get_integrations(project_id).map_err(sanitize_error)?;
        for config in &mut configs {
            crate::keyring::redact_integration_secrets(config);
        }
        Ok(configs)
    })
    .await?
}

/// Delete an integration and clean up its keychain entries.
#[tracing::instrument(skip(app, db), fields(project_id, integration_type = %integration_type))]
pub async fn delete_integration(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    integration_type: String,
) -> Result<(), String> {
    let display_name = integration_type
        .parse::<IntegrationType>()
        .map(|itype| itype.display_name().to_string())
        .unwrap_or_else(|_| integration_type.clone());
    confirm_sensitive_action(
        app.clone(),
        "Delete this integration?",
        format!(
            "This removes the saved {display_name} integration and deletes its stored credentials."
        ),
        "Delete Integration",
    )
    .await?;

    api_cache::invalidate_project(project_id);

    {
        let db = (*db).clone();
        let app = app.clone();
        run_blocking(move || -> Result<(), String> {
            if let Err(e) = crate::keyring::delete_api_key(&app, &db, project_id, &integration_type)
            {
                tracing::warn!("Failed to delete API key from keyring: {}", e);
            }
            if let Err(e) = crate::keyring::delete_tokens(&app, &db, project_id, &integration_type)
            {
                tracing::warn!("Failed to delete tokens from keyring: {}", e);
            }

            db.delete_integration(project_id, &integration_type)
                .map_err(sanitize_error)?;
            db.invalidate_project_signal_snapshots(project_id, None)
                .map_err(sanitize_error)
        })
        .await??;
    }
    emit_integration_signals_changed(&app, project_id);
    Ok(())
}

/// Fetch data from a single integration (Plausible, Cloudflare, UptimeRobot, Bing, Google).
/// Resolves keychain placeholders and refreshes OAuth tokens as needed.
#[tracing::instrument(skip(app, db, url_filter), fields(project_id, integration_type = %integration_type))]
pub async fn fetch_integration_data(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    integration_type: String,
    url_filter: Option<String>,
) -> Result<IntegrationData, String> {
    fetch_integration_data_internal(
        &app,
        &db,
        project_id,
        &integration_type,
        url_filter.as_deref(),
    )
    .await
}

/// Fetch the latest GitHub release for a project's linked repo.
/// Returns None when the repo has no releases yet.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn github_latest_release(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Option<integrations::github::GithubReleaseSummary>, String> {
    let gh_config =
        load_enabled_integration_config(&app, &db, project_id, &IntegrationType::GitHub)?;

    let token = if let Some(key) = usable_api_key(&gh_config) {
        key.to_string()
    } else if let Some(extra) = &gh_config.extra {
        extra["tokens"]["access_token"]
            .as_str()
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };
    let repo = gh_config
        .site_id
        .as_deref()
        .ok_or("GitHub repo not configured. Set the repo as 'owner/repo' in site_id.")?;

    integrations::github::fetch_latest_release(repo, &token).await
}

/// Fetch GitHub CI/deploy data for a project.
/// Uses the GitHub integration config (PAT + repo) to fetch workflow runs,
/// deployments, and open PRs.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn fetch_github_data(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<serde_json::Value, String> {
    let data = fetch_github_data_internal(&app, &db, project_id).await?;
    serde_json::to_value(&data).map_err(|e| format!("Serialize error: {}", e))
}
