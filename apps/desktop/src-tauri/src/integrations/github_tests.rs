use super::*;

#[test]
fn repository_identity_uses_githubs_immutable_id_and_canonical_name() {
    let identity = parse_repository_identity(&serde_json::json!({
        "id": 1296269,
        "full_name": "BrambleWorks/SiteCMD"
    }))
    .expect("valid repository identity");

    assert_eq!(identity.id, "1296269");
    assert_eq!(identity.full_name, "BrambleWorks/SiteCMD");
}

#[test]
fn truncate_sha_picks_first_n_chars() {
    assert_eq!(truncate_sha("abcdef1234567890", 7), "abcdef1");
    assert_eq!(truncate_sha("abc", 7), "abc", "short SHA must pass through");
    assert_eq!(truncate_sha("", 7), "");
}

#[test]
fn truncate_sha_handles_zero_length() {
    // Defensive - caller might pass 0 by accident; must not panic.
    assert_eq!(truncate_sha("abcdef", 0), "");
}

#[test]
fn calculate_run_duration_returns_seconds_between_timestamps() {
    // 10:00 → 10:05 = 5 minutes = 300 seconds.
    let duration = calculate_run_duration(Some("2026-04-19T10:00:00Z"), "2026-04-19T10:05:00Z");
    assert_eq!(duration, Some(300));
}

#[test]
fn calculate_run_duration_returns_none_when_started_missing() {
    // GitHub may omit run_started_at on queued/never-started runs.
    assert!(calculate_run_duration(None, "2026-04-19T10:00:00Z").is_none());
}

#[test]
fn calculate_run_duration_returns_none_for_unparseable_timestamps() {
    assert!(calculate_run_duration(Some("garbage"), "2026-04-19T10:00:00Z").is_none());
    assert!(calculate_run_duration(Some("2026-04-19T10:00:00Z"), "garbage").is_none());
    assert!(calculate_run_duration(Some(""), "").is_none());
}

#[test]
fn calculate_run_duration_clamps_negative_to_zero() {
    let duration = calculate_run_duration(Some("2026-04-19T10:05:00Z"), "2026-04-19T10:00:00Z");
    assert_eq!(duration, Some(0));
}

#[test]
fn parse_workflow_runs_returns_empty_for_invalid_body() {
    let result = parse_workflow_runs_response(&serde_json::json!({}));
    assert!(result.is_empty(), "missing workflow_runs key -> empty list");
}

#[test]
fn parse_workflow_runs_extracts_full_payload() {
    let body = serde_json::json!({
        "workflow_runs": [{
            "id": 123u64,
            "name": "CI",
            "head_branch": "main",
            "head_sha": "abc123def",
            "status": "completed",
            "conclusion": "success",
            "run_number": 42u32,
            "created_at": "2026-04-19T10:00:00Z",
            "updated_at": "2026-04-19T10:03:00Z",
            "html_url": "https://github.com/owner/repo/actions/runs/123",
            "run_started_at": "2026-04-19T10:00:30Z"
        }]
    });
    let runs = parse_workflow_runs_response(&body);
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.id, 123);
    assert_eq!(run.name, "CI");
    assert_eq!(run.head_branch, "main");
    assert_eq!(run.head_sha, "abc123def");
    assert_eq!(run.status, "completed");
    assert_eq!(run.conclusion.as_deref(), Some("success"));
    assert_eq!(run.run_number, 42);
    assert_eq!(
        run.duration_seconds,
        Some(150),
        "10:03:00 - 10:00:30 = 150s"
    );
}

#[test]
fn parse_workflow_runs_handles_missing_head_branch() {
    // Detached-head runs (e.g. some webhook-triggered runs) lack
    // head_branch - must default to empty string rather than skip.
    let body = serde_json::json!({
        "workflow_runs": [{
            "id": 1u64,
            "name": "x",
            "head_sha": "deadbeef",
            "status": "completed",
            "run_number": 1u32,
            "created_at": "2026-04-19T10:00:00Z",
            "updated_at": "2026-04-19T10:01:00Z",
            "html_url": "https://github.com/o/r/actions/runs/1"
        }]
    });
    let runs = parse_workflow_runs_response(&body);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].head_branch, "");
}

#[test]
fn parse_workflow_runs_leaves_duration_none_when_run_started_at_missing() {
    let body = serde_json::json!({
        "workflow_runs": [{
            "id": 1u64,
            "name": "x",
            "head_sha": "abc",
            "status": "completed",
            "run_number": 1u32,
            "created_at": "2026-04-19T10:00:00Z",
            "updated_at": "2026-04-19T10:01:00Z",
            "html_url": "https://x.test"
        }]
    });
    let runs = parse_workflow_runs_response(&body);
    assert!(runs[0].duration_seconds.is_none());
}

#[test]
fn pick_deployment_status_returns_first_state() {
    // GitHub returns deployment statuses newest-first.
    let body = serde_json::json!([
        {"state": "success"},
        {"state": "in_progress"},
    ]);
    assert_eq!(pick_deployment_status(&body), "success");
}

#[test]
fn pick_deployment_status_defaults_to_pending_when_empty() {
    // A deployment with no status records yet - caller's UX falls back
    // to "pending" rather than blanking the row.
    assert_eq!(pick_deployment_status(&serde_json::json!([])), "pending");
}

#[test]
fn pick_deployment_status_defaults_to_pending_when_invalid_json() {
    // Malformed body - defensive default.
    assert_eq!(pick_deployment_status(&serde_json::json!({})), "pending");
    assert_eq!(
        pick_deployment_status(&serde_json::json!("garbage")),
        "pending"
    );
}

