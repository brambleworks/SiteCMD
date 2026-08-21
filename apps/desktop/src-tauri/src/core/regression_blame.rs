//! Attributes new findings to a named commit window after scan completion.
//!
//! Findings are eligible only when their check contracts are comparable across
//! the two recorded engine releases.

use std::collections::{HashMap, HashSet};

use sitecmd_engine::release::{comparability, Comparability};

use crate::checks::Severity;
use crate::core::git::{get_commits_between, GitCommit};
use crate::db::alerts::AlertInput;
use crate::db::RegressionInput;
use crate::db::StoredBasis;
use crate::db::{Database, DbError};

const SITECMD_ALERT_SOURCE: &str = "sitecmd";
/// Blame fires below the generic 10-point threshold so "dropped 8 points"
/// alerts exist - but only with an attributable deploy AND new issues.
const BLAME_SCORE_DROP_THRESHOLD: i64 = 5;
const CRITICAL_SCORE_DROP_THRESHOLD: i64 = 20;
/// Commits stored on the row/alert; the true window count is kept alongside.
pub const STORED_COMMITS_MAX: usize = 20;

/// Pre-persist state captured BEFORE work items are upserted for the current
/// scan. `active_check_ids` are canonical check_ids with resolved_at IS NULL.
#[derive(Debug, Clone)]
pub struct BlameSnapshot {
    pub previous: Option<PreviousScan>,
    pub active_check_ids: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct PreviousScan {
    pub scan_id: i64,
    pub overall_score: i64,
    /// RFC 3339, used as the git window's --since.
    pub timestamp: String,
}

/// Canonical current failure; duplicate check ids collapse to highest severity.
#[derive(Debug, Clone)]
pub struct CurrentIssue {
    pub check_id: String,
    pub title: String,
    pub severity: Severity,
}

/// Returned to the (commands-layer) caller so it can fire the desktop
/// notification; core stays tauri-free.
#[derive(Debug, Clone)]
pub struct RegressionNotice {
    pub regression_id: i64,
    pub title: String,
    pub body: String,
}

/// Everything needed to emit one blame record, independent of scan kind.
pub struct BlameContext<'a> {
    pub db: &'a Database,
    pub project_id: i64,
    pub env_url: &'a str,
    pub scan_kind: &'a str, // "web" | "code"
    pub scan_id: i64,
    pub current_score: i64,
    /// RFC 3339 completion time of the current scan (git window --until).
    pub current_timestamp: &'a str,
    pub current_issues: &'a [CurrentIssue],
    pub project_path: Option<&'a str>,
}

/// Pure decision: does this scan get a blame alert?
/// `new_critical_or_high` means at least one introduced issue is critical or high.
pub fn should_blame(
    score_drop: i64,
    new_issue_count: usize,
    commit_count: u32,
    new_critical_or_high: bool,
) -> bool {
    if new_issue_count == 0 || commit_count == 0 {
        return false;
    }
    score_drop >= BLAME_SCORE_DROP_THRESHOLD || new_critical_or_high
}

/// Alert severity per the spec: critical when a new critical issue exists or
/// the drop is >= 20 points; warn otherwise.
pub fn blame_severity(score_drop: i64, has_new_critical: bool) -> &'static str {
    if has_new_critical || score_drop >= CRITICAL_SCORE_DROP_THRESHOLD {
        "critical"
    } else {
        "warn"
    }
}

/// Provenance for both compared runs. Missing stamps make the result unattributable.
pub(crate) struct Attribution {
    before: Option<StoredBasis>,
    after: Option<StoredBasis>,
}

impl Attribution {
    pub(crate) fn between(db: &Database, before_run_id: i64, after_run_id: i64) -> Self {
        let read = |run_id: i64| match db.run_release_basis(run_id) {
            Ok(basis) => basis,
            Err(error) => {
                tracing::warn!("could not read scan provenance for run {run_id}: {error}");
                None
            }
        };
        Self {
            before: read(before_run_id),
            after: read(after_run_id),
        }
    }

