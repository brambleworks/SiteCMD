//! Dashboard work-list entries derived from the unified issue model.
//!
//! This combines issue lifecycle, detected signals, fix attempts, and updates
//! without persisting a second workflow state machine.

use crate::checks::Severity;
use crate::core::types_work_items::{IssueGroup, IssueStatus};
use crate::db::{Database, ProjectWorkItem, WorkItemKind, WorkItemStatus, WorkItemTarget};

fn web_work_item_key(url: &str, check_id: &str) -> String {
    format!("web:{}:{}", url.trim_end_matches('/'), check_id)
}

fn code_work_item_key(project_id: i64, environment_url: Option<&str>, issue_id: &str) -> String {
    let scope = environment_url.unwrap_or("");
    format!(
        "code:{}:{}:{}",
        project_id,
        scope.trim_end_matches('/'),
        issue_id
    )
}

fn update_work_item_key(project_id: i64, ecosystem: &str, name: &str) -> String {
    format!("update:{}:{}:{}", project_id, ecosystem, name)
}

fn update_ecosystem_key(ecosystem: &crate::updates::types::Ecosystem) -> String {
    serde_json::to_string(ecosystem)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn update_work_item_target_id(ecosystem: &crate::updates::types::Ecosystem, name: &str) -> String {
    format!("{}:{}", update_ecosystem_key(ecosystem), name)
}

pub(crate) fn build_work_target(
    page: &str,
    project_id: i64,
    environment_url: Option<&str>,
    item_id: Option<&str>,
    focus: Option<&str>,
    file_path: Option<&str>,
    reason: &str,
    scan_kind: Option<&str>,
) -> WorkItemTarget {
    WorkItemTarget {
        page: page.to_string(),
        project_id: Some(project_id),
        url: environment_url.map(|value| value.to_string()),
        scan_id: None,
        session_id: None,
        scan_kind: scan_kind.map(|value| value.to_string()),
        focus: focus.map(|value| value.to_string()),
        item_id: item_id.map(|value| value.to_string()),
        prompt_id: None,
        lane: None,
        reason: Some(reason.to_string()),
        file_path: file_path.map(|value| value.to_string()),
        restore_scan: None,
    }
}

pub(crate) fn parse_timestamp_millis(value: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.timestamp_millis())
        .and_then(|millis| u64::try_from(millis).ok())
}

fn millis_to_rfc3339(millis: i64) -> String {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|value| value.to_rfc3339())
        .unwrap_or_default()
}

/// Map stored issue states to dashboard queue states.
fn issue_status_to_work_item_status(status: IssueStatus) -> WorkItemStatus {
    match status {
        IssueStatus::New => WorkItemStatus::New,
        IssueStatus::Snoozed => WorkItemStatus::Snoozed,
        IssueStatus::Ignored => WorkItemStatus::Ignored,
        IssueStatus::Blocked => WorkItemStatus::Blocked,
        IssueStatus::Verified => WorkItemStatus::Verified,
        IssueStatus::Regressed => WorkItemStatus::Regressed,
    }
}

/// Lane label from the promoted `domain` column, or the generic code fallback.
fn code_domain(group: &IssueGroup) -> Option<String> {
    group
        .instances
        .iter()
        .find_map(|instance| instance.domain)
        .map(|domain| domain.as_str().to_string())
}

