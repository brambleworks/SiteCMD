use crate::core::project;
use crate::db::{Database, ProjectRecord};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[cfg(test)]
use super::project_signal_state::{load_relevant_code_scan, take_monitored_integrations};
#[cfg(test)]
use super::project_work_items::build_update_work_items;
use super::validate_url_async;
use super::{confirm_sensitive_action, run_blocking, sanitize_error, SensitiveActionTone};

fn normalize_environment_for_url(url: &str, environment: &str) -> String {
    crate::core::localhost::resolve_environment_name(url, Some(environment)).to_string()
}

/// Health check endpoint. Returns "pong" to confirm the backend is alive.
#[tauri::command]
#[tracing::instrument]
pub async fn ping() -> String {
    "pong".to_string()
}

/// Detect project info from a folder path
#[tracing::instrument(skip(path), fields(path_len = path.len()))]
pub async fn detect_project_urls(path: String) -> Result<project::ProjectInfo, String> {
    let dir = crate::project_paths::canonicalize_project_dir(&path)?;
    Ok(project::detect_project(&dir))
}

/// Add a project and all its detected environments
#[tauri::command]
#[tracing::instrument(skip(db, urls, path), fields(name = %name, has_project_path = !path.trim().is_empty(), framework = ?framework))]
pub async fn add_project(
    db: State<'_, Arc<Database>>,
    name: String,
    path: String,
    framework: Option<String>,
    urls: Vec<project::DetectedUrl>,
) -> Result<i64, String> {
    // Empty URLs are valid for code-only projects; supplied URLs must be fetchable.
    for detected in &urls {
        validate_url_async(&detected.url).await?;
    }
    let db = (*db).clone();
    run_blocking(move || -> Result<i64, String> {
        let path = crate::project_paths::canonicalize_project_dir(&path)?
            .to_string_lossy()
            .to_string();
        let project_id = db
            .upsert_project(&name, &path, framework.as_deref())
            .map_err(sanitize_error)?;

        for detected in &urls {
            let environment = normalize_environment_for_url(&detected.url, &detected.environment);
            let label = format!("{} ({})", name, environment);
            db.add_environment(
                project_id,
                &detected.url,
                &label,
                &environment,
                &detected.source,
            )
            .map_err(sanitize_error)?;
        }

        tracing::info!("Added project '{}' with {} environments", name, urls.len());
        Ok(project_id)
    })
    .await?
}

/// Add a project by URL only (no local folder)
#[tauri::command]
#[tracing::instrument(skip(db, url), fields(name = %name))]
pub async fn add_project_by_url(
    db: State<'_, Arc<Database>>,
    name: String,
    url: String,
) -> Result<i64, String> {
    validate_url_async(&url).await?;
    let db = (*db).clone();
    run_blocking(move || -> Result<i64, String> {
        let project_id = db.upsert_project(&name, "", None).map_err(sanitize_error)?;
        let environment = crate::core::localhost::resolve_environment_name(&url, None);
        let label = format!("{} ({})", name, environment);
        db.add_environment(project_id, &url, &label, environment, "manual")
            .map_err(sanitize_error)?;
        tracing::info!("Added URL-only project '{}' → {}", name, url);
        Ok(project_id)
    })
    .await?
}

/// Rename a project
#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, name = %name))]
pub async fn rename_project(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    name: String,
) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || db.rename_project(project_id, &name))
        .await?
        .map_err(sanitize_error)
}

/// Update a project's local folder path and auto-detect the framework from it.
#[tracing::instrument(skip(db, path), fields(project_id, path_len = path.len()))]
pub async fn update_project_path(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    path: String,
) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || -> Result<(), String> {
        let path = if path.trim().is_empty() {
            String::new()
        } else {
            crate::project_paths::canonicalize_project_dir(&path)?
                .to_string_lossy()
                .to_string()
        };
        let framework = if !path.is_empty() {
            let p = std::path::Path::new(&path);
            if p.is_dir() {
                let info = crate::core::project::detect_project(p);
                info.framework
            } else {
                None
            }
        } else {
            None
        };
        db.update_project_path(project_id, &path, framework.as_deref())
            .map_err(sanitize_error)?;
        db.invalidate_project_signal_snapshots(project_id, None)
            .map_err(sanitize_error)
    })
    .await?
}

/// Get all projects with their environments
#[tauri::command]
#[tracing::instrument(skip(db))]
pub async fn get_projects(db: State<'_, Arc<Database>>) -> Result<Vec<ProjectRecord>, String> {
    let db = (*db).clone();
    run_blocking(move || db.get_projects())
        .await?
        .map_err(sanitize_error)
}

/// Delete a project and all associated data
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn delete_project(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<(), String> {
    let subject = {
        let db = (*db).clone();
        match run_blocking(move || db.get_project_name(project_id)).await? {
            Some(name) => format!("\"{name}\""),
            None => "this project".to_string(),
        }
    };
    confirm_sensitive_action(
        app,
        "Delete this project?",
        SensitiveActionTone::Warning,
        format!(
            "This permanently removes {subject}, along with its environments, scan history, issues, reports, and saved workflow data."
        ),
        "Delete Project",
    )
    .await?;
    let db = (*db).clone();
    run_blocking(move || db.delete_project(project_id))
        .await?
        .map_err(sanitize_error)
}

/// Add a URL/environment to an existing project
#[tauri::command]
#[tracing::instrument(skip(db, url), fields(project_id, label = %label, environment = %environment))]
pub async fn add_environment_url(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: String,
    label: String,
    environment: String,
) -> Result<i64, String> {
    validate_url_async(&url).await?;
    let normalized_environment = normalize_environment_for_url(&url, &environment);
    let db = (*db).clone();
    run_blocking(move || {
        db.add_environment(project_id, &url, &label, &normalized_environment, "manual")
    })
    .await?
    .map_err(sanitize_error)
}

/// Delete an environment URL from a project
#[tracing::instrument(skip(app, db), fields(environment_id))]
pub async fn delete_environment(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    environment_id: i64,
) -> Result<(), String> {
    confirm_sensitive_action(
        app,
        "Remove this environment?",
        SensitiveActionTone::Warning,
        "This removes the selected site URL from the project and may hide scan history tied to that environment.".to_string(),
        "Remove Environment",
    )
    .await?;
    let db = (*db).clone();
    run_blocking(move || db.delete_environment(environment_id))
        .await?
        .map_err(sanitize_error)
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