    pub(crate) fn verdict(&self, check_id: &str) -> Comparability {
        comparability(
            check_id,
            self.before.as_ref().map(StoredBasis::basis),
            self.after.as_ref().map(StoredBasis::basis),
        )
    }

    /// Whether a check difference may be attributed to the deploy.
    ///
    /// Scanner changes and missing provenance prevent attribution. Unknown
    /// check ids remain attributable because neither build owns them.
    pub(crate) fn attributable(&self, check_id: &str) -> bool {
        matches!(
            self.verdict(check_id),
            Comparability::Comparable | Comparability::Unregistered
        )
    }

    /// The release the current run was produced by, for the copy that explains
    /// what was held back.
    pub(crate) fn engine_release(&self) -> Option<&str> {
        self.after
            .as_ref()
            .map(|basis| basis.stamp.engine_release.as_str())
    }
}

/// Keep the highest-severity finding for each canonical check ID.
fn dedup_by_check_id(issues: Vec<&CurrentIssue>) -> Vec<&CurrentIssue> {
    let mut best: HashMap<&str, &CurrentIssue> = HashMap::new();
    for issue in issues {
        match best.get(issue.check_id.as_str()) {
            Some(kept) if kept.severity.impact_rank() >= issue.severity.impact_rank() => {}
            _ => {
                best.insert(issue.check_id.as_str(), issue);
            }
        }
    }
    best.into_values().collect()
}

/// Capture the pre-persist snapshot for one (project, env, source).
/// `source` is the work_items source: "web_scan" or "code_scan".
pub fn capture_snapshot(
    db: &Database,
    project_id: i64,
    env_url: &str,
    source: &str,
    previous: Option<PreviousScan>,
) -> Result<BlameSnapshot, DbError> {
    let active_check_ids = db
        .get_active_work_item_idents(project_id, Some(env_url))?
        .into_iter()
        .filter(|(_check_id, row_source)| row_source == source)
        .map(|(check_id, _source)| check_id)
        .collect::<HashSet<_>>();
    Ok(BlameSnapshot {
        previous,
        active_check_ids,
    })
}

