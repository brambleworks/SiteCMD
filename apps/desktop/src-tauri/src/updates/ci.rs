//! Recent GitHub Actions failure detector for the updates adapter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiFailure {
    pub workflow_name: String,
    pub run_id: u64,
    pub conclusion: String, // "failure" | "cancelled" | "timed_out"
    pub html_url: String,
    pub commit_sha: String,
    pub completed_at: String,
}

/// Returns the latest failed CI run on the default branch, or `Ok(None)`.
///
/// `repo` must be in `"owner/repo"` format.
pub async fn latest_ci_failure(oauth_token: &str, repo: &str) -> Result<Option<CiFailure>, String> {
    let client = crate::http_client::client();
    let runs = crate::integrations::github::fetch_workflow_runs(client, oauth_token, repo).await?;

    for run in runs {
        let conclusion = run.conclusion.as_deref().unwrap_or("");
        if matches!(conclusion, "failure" | "cancelled" | "timed_out") {
            return Ok(Some(CiFailure {
                workflow_name: run.name,
                run_id: run.id,
                conclusion: conclusion.to_string(),
                html_url: run.html_url,
                commit_sha: run.head_sha,
                completed_at: run.updated_at,
            }));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Full CI fetch requires a real GitHub API; smoke-test serialisation only.
    #[test]
    fn ci_failure_serializes() {
        let ci = CiFailure {
            workflow_name: "CI".into(),
            run_id: 123,
            conclusion: "failure".into(),
            html_url: "https://github.com/x/y/actions/runs/123".into(),
            commit_sha: "abcdef12345".into(),
            completed_at: "2026-04-20T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&ci).unwrap();
        assert!(json.contains("failure"));
    }
}
