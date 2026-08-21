use crate::checks::Severity;
use crate::db::{
    ProjectWorkItem, ProjectWorkQueue, ProjectWorkSummary, WorkItemKind, WorkItemStatus,
};

/// Rank persisted project work items for operator-facing queues.
fn severity_rank(value: Option<Severity>) -> i32 {
    value.map(|s| s.sort_rank()).unwrap_or(4) as i32
}

fn work_item_sort_key(item: &ProjectWorkItem) -> (i32, i32, i32, String) {
    let kind_rank = match item.kind {
        WorkItemKind::Launch => 0,
        WorkItemKind::Code => 1,
        WorkItemKind::Web => 2,
        WorkItemKind::Update => 3,
    };
    let status_rank = match item.status {
        WorkItemStatus::Regressed => 0,
        WorkItemStatus::Working => 1,
        WorkItemStatus::New => 2,
        WorkItemStatus::Snoozed => 2,
        WorkItemStatus::Blocked => 3,
        WorkItemStatus::Ignored => 4,
        WorkItemStatus::Verified => 5,
    };
    (
        kind_rank,
        status_rank,
        severity_rank(item.severity),
        item.title.clone(),
    )
}

#[tracing::instrument(skip(items, maintenance))]
pub(crate) fn build_project_work_queue(
    items: &[ProjectWorkItem],
    maintenance: Vec<ProjectWorkItem>,
) -> ProjectWorkQueue {
    let mut resume_now = Vec::new();
    let mut verify_now = Vec::new();
    let mut fix_next = Vec::new();
    let mut blocked_items = Vec::new();

    for item in items {
        if matches!(
            item.status,
            WorkItemStatus::Ignored | WorkItemStatus::Verified
        ) {
            continue;
        }
        let safe_item = item.clone();
        match item.status {
            WorkItemStatus::Working => verify_now.push(safe_item),
            WorkItemStatus::Regressed => resume_now.push(safe_item),
            WorkItemStatus::New => fix_next.push(safe_item),
            WorkItemStatus::Blocked => blocked_items.push(safe_item),
            _ => {}
        }
    }

    resume_now.sort_by_key(work_item_sort_key);
    verify_now.sort_by_key(work_item_sort_key);
    fix_next.sort_by_key(work_item_sort_key);
    let mut maintenance = maintenance;
    maintenance.extend(blocked_items);
    maintenance.sort_by_key(work_item_sort_key);

    ProjectWorkQueue {
        resume_now,
        verify_now,
        fix_next,
        maintenance,
    }
}