/// Main entry. Returns Some(notice) when a blame alert was created.
pub fn emit_regression_blame(
    ctx: BlameContext<'_>,
    snapshot: &BlameSnapshot,
) -> Option<RegressionNotice> {
    let previous = snapshot.previous.as_ref()?;
    let project_path = ctx.project_path?;

    let current_ids: HashSet<&str> = ctx
        .current_issues
        .iter()
        .map(|issue| issue.check_id.as_str())
        .collect();
    // Dedup before counting: the headline count, detail list, and junction
    // rows must all describe the same per-check_id set.
    let appeared: Vec<&CurrentIssue> = dedup_by_check_id(
        ctx.current_issues
            .iter()
            .filter(|issue| !snapshot.active_check_ids.contains(&issue.check_id))
            .collect(),
    );
    // Split what the deploy can be held to from what the scanner did to
    // itself. A check the previous run's build could not produce, or produced
    // under a different meaning, explains its own appearance.
    let attribution = Attribution::between(ctx.db, previous.scan_id, ctx.scan_id);
    let (new_issues, detector_changed): (Vec<&CurrentIssue>, Vec<&CurrentIssue>) = appeared
        .into_iter()
        .partition(|issue| attribution.attributable(&issue.check_id));
    if !detector_changed.is_empty() {
        tracing::info!(
            held_back = detector_changed.len(),
            "findings withheld from deploy blame: their checks changed between the two runs"
        );
    }
    let mut fixed_check_ids: Vec<&String> = snapshot
        .active_check_ids
        .iter()
        .filter(|check_id| !current_ids.contains(check_id.as_str()))
        // A retired or re-contracted check stops producing findings. That is
        // the scanner shrinking, and counting it as fixed would credit a
        // deploy for work nobody did.
        .filter(|check_id| attribution.attributable(check_id))
        .collect();
    fixed_check_ids.sort();

    let score_drop = previous.overall_score - ctx.current_score;
    let (commits, commit_count) =
        get_commits_between(project_path, &previous.timestamp, ctx.current_timestamp);
    let new_critical = new_issues
        .iter()
        .any(|issue| issue.severity == Severity::Critical);
    let new_critical_or_high = new_critical
        || new_issues
            .iter()
            .any(|issue| issue.severity == Severity::High);

    if !should_blame(
        score_drop,
        new_issues.len(),
        commit_count,
        new_critical_or_high,
    ) {
        return None;
    }

    // git log prints newest first.
    let commit_to = commits.first().map(|c| c.hash.clone())?;
    // Known v1 limitation: when the window is capped (commit_count >
    // commits.len), commit_from is the oldest STORED commit, not the
    // window's true oldest.
    let commit_from = commits.last().map(|c| c.hash.clone())?;
    let stored: Vec<&GitCommit> = commits.iter().take(STORED_COMMITS_MAX).collect();
    let commits_json = serde_json::to_string(
        &stored
            .iter()
            .map(|c| {
                serde_json::json!({
                    "hash": c.hash,
                    "short_hash": c.short_hash,
                    "message": c.message,
                    "author": c.author,
                    "date": c.date,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());

    let mut new_sorted: Vec<&CurrentIssue> = new_issues.clone();
    new_sorted.sort_by(|a, b| a.check_id.cmp(&b.check_id));
    let now = chrono::Utc::now().timestamp_millis();

    let regression_id = match ctx.db.insert_regression(RegressionInput {
        project_id: ctx.project_id,
        env_url: ctx.env_url.to_string(),
        scan_type: ctx.scan_kind.to_string(),
        prev_scan_id: previous.scan_id,
        scan_id: ctx.scan_id,
        prev_score: previous.overall_score,
        score: ctx.current_score,
        commit_from: commit_from.clone(),
        commit_to: commit_to.clone(),
        commit_count: commit_count as i64,
        commits_json: commits_json.clone(),
        new_check_ids: new_sorted.iter().map(|i| i.check_id.clone()).collect(),
        fixed_check_ids_json: serde_json::to_string(&fixed_check_ids)
            .unwrap_or_else(|_| "[]".to_string()),
        created_at: now,
    }) {
        Ok(id) => id,
        Err(error) => {
            tracing::warn!("failed to insert regression: {}", error);
            return None;
        }
    };

    let detail = build_detail_json(DetailArgs {
        scan_kind: ctx.scan_kind,
        scan_id: ctx.scan_id,
        regression_id,
        previous_score: previous.overall_score,
        current_score: ctx.current_score,
        score_drop,
        new_issues: &new_sorted,
        fixed_count: fixed_check_ids.len(),
        detector_changed_count: detector_changed.len(),
        engine_release: attribution.engine_release(),
        commit_from: &commit_from,
        commit_to: &commit_to,
        commit_count,
        commits_json: &commits_json,
        env_url: ctx.env_url,
    });

    let scan_label = if ctx.scan_kind == "web" {
        "Web Scan"
    } else {
        "Code Scan"
    };
    let title = if score_drop >= BLAME_SCORE_DROP_THRESHOLD {
        format!("Deploy dropped the {scan_label} score by {score_drop} points")
    } else {
        format!(
            "Deploy introduced {} new {scan_label} {}",
            new_sorted.len(),
            if new_sorted.len() == 1 {
                "finding"
            } else {
                "findings"
            }
        )
    };
    let short_from = &commit_from[..commit_from.len().min(7)];
    let short_to = &commit_to[..commit_to.len().min(7)];
    // Honest copy when the window outgrew the stored list: name how many of
    // the counted commits the range actually covers.
    let capped_note = if commit_count as usize > commits.len() {
        format!(" (newest {} shown)", commits.len())
    } else {
        String::new()
    };
    // Say what was held back and why, so a count smaller than the issues list
    // reads as a decision rather than an inconsistency.
    let withheld_note = match (detector_changed.len(), attribution.engine_release()) {
        (0, _) => String::new(),
        (count, Some(release)) => format!(
            " {} other {} {} from {} that changed in SiteCMD {}, so {} not attributed to these commits.",
            count,
            if count == 1 { "finding" } else { "findings" },
            if count == 1 { "comes" } else { "come" },
            if count == 1 { "a check" } else { "checks" },
            release,
            if count == 1 { "it is" } else { "they are" },
        ),
        (count, None) => format!(
            " {} other {} {} from {} that changed between the two scans, so {} not attributed to these commits.",
            count,
            if count == 1 { "finding" } else { "findings" },
            if count == 1 { "comes" } else { "come" },
            if count == 1 { "a check" } else { "checks" },
            if count == 1 { "it is" } else { "they are" },
        ),
    };
    let description = format!(
        "{} {} landed between the last two scans ({}..{}){}. {} new {} appeared and {} resolved.{} The commit range is the blame window.",
        commit_count,
        if commit_count == 1 { "commit" } else { "commits" },
        short_from,
        short_to,
        capped_note,
        new_sorted.len(),
        if new_sorted.len() == 1 { "issue" } else { "issues" },
        fixed_check_ids.len(),
        withheld_note,
    );

    let severity = blame_severity(score_drop, new_critical);
    let alert = AlertInput {
        project_id: ctx.project_id,
        env_url: Some(ctx.env_url.to_string()),
        source: SITECMD_ALERT_SOURCE.to_string(),
        alert_id: format!("deploy-regression:{}:{}", ctx.scan_kind, ctx.scan_id),
        severity: severity.to_string(),
        title: title.clone(),
        description,
        detail_json: Some(detail),
        occurred_at: now,
        observed_at: now,
    };
    if let Err(error) = ctx.db.upsert_alert(alert) {
        tracing::warn!("failed to upsert deploy-regression alert: {}", error);
        return None;
    }

    Some(RegressionNotice {
        regression_id,
        title,
        body: format!(
            "{} new {} after {} {} on {}",
            new_sorted.len(),
            if new_sorted.len() == 1 {
                "issue"
            } else {
                "issues"
            },
            commit_count,
            if commit_count == 1 {
                "commit"
            } else {
                "commits"
            },
            ctx.env_url,
        ),
    })
}

struct DetailArgs<'a> {
    scan_kind: &'a str,
    scan_id: i64,
    regression_id: i64,
    previous_score: i64,
    current_score: i64,
    score_drop: i64,
    new_issues: &'a [&'a CurrentIssue],
    fixed_count: usize,
    /// Findings held back because their check changed between the two runs.
    /// Rendered so the dossier's count is explainable rather than merely
    /// smaller than what the issues list shows.
    detector_changed_count: usize,
    engine_release: Option<&'a str>,
    commit_from: &'a str,
    commit_to: &'a str,
    commit_count: u32,
    commits_json: &'a str,
    env_url: &'a str,
}

fn build_detail_json(args: DetailArgs<'_>) -> String {
    let commits: serde_json::Value =
        serde_json::from_str(args.commits_json).unwrap_or_else(|_| serde_json::json!([]));
    serde_json::json!({
        "alert_type": "deploy_regression",
        "scan_kind": args.scan_kind,
        "scan_id": args.scan_id,
        "regression_id": args.regression_id,
        "previous_score": args.previous_score,
        "current_score": args.current_score,
        "score_drop": args.score_drop,
        "new_issues": args.new_issues
            .iter()
            .map(|issue| serde_json::json!({ "check_id": issue.check_id, "title": issue.title }))
            .collect::<Vec<_>>(),
        "fixed_count": args.fixed_count,
        "detector_changed_count": args.detector_changed_count,
        "engine_release": args.engine_release,
        "commit_from": args.commit_from,
        "commit_to": args.commit_to,
        "commit_count": args.commit_count,
        "commits": commits,
        "url": args.env_url,
        "destination": "issues"
    })
    .to_string()
}

#[cfg(test)]
#[path = "regression_blame_tests.rs"]
mod tests;