fn issue_group_work_entry(
    project_id: i64,
    environment_url: Option<&str>,
    group: &IssueGroup,
    verifying: &std::collections::HashSet<String>,
) -> ProjectWorkItem {
    // A cross-source group is a unified issue, not a Code-only issue. Attribute
    // it to Web for the two-way presentation filter, matching the frontend's
    // canonical IssueGroup summary while preserving every occurrence in the
    // dossier.
    let kind = if group.sources.iter().all(|source| source == "code_scan") {
        WorkItemKind::Code
    } else {
        WorkItemKind::Web
    };
    let mut status = issue_status_to_work_item_status(group.status);
    // Verify-in-flight comes from fix_attempts, the single source of that
    // state. Deliberate suppressions (ignored/blocked/snoozed/verified) win
    // over an in-flight attempt so a dismissal is never overridden.
    if matches!(status, WorkItemStatus::New | WorkItemStatus::Regressed)
        && verifying.contains(&group.check_id)
    {
        status = WorkItemStatus::Working;
    }
    let first_seen = group
        .instances
        .iter()
        .map(|instance| instance.first_seen_at)
        .min()
        .unwrap_or(0);
    let last_seen = group
        .instances
        .iter()
        .map(|instance| instance.last_seen_at)
        .max()
        .unwrap_or(first_seen);
    let domain = (kind == WorkItemKind::Code)
        .then(|| code_domain(group))
        .flatten();
    let (stable_key, reason, scan_kind) = match kind {
        WorkItemKind::Code => (
            code_work_item_key(project_id, environment_url, &group.check_id),
            "code-issue",
            Some("code"),
        ),
        _ => (
            web_work_item_key(environment_url.unwrap_or(""), &group.check_id),
            "web-issue",
            Some("site"),
        ),
    };
    ProjectWorkItem {
        stable_key,
        project_id,
        environment_url: environment_url.map(|value| value.to_string()),
        kind,
        status,
        severity: Some(group.severity),
        title: group.title.clone(),
        summary: group.description.clone(),
        category: Some(group.category.clone()),
        domain: domain.clone(),
        package_name: None,
        target: build_work_target(
            "issues",
            project_id,
            environment_url,
            Some(&group.check_id),
            domain.as_deref(),
            None,
            reason,
            scan_kind,
        ),
        first_seen_at: millis_to_rfc3339(first_seen),
        last_seen_at: millis_to_rfc3339(last_seen),
        last_verified_at: None,
        last_status_changed_at: millis_to_rfc3339(last_seen),
        snooze_until: group.snooze_until,
        block_reason: group.block_reason.clone(),
    }
}

/// Build dashboard entries for active Web and Code issue groups.
/// Update groups use the fresher package snapshot path instead.
fn build_issue_work_entries_with_source_policy(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    include_update_groups: bool,
) -> Result<Vec<ProjectWorkItem>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let groups = db.get_active_issue_groups(project_id, environment_url, now_ms)?;
    let verifying = db.active_fix_attempt_check_ids(project_id, environment_url)?;
    Ok(groups
        .iter()
        .filter(|group| {
            include_update_groups || !group.sources.iter().all(|source| source == "updates")
        })
        .map(|group| issue_group_work_entry(project_id, environment_url, group, &verifying))
        .collect())
}