#[tracing::instrument(skip(items, queue))]
pub(crate) fn build_project_work_summary(
    items: &[ProjectWorkItem],
    queue: &ProjectWorkQueue,
) -> ProjectWorkSummary {
    let active_issue_items = items.iter().filter(|item| {
        matches!(item.kind, WorkItemKind::Web | WorkItemKind::Code)
            && matches!(
                item.status,
                WorkItemStatus::New | WorkItemStatus::Working | WorkItemStatus::Regressed
            )
    });
    let mut issue_count = 0;
    let mut issue_web_count = 0;
    let mut issue_code_count = 0;
    let mut issue_critical_count = 0;
    let mut issue_high_count = 0;
    let mut issue_medium_count = 0;
    let mut issue_low_count = 0;
    for item in active_issue_items {
        issue_count += 1;
        match item.kind {
            WorkItemKind::Code => issue_code_count += 1,
            WorkItemKind::Web => issue_web_count += 1,
            _ => unreachable!("active issue filter only admits Web and Code work items"),
        }
        match item.severity.unwrap_or(Severity::Low) {
            Severity::Critical => issue_critical_count += 1,
            Severity::High => issue_high_count += 1,
            Severity::Medium => issue_medium_count += 1,
            Severity::Low => issue_low_count += 1,
        }
    }
    let unresolved = items
        .iter()
        .filter(|item| {
            !matches!(
                item.status,
                WorkItemStatus::Verified | WorkItemStatus::Ignored | WorkItemStatus::Blocked
            )
        })
        .count() as u32;
    let new_count = items
        .iter()
        .filter(|item| matches!(item.status, WorkItemStatus::New))
        .count() as u32;
    let working_count = items
        .iter()
        .filter(|item| matches!(item.status, WorkItemStatus::Working))
        .count() as u32;
    let regressed_count = items
        .iter()
        .filter(|item| matches!(item.status, WorkItemStatus::Regressed))
        .count() as u32;
    let ignored_count = items
        .iter()
        .filter(|item| matches!(item.status, WorkItemStatus::Ignored))
        .count() as u32;
    let blocked_count = items
        .iter()
        .filter(|item| matches!(item.status, WorkItemStatus::Blocked))
        .count() as u32;
    let launch_blocker_count = items
        .iter()
        .filter(|item| {
            item.kind == WorkItemKind::Launch
                && !matches!(
                    item.status,
                    WorkItemStatus::Verified | WorkItemStatus::Ignored | WorkItemStatus::Blocked
                )
        })
        .count() as u32;
    let primary_action = queue
        .resume_now
        .first()
        .cloned()
        .or_else(|| queue.verify_now.first().cloned())
        .or_else(|| queue.fix_next.first().cloned())
        .or_else(|| queue.maintenance.first().cloned());
    let regressed_action = queue.resume_now.first().cloned();
    let working_action = queue.verify_now.first().cloned();
    let mut blocked_items = items
        .iter()
        .filter(|item| matches!(item.status, WorkItemStatus::Blocked))
        .cloned()
        .collect::<Vec<_>>();
    blocked_items.sort_by_key(work_item_sort_key);
    let blocked_action = blocked_items.first().cloned();
    let mut ignored_items = items
        .iter()
        .filter(|item| matches!(item.status, WorkItemStatus::Ignored))
        .cloned()
        .collect::<Vec<_>>();
    ignored_items.sort_by_key(work_item_sort_key);
    let ignored_action = ignored_items.first().cloned();
    let mut launch_blocker_items = items
        .iter()
        .filter(|item| {
            item.kind == WorkItemKind::Launch
                && !matches!(
                    item.status,
                    WorkItemStatus::Verified | WorkItemStatus::Ignored | WorkItemStatus::Blocked
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    launch_blocker_items.sort_by_key(work_item_sort_key);
    let launch_blocker_action = launch_blocker_items.first().cloned();
    let weekly_summary = if unresolved == 0
        && regressed_count == 0
        && working_count == 0
        && blocked_count == 0
    {
        None
    } else {
        let summary_target = if regressed_count > 0 {
            regressed_action.as_ref().or(primary_action.as_ref())
        } else if working_count > 0 {
            working_action.as_ref().or(primary_action.as_ref())
        } else if blocked_count > 0 {
            blocked_action.as_ref().or(primary_action.as_ref())
        } else {
            primary_action.as_ref()
        };
        Some(ProjectWorkItem {
            stable_key: "weekly-summary".to_string(),
            project_id: summary_target
                .map(|item| item.project_id)
                .unwrap_or_default(),
            environment_url: summary_target.and_then(|item| item.environment_url.clone()),
            kind: WorkItemKind::Web,
            status: if regressed_count > 0 {
                WorkItemStatus::Regressed
            } else if working_count > 0 {
                WorkItemStatus::Working
            } else if blocked_count > 0 {
                WorkItemStatus::Blocked
            } else {
                WorkItemStatus::New
            },
            severity: if regressed_count > 0 {
                Some(Severity::High)
            } else {
                Some(Severity::Medium)
            },
            title: if regressed_count > 0 {
                format!(
                    "{} issue{} came back this week",
                    regressed_count,
                    if regressed_count == 1 { "" } else { "s" }
                )
            } else if blocked_count > 0 && unresolved == 0 && working_count == 0 {
                format!(
                    "{} blocked item{} waiting on a decision",
                    blocked_count,
                    if blocked_count == 1 { "" } else { "s" }
                )
            } else {
                format!(
                    "{} open item{} still in play",
                    unresolved,
                    if unresolved == 1 { "" } else { "s" }
                )
            },
            summary: if blocked_count > 0
                && unresolved == 0
                && working_count == 0
                && regressed_count == 0
            {
                format!(
                    "{} blocked and {} ignored. Re-open the stuck work and decide the smallest next step.",
                    blocked_count, ignored_count
                )
            } else {
                format!(
                    "{} new, {} in progress, {} came back{} Start with the highest-impact item and verify it before moving on.",
                    new_count,
                    working_count,
                    regressed_count,
                    if blocked_count > 0 {
                        format!(", {} blocked.", blocked_count)
                    } else {
                        ".".to_string()
                    },
                )
            },
            category: Some("maintenance".to_string()),
            domain: None,
            package_name: None,
            target: summary_target
                .map(|item| item.target.clone())
                .unwrap_or_default(),
            first_seen_at: chrono::Utc::now().to_rfc3339(),
            last_seen_at: chrono::Utc::now().to_rfc3339(),
            last_verified_at: None,
            last_status_changed_at: chrono::Utc::now().to_rfc3339(),
            snooze_until: None,
            block_reason: None,
        })
    };

    ProjectWorkSummary {
        issue_count,
        issue_web_count,
        issue_code_count,
        issue_critical_count,
        issue_high_count,
        issue_medium_count,
        issue_low_count,
        unresolved_count: unresolved,
        new_count,
        working_count,
        regressed_count,
        ignored_count,
        blocked_count,
        launch_blocker_count,
        maintenance_count: queue.maintenance.len() as u32,
        primary_action,
        regressed_action,
        working_action,
        blocked_action,
        ignored_action,
        launch_blocker_action,
        weekly_summary,
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct WorkflowEventCue {
    pub key: &'static str,
    pub label: String,
    pub sentence: String,
}

#[cfg(test)]
#[tracing::instrument(skip(summary))]
pub(crate) fn build_workflow_event_cue(summary: &ProjectWorkSummary) -> Option<WorkflowEventCue> {
    if summary.regressed_count > 0 {
        let count = summary.regressed_count;
        return Some(WorkflowEventCue {
            key: "regressed",
            label: format!("{} regressed", count),
            sentence: format!(
                "{} issue{} came back and should be picked up next.",
                count,
                if count == 1 { "" } else { "s" }
            ),
        });
    }

    if summary.working_count > 0 {
        let count = summary.working_count;
        return Some(WorkflowEventCue {
            key: "working",
            label: format!("{} working", count),
            sentence: format!(
                "{} in-progress item{} ready to pick back up.",
                count,
                if count == 1 { " is" } else { "s are" }
            ),
        });
    }

    if summary.blocked_count > 0 {
        let count = summary.blocked_count;
        return Some(WorkflowEventCue {
            key: "blocked",
            label: format!("{} blocked", count),
            sentence: format!(
                "{} blocked item{} waiting on a decision.",
                count,
                if count == 1 { "" } else { "s" }
            ),
        });
    }

    if summary.launch_blocker_count > 0 {
        let count = summary.launch_blocker_count;
        return Some(WorkflowEventCue {
            key: "launch-blockers",
            label: format!(
                "{} launch blocker{}",
                count,
                if count == 1 { "" } else { "s" }
            ),
            sentence: format!(
                "{} launch blocker{} still open.",
                count,
                if count == 1 { " is" } else { "s are" }
            ),
        });
    }

    if summary.ignored_count > 0 {
        let count = summary.ignored_count;
        return Some(WorkflowEventCue {
            key: "ignored",
            label: format!("{} ignored", count),
            sentence: format!(
                "{} ignored item{} still parked.",
                count,
                if count == 1 { "" } else { "s" }
            ),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{build_project_work_queue, build_project_work_summary, build_workflow_event_cue};
    use crate::checks::Severity;
    use crate::db::{
        ProjectWorkItem, ProjectWorkSummary, WorkItemKind, WorkItemStatus, WorkItemTarget,
    };

    fn sample_work_item(
        project_id: i64,
        environment_url: Option<&str>,
        stable_key: &str,
        kind: WorkItemKind,
        status: WorkItemStatus,
    ) -> ProjectWorkItem {
        ProjectWorkItem {
            stable_key: stable_key.to_string(),
            project_id,
            environment_url: environment_url.map(|value| value.to_string()),
            kind,
            status,
            severity: Some(Severity::High),
            title: "Raw issue title".to_string(),
            summary: "Raw issue summary".to_string(),
            category: Some("security".to_string()),
            domain: Some("database".to_string()),
            package_name: None,
            target: WorkItemTarget {
                page: "issues".to_string(),
                project_id: Some(project_id),
                url: environment_url.map(|value| value.to_string()),
                item_id: Some("issue-1".to_string()),
                file_path: Some("/tmp/code-test/app/api/projects/route.ts".to_string()),
                ..WorkItemTarget::default()
            },
            first_seen_at: "2026-04-01T00:00:00Z".to_string(),
            last_seen_at: "2026-04-01T00:00:00Z".to_string(),
            last_verified_at: None,
            last_status_changed_at: "2026-04-01T00:00:00Z".to_string(),
            snooze_until: None,
            block_reason: None,
        }
    }

    #[test]
    fn build_project_work_queue_serves_complete_code_items_to_everyone() {
        let item = sample_work_item(
            1,
            Some("https://example.com"),
            "code:1:https://example.com:issue-1",
            WorkItemKind::Code,
            WorkItemStatus::New,
        );

        let queue = build_project_work_queue(std::slice::from_ref(&item), vec![]);
        let served = queue.fix_next.first().expect("queue item");

        assert_eq!(served.title, item.title);
        assert_eq!(served.target.item_id, item.target.item_id);
        assert_eq!(served.target.file_path, item.target.file_path);
    }

    #[test]
    fn build_project_work_queue_includes_blocked_items_in_maintenance() {
        let blocked = sample_work_item(
            1,
            Some("https://example.com"),
            "launch:https://example.com:ssl-expiry",
            WorkItemKind::Launch,
            WorkItemStatus::Blocked,
        );

        let queue = build_project_work_queue(std::slice::from_ref(&blocked), vec![]);
        let maintenance_item = queue.maintenance.first().expect("blocked maintenance item");

        assert_eq!(maintenance_item.stable_key, blocked.stable_key);
        assert!(matches!(maintenance_item.status, WorkItemStatus::Blocked));
    }

    #[test]
    fn build_project_work_summary_exposes_status_representative_actions() {
        let regressed = sample_work_item(
            1,
            Some("https://example.com"),
            "web:https://example.com:headers.csp",
            WorkItemKind::Web,
            WorkItemStatus::Regressed,
        );
        let working = sample_work_item(
            1,
            Some("https://example.com"),
            "launch:https://example.com:launch-item",
            WorkItemKind::Launch,
            WorkItemStatus::Working,
        );
        let blocked = sample_work_item(
            1,
            Some("https://example.com"),
            "code:1:https://example.com:issue-1",
            WorkItemKind::Code,
            WorkItemStatus::Blocked,
        );
        let ignored = sample_work_item(
            1,
            Some("https://example.com"),
            "update:1:npm:next",
            WorkItemKind::Update,
            WorkItemStatus::Ignored,
        );

        let items = vec![regressed.clone(), working.clone(), blocked, ignored];
        let queue = build_project_work_queue(&items, vec![]);
        let summary = build_project_work_summary(&items, &queue);

        assert_eq!(
            summary
                .regressed_action
                .expect("regressed action")
                .stable_key,
            regressed.stable_key
        );
        assert_eq!(
            summary.working_action.expect("working action").stable_key,
            working.stable_key
        );
        let blocked_action = summary.blocked_action.expect("blocked action");
        assert_eq!(blocked_action.title, "Raw issue title");
        assert_eq!(blocked_action.target.item_id.as_deref(), Some("issue-1"));
        let ignored_action = summary.ignored_action.expect("ignored action");
        assert_eq!(ignored_action.kind, WorkItemKind::Update);
        let weekly_summary = summary.weekly_summary.expect("weekly summary");
        assert!(matches!(weekly_summary.status, WorkItemStatus::Regressed));
    }

    #[test]
    fn build_project_work_summary_keeps_weekly_summary_for_blocked_items() {
        let blocked = sample_work_item(
            1,
            Some("https://example.com"),
            "launch:https://example.com:ssl-expiry",
            WorkItemKind::Launch,
            WorkItemStatus::Blocked,
        );

        let items = vec![blocked.clone()];
        let queue = build_project_work_queue(&items, vec![]);
        let summary = build_project_work_summary(&items, &queue);

        let weekly_summary = summary.weekly_summary.expect("weekly summary");
        assert!(matches!(weekly_summary.status, WorkItemStatus::Blocked));
        assert!(summary.launch_blocker_action.is_none());
        assert_eq!(
            summary.blocked_action.expect("blocked action").stable_key,
            blocked.stable_key
        );
    }

    #[test]
    fn build_workflow_event_cue_prefers_historical_resume_states() {
        let regressed_summary = ProjectWorkSummary {
            regressed_count: 2,
            working_count: 1,
            blocked_count: 1,
            ..ProjectWorkSummary::default()
        };
        let regressed_cue = build_workflow_event_cue(&regressed_summary).expect("regressed cue");
        assert_eq!(regressed_cue.key, "regressed");
        assert_eq!(regressed_cue.label, "2 regressed");

        let blocked_summary = ProjectWorkSummary {
            blocked_count: 1,
            launch_blocker_count: 1,
            ..ProjectWorkSummary::default()
        };
        let blocked_cue = build_workflow_event_cue(&blocked_summary).expect("blocked cue");
        assert_eq!(blocked_cue.key, "blocked");
        assert!(blocked_cue.sentence.contains("decision"));
    }
}
