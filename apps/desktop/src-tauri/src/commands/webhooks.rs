use crate::db::Database;
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::{confirm_sensitive_action, run_blocking, sanitize_error};

/// Get all webhook configurations for a project.
#[tauri::command]
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn get_webhook_configs(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Vec<crate::db::WebhookConfig>, String> {
    let db = (*db).clone();
    run_blocking(move || -> Result<Vec<crate::db::WebhookConfig>, String> {
        if let Err(e) = crate::keyring::migrate_webhook_secrets(&app, &db) {
            tracing::warn!("Webhook secret migration skipped: {}", e);
        }
        let mut configs = db.get_webhook_configs(project_id).map_err(sanitize_error)?;
        for config in &mut configs {
            config.secret = None;
        }
        Ok(configs)
    })
    .await?
}

/// Save (create or update) a webhook configuration.
#[tracing::instrument(skip(app, db, secret, url), fields(project_id, events = %events, has_secret = secret.as_ref().is_some_and(|s| !s.is_empty()), enabled))]
pub async fn save_webhook_config(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: String,
    events: String,
    secret: Option<String>,
    enabled: bool,
) -> Result<i64, String> {
    let db = (*db).clone();
    let host = url::Url::parse(&url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string));
    let scheme = url::Url::parse(&url).ok().map(|u| u.scheme().to_string());
    let audit_detail = serde_json::json!({
        "project_id": project_id,
        "host": host,
        "scheme": scheme,
        "has_secret": secret.as_ref().is_some_and(|s| !s.is_empty()),
        "enabled": enabled,
    });

    if let Err(e) = super::validate_external_callback_url_async(&url).await {
        crate::audit_log::record("webhook.save", audit_detail, "fail");
        return Err(e);
    }
    run_blocking(move || {
        let new_secret = secret.as_deref().filter(|s| !s.is_empty());
        let previous_secret = if new_secret.is_some() {
            db.get_webhook_configs(project_id)
                .ok()
                .and_then(|configs| configs.into_iter().find(|config| config.url == url))
                .and_then(|config| {
                    crate::keyring::get_webhook_secret(
                        &app,
                        &db,
                        config.project_id,
                        config.id,
                        &config.url,
                    )
                    .ok()
                    .flatten()
                    .or(config.secret)
                })
        } else {
            None
        };

        if let Some(secret) = new_secret {
            if let Err(e) =
                crate::keyring::store_webhook_secret(&app, &db, project_id, &url, secret)
                    .map_err(sanitize_error)
            {
                crate::audit_log::record("webhook.save", audit_detail, "fail");
                return Err(e);
            }
        }

        match db.save_webhook_config(project_id, &url, &events, None, enabled) {
            Ok(id) => {
                crate::audit_log::record("webhook.save", audit_detail, "ok");
                Ok(id)
            }
            Err(error) => {
                if new_secret.is_some() {
                    if let Some(previous_secret) = previous_secret.as_deref() {
                        let _ = crate::keyring::store_webhook_secret(
                            &app,
                            &db,
                            project_id,
                            &url,
                            previous_secret,
                        );
                    } else {
                        let _ = crate::keyring::delete_webhook_secret_for_url(
                            &app, &db, project_id, &url,
                        );
                    }
                }
                crate::audit_log::record("webhook.save", audit_detail, "fail");
                Err(sanitize_error(error))
            }
        }
    })
    .await?
}

/// Delete a webhook configuration by ID.
#[tracing::instrument(skip(app, db), fields(id))]
pub async fn delete_webhook_config(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    confirm_sensitive_action(
        app.clone(),
        "Delete this webhook?",
        "This removes the saved webhook destination and its stored signing secret.".to_string(),
        "Delete Webhook",
    )
    .await?;

    let db = (*db).clone();
    run_blocking(move || {
        if let Some(config) = db.get_webhook_config(id).map_err(sanitize_error)? {
            if let Err(e) =
                crate::keyring::delete_webhook_secret(&app, &db, config.project_id, id, &config.url)
            {
                tracing::warn!("Failed to delete webhook secret from keyring: {}", e);
            }
        }
        db.delete_webhook_config(id).map_err(sanitize_error)
    })
    .await?
}

/// Test a webhook by sending a sample payload.
#[tracing::instrument(skip(app, db), fields(id))]
pub async fn test_webhook(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<String, String> {
    let db = (*db).clone();
    let config = {
        let db = db.clone();
        run_blocking(move || -> Result<crate::db::WebhookConfig, String> {
            db.get_webhook_config(id)
                .map_err(sanitize_error)?
                .ok_or_else(|| "Webhook config not found".to_string())
        })
        .await??
    };
    super::validate_external_callback_url_async(&config.url).await?;
    let secret = {
        let db = db.clone();
        let config = config.clone();
        run_blocking(move || {
            crate::keyring::get_webhook_secret(&app, &db, config.project_id, config.id, &config.url)
                .map_err(sanitize_error)
                .map(|secret| secret.or(config.secret))
        })
        .await??
    };
    let payload = serde_json::json!({
        "event": "test",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "data": {
            "message": "This is a test webhook from SiteCMD",
            "url": "https://example.com",
            "score": 85
        }
    });

    crate::webhooks::send_webhook(&config.url, secret.as_deref(), &payload).await?;
    Ok("Webhook delivered successfully".to_string())
}