pub(crate) fn build_issue_work_entries(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Result<Vec<ProjectWorkItem>, String> {
    build_issue_work_entries_with_source_policy(db, project_id, environment_url, false)
}

/// Canonical active-group projection including update-only count entries.
pub(crate) fn build_canonical_issue_work_entries(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Result<Vec<ProjectWorkItem>, String> {
    build_issue_work_entries_with_source_policy(db, project_id, environment_url, true)
}

/// The canonical check_id shared by every vulnerable package (see
/// `signal_mapping.rs`): update lifecycle actions land on this row in
/// `project_issue_states`, so suppressing it suppresses every package entry.
fn security_update_check_id() -> String {
    crate::core::correlation::resolve_check_id("updates", "vulnerability")
}

/// Per-package security-update entries from the updates snapshot, with
/// lifecycle overlaid from `project_issue_states` under the shared
/// `dependencies.vulnerability` check_id (the same row the UpdateDossier's
/// lifecycle actions write through `@/lib/issues`).
pub(crate) fn build_update_work_entries(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    updates: Option<&crate::updates::types::UpdateReport>,
) -> Result<Vec<ProjectWorkItem>, String> {
    let mut entries = build_update_work_items(project_id, environment_url, updates);
    if entries.is_empty() {
        return Ok(entries);
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let check_id = security_update_check_id();
    let state = match environment_url {
        Some(env) => db.get_issue_state(project_id, Some(env), &check_id)?,
        None => None,
    };
    let mut status = match &state {
        Some((raw, snooze_until, ..)) => {
            issue_status_to_work_item_status(raw.effective(*snooze_until, now_ms))
        }
        None => WorkItemStatus::New,
    };
    if matches!(status, WorkItemStatus::New | WorkItemStatus::Regressed)
        && db
            .active_fix_attempt_check_ids(project_id, environment_url)?
            .contains(&check_id)
    {
        status = WorkItemStatus::Working;
    }
    for entry in &mut entries {
        entry.status = status.clone();
        entry.snooze_until = state.as_ref().and_then(|row| row.1);
        entry.block_reason = state.as_ref().and_then(|row| row.2.clone());
    }
    Ok(entries)
}

pub(crate) fn build_update_work_items(
    project_id: i64,
    environment_url: Option<&str>,
    updates: Option<&crate::updates::types::UpdateReport>,
) -> Vec<ProjectWorkItem> {
    let Some(updates) = updates else {
        return Vec::new();
    };
    let now = chrono::Utc::now().to_rfc3339();
    updates
        .updates
        .iter()
        .filter(|update| {
            update.is_security
                && update
                    .advisory_severity
                    .as_deref()
                    .map(|severity| severity.eq_ignore_ascii_case("critical"))
                    .unwrap_or(false)
        })
        .map(|update| {
            let ecosystem_key = update_ecosystem_key(&update.ecosystem);
            let target_item_id = update_work_item_target_id(&update.ecosystem, &update.name);
            let summary = update
                .advisory_fixed_version
                .as_deref()
                .map(|version| format!("{} → {}", update.current_version, version))
                .unwrap_or_else(|| format!("{} (no fixed release)", update.current_version));

            ProjectWorkItem {
                stable_key: update_work_item_key(project_id, &ecosystem_key, &update.name),
                project_id,
                environment_url: environment_url.map(|value| value.to_string()),
                kind: WorkItemKind::Update,
                status: WorkItemStatus::New,
                severity: Some(Severity::Critical),
                title: format!("{} has a critical security advisory", update.name),
                summary,
                category: Some("updates".to_string()),
                domain: None,
                package_name: Some(update.name.clone()),
                target: build_work_target(
                    "updates",
                    project_id,
                    environment_url,
                    Some(&target_item_id),
                    None,
                    None,
                    "security-update",
                    None,
                ),
                first_seen_at: now.clone(),
                last_seen_at: now.clone(),
                last_verified_at: None,
                last_status_changed_at: now.clone(),
                snooze_until: None,
                block_reason: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::project_snapshot::build_project_work_queue;
    use crate::db::test_helpers::temp_db;
    use crate::db::work_items::WorkItemInput;
    use crate::db::work_items::WorkItemMetadata;
    use crate::db::IssueLifecycle;
    use crate::scoring::calculator::compute_current_score;

    fn input(
        source: &str,
        signal_id: &str,
        check_id: &str,
        detail_json: Option<&str>,
    ) -> WorkItemInput {
        WorkItemInput {
            project_id: 1,
            env_url: "https://example.com".into(),
            source: source.into(),
            signal_id: signal_id.into(),
            check_id: check_id.into(),
            category: "security".into(),
            severity: Severity::High,
            title: "Missing CSP header".into(),
            description: "No Content-Security-Policy".into(),
            detail_json: detail_json.map(String::from),
            scan_ref: None,
            page_url: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: 1_000,
            metadata: WorkItemMetadata::default(),
        }
    }

    fn queue_keys(queue: &crate::db::ProjectWorkQueue) -> Vec<String> {
        queue
            .resume_now
            .iter()
            .chain(&queue.verify_now)
            .chain(&queue.fix_next)
            .chain(&queue.maintenance)
            .map(|item| item.stable_key.clone())
            .collect()
    }

    #[test]
    fn ignoring_an_issue_hides_it_from_score_list_and_dashboard_queue() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Disagreement", "/tmp/disagreement", Some("astro"))
            .expect("upsert");
        db.upsert_work_items_diff(
            "web_scan",
            project_id,
            "https://example.com",
            vec![input("web_scan", "web:csp:1", "security.csp", None)],
            1_000,
        )
        .expect("seed item");

        db.set_issue_group_state(
            project_id,
            "https://example.com",
            "security.csp",
            IssueLifecycle::Ignored,
            2_000,
        )
        .expect("ignore via the dossier's write path");

        // Score: no penalty.
        let groups = db
            .get_active_issue_groups(project_id, Some("https://example.com"), 3_000)
            .expect("score groups");
        assert_eq!(compute_current_score(&groups, 3_000).overall, 100.0);

        // Issues list: the group reports ignored, the status the list filters on.
        let listed = db
            .get_work_items_grouped(project_id, Some("https://example.com"), 3_000)
            .expect("list groups");
        assert_eq!(
            listed[0].status,
            crate::core::types_work_items::IssueStatus::Ignored
        );

        // Dashboard queue: absent from every lane.
        let entries = build_issue_work_entries(&db, project_id, Some("https://example.com"))
            .expect("entries");
        let queue = build_project_work_queue(&entries, Vec::new());
        assert!(
            queue_keys(&queue).is_empty(),
            "an ignored issue must not appear in any dashboard lane"
        );
    }

    #[test]
    fn dashboard_entries_derive_kind_domain_and_verify_in_flight() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Entries", "/tmp/entries", Some("astro"))
            .expect("upsert");
        db.upsert_work_items_diff(
            "web_scan",
            project_id,
            "https://example.com",
            vec![input("web_scan", "web:csp:1", "security.csp", None)],
            1_000,
        )
        .expect("seed web");
        // The lane label reads the promoted domain column, not the
        // blob - a blob-only domain must NOT surface.
        let mut code_input = input(
            "code_scan",
            "code_scan:nplus:a",
            "code_scan.n-plus-one-query",
            Some(r#"{"domain":"ignored-blob-value"}"#),
        );
        code_input.metadata.domain = Some(crate::core::code_scan::CodeScanDomain::Architecture);
        db.upsert_work_items_diff(
            "code_scan",
            project_id,
            "https://example.com",
            vec![code_input],
            1_000,
        )
        .expect("seed code");
        // Updates-source signals stay off the dashboard queue (their
        // per-package entries come from the updates snapshot instead).
        db.upsert_work_items_diff(
            "updates",
            project_id,
            "https://example.com",
            vec![input(
                "updates",
                "updates:vulnerability:npm:react",
                "dependencies.vulnerability",
                None,
            )],
            1_000,
        )
        .expect("seed update signal");

        // An in-flight fix attempt moves the web issue to the verify lane.
        db.create_fix_attempt(
            project_id,
            "https://example.com",
            "security.csp",
            "cursor",
            2_000,
        )
        .expect("fix attempt");

        let entries = build_issue_work_entries(&db, project_id, Some("https://example.com"))
            .expect("entries");
        assert_eq!(entries.len(), 2, "updates-source group is skipped");
        let counted =
            build_canonical_issue_work_entries(&db, project_id, Some("https://example.com"))
                .expect("canonical count entries");
        assert_eq!(
            counted.len(),
            3,
            "count surfaces include the update IssueGroup without duplicating it in the queue"
        );

        let web = entries
            .iter()
            .find(|entry| entry.kind == WorkItemKind::Web)
            .expect("web entry");
        assert_eq!(web.status, WorkItemStatus::Working);
        assert_eq!(web.target.item_id.as_deref(), Some("security.csp"));

        let code = entries
            .iter()
            .find(|entry| entry.kind == WorkItemKind::Code)
            .expect("code entry");
        assert_eq!(code.status, WorkItemStatus::New);
        assert_eq!(code.domain.as_deref(), Some("architecture"));
    }

    #[test]
    fn security_update_entries_share_lifecycle_with_the_vulnerability_issue() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Updates", "/tmp/updates", Some("astro"))
            .expect("upsert");
        let report = crate::updates::types::UpdateReport {
            packages: vec![],
            updates: vec![crate::updates::types::PackageUpdate {
                name: "react".to_string(),
                current_version: "18.2.0".to_string(),
                latest_version: "19.0.0".to_string(),
                ecosystem: crate::updates::types::Ecosystem::Npm,
                update_type: crate::updates::types::UpdateType::Major,
                is_security: true,
                advisory_severity: Some("critical".to_string()),
                advisory_url: None,
                source: "package.json".to_string(),
                is_dev: false,
                ..Default::default()
            }],
            ecosystems_detected: vec![crate::updates::types::Ecosystem::Npm],
            scan_duration_ms: 12,
        };

        let fresh =
            build_update_work_entries(&db, project_id, Some("https://example.com"), Some(&report))
                .expect("entries");
        assert_eq!(fresh[0].status, WorkItemStatus::New);
        assert_eq!(fresh[0].title, "react has a critical security advisory");
        assert_eq!(fresh[0].summary, "18.2.0 (no fixed release)");

        // The UpdateDossier ignores through the shared vulnerability check_id;
        // the dashboard entries must follow the same row.
        db.set_issue_state(
            project_id,
            "https://example.com",
            &security_update_check_id(),
            IssueLifecycle::Ignored,
            2_000,
        )
        .expect("ignore updates issue");

        let suppressed =
            build_update_work_entries(&db, project_id, Some("https://example.com"), Some(&report))
                .expect("entries after ignore");
        assert_eq!(suppressed[0].status, WorkItemStatus::Ignored);
    }
}
