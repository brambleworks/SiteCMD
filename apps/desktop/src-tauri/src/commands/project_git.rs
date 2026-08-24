use std::sync::Arc;

use tauri::State;

use crate::core::git;
use crate::db::{Database, SiteEvent};

use super::{run_blocking, sanitize_error};

/// Get git status (branch, commits, dirty state) for a project's local repo.
#[tracing::instrument(skip(db), fields(project_id, limit))]
pub async fn get_git_status(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    limit: Option<u32>,
) -> Result<git::GitStatus, String> {
    let project_path = db.get_project_path_async(project_id).await;
    let Some(project_path) = project_path else {
        return Ok(git::GitStatus {
            is_git_repo: false,
            branch: None,
            commits: Vec::new(),
            total_commits: 0,
            has_uncommitted: false,
        });
    };
    let limit = limit.unwrap_or(20).clamp(1, 200);

    git::get_git_status_async(project_path, limit)
        .await
        .map_err(sanitize_error)
}

/// Get git commits since a given date and auto-create deploy events from them.
#[tracing::instrument(skip(db), fields(project_id, since = %since))]
pub async fn get_commits_since(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    since: String,
) -> Result<Vec<git::GitCommit>, String> {
    let db = (*db).clone();
    let project_path = db.get_project_path_async(project_id).await;
    let Some(project_path) = project_path else {
        return Ok(Vec::new());
    };

    let commits = git::get_commits_since_async(project_path, since)
        .await
        .map_err(sanitize_error)?;

    if !commits.is_empty() {
        let events: Vec<SiteEvent> = commits
            .iter()
            .map(|c| git::commit_to_deploy_event(c, project_id))
            .collect();
        run_blocking(move || {
            if let Err(e) = db.insert_events(&events) {
                tracing::warn!("Failed to insert commit events: {}", e);
            }
        })
        .await?;
    }

    Ok(commits)
}