#[test]
fn parse_pull_requests_extracts_full_payload() {
    let body = serde_json::json!([{
        "number": 42u32,
        "title": "Fix the thing",
        "state": "open",
        "user": {"login": "alice"},
        "head": {"ref": "fix/the-thing"},
        "created_at": "2026-04-18T10:00:00Z",
        "updated_at": "2026-04-19T10:00:00Z",
        "html_url": "https://github.com/o/r/pull/42",
        "draft": false,
        "additions": 50u32,
        "deletions": 20u32,
        "changed_files": 3u32
    }]);
    let prs = parse_pull_requests_response(&body);
    assert_eq!(prs.len(), 1);
    let pr = &prs[0];
    assert_eq!(pr.number, 42);
    assert_eq!(pr.title, "Fix the thing");
    assert_eq!(pr.state, "open");
    assert_eq!(pr.user, "alice");
    assert_eq!(pr.head_branch, "fix/the-thing");
    assert!(!pr.draft);
    assert_eq!(pr.additions, 50);
    assert_eq!(pr.deletions, 20);
    assert_eq!(pr.changed_files, 3);
}

#[test]
fn parse_pull_requests_defaults_optional_metric_fields_to_zero() {
    let body = serde_json::json!([{
        "number": 7u32,
        "title": "WIP",
        "state": "open",
        "user": {"login": "bob"},
        "head": {"ref": "wip"},
        "created_at": "2026-04-19T10:00:00Z",
        "updated_at": "2026-04-19T10:00:00Z",
        "html_url": "https://github.com/o/r/pull/7"
    }]);
    let prs = parse_pull_requests_response(&body);
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].additions, 0);
    assert_eq!(prs[0].deletions, 0);
    assert_eq!(prs[0].changed_files, 0);
    assert!(!prs[0].draft, "missing `draft` defaults to false");
}

#[test]
fn parse_pull_requests_returns_empty_for_invalid_body() {
    // A non-array body (e.g. an error response that slipped through)
    // should produce an empty list rather than panic.
    assert!(parse_pull_requests_response(&serde_json::json!({})).is_empty());
    assert!(parse_pull_requests_response(&serde_json::json!("garbage")).is_empty());
}

#[test]
fn parse_pull_requests_returns_empty_for_empty_array() {
    let prs = parse_pull_requests_response(&serde_json::json!([]));
    assert!(prs.is_empty());
}

#[test]
fn parse_latest_release_returns_summary_for_valid_response() {
    let body = serde_json::json!({
        "tag_name": "v2.14.0",
        "published_at": "2026-04-01T10:00:00Z",
        "target_commitish": "main",
    });
    let summary = parse_latest_release_response(&body).expect("summary");
    assert_eq!(summary.tag_name, "v2.14.0");
    assert_eq!(summary.published_at, "2026-04-01T10:00:00Z");
}

#[test]
fn parse_latest_release_returns_none_when_tag_missing() {
    let body = serde_json::json!({ "published_at": "2026-04-01T10:00:00Z" });
    assert!(parse_latest_release_response(&body).is_none());
}

#[test]
fn parse_latest_release_returns_none_when_published_at_missing() {
    let body = serde_json::json!({ "tag_name": "v2.14.0" });
    assert!(parse_latest_release_response(&body).is_none());
}

#[test]
fn parse_pr_files_extracts_filenames() {
    let body = serde_json::json!([
        {"filename": "src/lib.rs", "status": "modified", "additions": 10, "deletions": 2},
        {"filename": "tests/integration.rs", "status": "added", "additions": 50, "deletions": 0},
    ]);
    let files = parse_pr_files_response(&body);
    assert_eq!(files.len(), 2);
    assert!(files.contains(&"src/lib.rs".to_string()));
    assert!(files.contains(&"tests/integration.rs".to_string()));
}

#[test]
fn parse_pr_files_returns_empty_for_empty_array() {
    let files = parse_pr_files_response(&serde_json::json!([]));
    assert!(files.is_empty());
}

#[test]
fn parse_pr_files_returns_empty_for_invalid_body() {
    assert!(parse_pr_files_response(&serde_json::json!({})).is_empty());
    assert!(parse_pr_files_response(&serde_json::json!("garbage")).is_empty());
}

#[test]
fn parse_pr_files_skips_entries_missing_filename() {
    // Some GitHub API errors or edge cases return entries without filename
    let body = serde_json::json!([
        {"status": "modified"},
        {"filename": "src/main.rs", "status": "added"},
    ]);
    let files = parse_pr_files_response(&body);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0], "src/main.rs");
}

#[test]
fn parse_pull_requests_changed_file_paths_defaults_to_empty() {
    let body = serde_json::json!([{
        "number": 1u32,
        "title": "Test",
        "state": "open",
        "user": {"login": "alice"},
        "head": {"ref": "feature"},
        "created_at": "2026-05-01T00:00:00Z",
        "updated_at": "2026-05-01T00:00:00Z",
        "html_url": "https://github.com/o/r/pull/1"
    }]);
    let prs = parse_pull_requests_response(&body);
    assert_eq!(prs.len(), 1);
    assert!(
        prs[0].changed_file_paths.is_empty(),
        "changed_file_paths should default to empty Vec"
    );
}

#[test]
fn parse_latest_release_tolerates_draft_field() {
    let body = serde_json::json!({
        "tag_name": "v3.0.0",
        "published_at": "2026-04-15T00:00:00Z",
        "draft": false,
        "prerelease": false,
    });
    let summary = parse_latest_release_response(&body).expect("summary");
    assert_eq!(summary.tag_name, "v3.0.0");
}
