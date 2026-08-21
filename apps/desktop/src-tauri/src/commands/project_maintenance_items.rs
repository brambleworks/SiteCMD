use crate::checks::Severity;
use crate::core::scanner::ScanResult;
use crate::db::{
    CodeScanSummary, Database, ProjectMonitoringSignals, ProjectWorkItem, WorkItemKind,
    WorkItemStatus,
};

use super::project_work_items::{build_work_target, parse_timestamp_millis};

fn modified_ms_to_rfc3339(value: u64) -> Result<String, String> {
    let value = i64::try_from(value)
        .map_err(|_| "watched-file modification time is outside the supported range".to_string())?;
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(value)
        .map(|timestamp| timestamp.to_rfc3339())
        .ok_or_else(|| "watched-file modification time is invalid".to_string())
}

fn optional_timestamp_millis(value: Option<&str>, field: &str) -> Result<Option<u64>, String> {
    value
        .map(|timestamp| {
            parse_timestamp_millis(timestamp)
                .ok_or_else(|| format!("{field} contains an invalid RFC 3339 timestamp"))
        })
        .transpose()
}

fn build_watch_file_maintenance_items(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    latest_site_scan: Option<&ScanResult>,
    updates_refreshed_at: Option<&str>,
) -> Result<Vec<ProjectWorkItem>, String> {
    let Some(url) = environment_url else {
        return Ok(Vec::new());
    };
    let Some(project_path) = db
        .get_project_path_result(project_id)
        .map_err(|error| format!("could not read linked project path: {error}"))?
    else {
        return Ok(Vec::new());
    };

    let latest_site_scan_ms = optional_timestamp_millis(
        latest_site_scan.map(|scan| scan.timestamp.as_str()),
        "latest Web Scan",
    )?;
    let updates_refreshed_ms = optional_timestamp_millis(updates_refreshed_at, "Updates refresh")?;
    let requests = vec![super::desktop::DesktopWatchRequest {
        project_id,
        project_path,
        primary_url: Some(url.to_string()),
    }];

    super::desktop::inspect_watch_files(&requests)
        .into_iter()
        .filter_map(|signal| {
            let reference_ms = match signal.page.as_str() {
                "updates" => updates_refreshed_ms,
                "search-console" | "security" => latest_site_scan_ms,
                _ => None,
            };

            let should_include = match signal.page.as_str() {
                "updates" => reference_ms.map(|millis| signal.modified_ms > millis).unwrap_or(true),
                "search-console" | "security" => {
                    reference_ms.map(|millis| signal.modified_ms > millis).unwrap_or(false)
                }
                _ => false,
            };

            if !should_include {
                return None;
            }

            let modified_at = match modified_ms_to_rfc3339(signal.modified_ms) {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            let stable_suffix = signal.relative_path.replace('\\', "/");
            let (kind, severity, reason, title, summary) = match signal.page.as_str() {
                "updates" => (
                    WorkItemKind::Update,
                    Severity::Medium,
                    "changed-dependencies",
                    "Dependency files changed since SiteCMD last checked".to_string(),
                    format!(
                        "{} changed after the last Updates refresh. Open Updates again so SiteCMD can recalculate what changed.",
                        signal.relative_path
                    ),
                ),
                "search-console" => (
                    WorkItemKind::Web,
                    Severity::Medium,
                    "changed-search-file",
                    format!("{} changed after the last Web Scan", signal.title),
                    format!(
                        "{} {} Open Search & SEO again before you trust the current picture.",
                        signal.relative_path, signal.detail
                    ),
                ),
                "security" => (
                    WorkItemKind::Web,
                    Severity::High,
                    "changed-security-file",
                    format!("{} changed after the last Web Scan", signal.title),
                    format!(
                        "{} {} Open Security again and verify the current Web Scan results before you rely on this release.",
                        signal.relative_path, signal.detail
                    ),
                ),
                _ => return None,
            };

            Some(Ok(ProjectWorkItem {
                stable_key: format!(
                    "maintenance:{}:watch:{}:{}",
                    url.trim_end_matches('/'),
                    signal.kind,
                    stable_suffix,
                ),
                project_id,
                environment_url: Some(url.to_string()),
                kind,
                status: WorkItemStatus::New,
                severity: Some(severity),
                title,
                summary,
                category: Some("maintenance".to_string()),
                domain: None,
                package_name: None,
                target: build_work_target(
                    &signal.page,
                    project_id,
                    Some(url),
                    None,
                    signal.focus.as_deref(),
                    Some(&signal.absolute_path),
                    reason,
                    None,
                ),
                first_seen_at: modified_at.clone(),
                last_seen_at: modified_at.clone(),
                last_verified_at: None,
                last_status_changed_at: modified_at,
                snooze_until: None,
                block_reason: None,
            }))
        })
        .collect()
}

pub(crate) fn build_project_maintenance_items(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    latest_site_scan: Option<&ScanResult>,
    code_scan_summary: Option<&CodeScanSummary>,
    updates_refreshed_at: Option<&str>,
    monitoring: &ProjectMonitoringSignals,
) -> Result<Vec<ProjectWorkItem>, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut items = Vec::new();

    let Some(url) = environment_url else {
        return Ok(items);
    };

    let deploy_events = db
        .get_events(
            project_id,
            (chrono::Utc::now() - chrono::Duration::days(30)).timestamp_millis(),
            chrono::Utc::now().timestamp_millis(),
            Some(&["deploy".to_string()]),
            None,
            None,
            None,
        )
        .map_err(|error| format!("could not load deploy events for maintenance issues: {error}"))?;
    let latest_deploy = deploy_events.first().cloned();
    let latest_scan_timestamp = latest_site_scan
        .map(|scan| {
            chrono::DateTime::parse_from_rfc3339(&scan.timestamp)
                .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
                .map_err(|error| format!("latest Web Scan has an invalid timestamp: {error}"))
        })
        .transpose()?;

    if latest_site_scan.is_none() {
        items.push(ProjectWorkItem {
            stable_key: format!("maintenance:{}:first-web-scan", url.trim_end_matches('/')),
            project_id,
            environment_url: Some(url.to_string()),
            kind: WorkItemKind::Web,
            status: WorkItemStatus::New,
            severity: Some(Severity::Medium),
            title: "Run your first Web Scan".to_string(),
            summary: "You do not have a Web Scan yet. Run one so you have a current list of issues to work from.".to_string(),
            category: Some("maintenance".to_string()),
            domain: None,
            package_name: None,
            target: build_work_target("issues", project_id, Some(url), None, None, None, "no-first-scan", Some("site")),
            first_seen_at: now.clone(),
            last_seen_at: now.clone(),
            last_verified_at: None,
            last_status_changed_at: now.clone(),
            snooze_until: None,
            block_reason: None,
        });
    } else if let Some(parsed) = latest_scan_timestamp {
        let age = chrono::Utc::now().signed_duration_since(parsed);
        if age > chrono::Duration::days(7) {
            items.push(ProjectWorkItem {
                stable_key: format!("maintenance:{}:stale-web-scan", url.trim_end_matches('/')),
                project_id,
                environment_url: Some(url.to_string()),
                kind: WorkItemKind::Web,
                status: WorkItemStatus::New,
                severity: Some(Severity::Medium),
                title: "Web Scan is getting stale".to_string(),
                summary: format!(
                    "Last Web Scan ran {} days ago. Run it again before you rely on those results.",
                    age.num_days()
                ),
                category: Some("maintenance".to_string()),
                domain: None,
                package_name: None,
                target: build_work_target(
                    "issues",
                    project_id,
                    Some(url),
                    None,
                    None,
                    None,
                    "stale-web-scan",
                    Some("site"),
                ),
                first_seen_at: now.clone(),
                last_seen_at: now.clone(),
                last_verified_at: None,
                last_status_changed_at: now.clone(),
                snooze_until: None,
                block_reason: None,
            });
        }
    }

    if let (Some(deploy), Some(scan_ts)) = (latest_deploy.as_ref(), latest_scan_timestamp) {
        let deploy_ts = chrono::DateTime::from_timestamp_millis(deploy.occurred_at_ms)
            .ok_or_else(|| format!("deploy event {} has an invalid timestamp", deploy.id))?;
        if deploy_ts > scan_ts {
            let age = chrono::Utc::now().signed_duration_since(deploy_ts);
            let age_label = if age < chrono::Duration::hours(1) {
                "less than an hour ago".to_string()
            } else if age < chrono::Duration::days(1) {
                format!(
                    "{} hour{} ago",
                    age.num_hours(),
                    if age.num_hours() == 1 { "" } else { "s" }
                )
            } else {
                format!(
                    "{} day{} ago",
                    age.num_days(),
                    if age.num_days() == 1 { "" } else { "s" }
                )
            };
            items.push(ProjectWorkItem {
                stable_key: format!(
                    "maintenance:{}:scan-after-deploy",
                    url.trim_end_matches('/')
                ),
                project_id,
                environment_url: Some(url.to_string()),
                kind: WorkItemKind::Web,
                status: WorkItemStatus::New,
                severity: Some(Severity::Medium),
                title: "Re-run Web Scan after deploy".to_string(),
                summary: format!(
                    "\"{}\" deployed {}. Re-run Web Scan so the results match the current release.",
                    deploy.title, age_label
                ),
                category: Some("maintenance".to_string()),
                domain: None,
                package_name: None,
                target: build_work_target(
                    "issues",
                    project_id,
                    Some(url),
                    None,
                    None,
                    None,
                    "scan-after-deploy",
                    Some("site"),
                ),
                first_seen_at: deploy_ts.to_rfc3339(),
                last_seen_at: deploy_ts.to_rfc3339(),
                last_verified_at: None,
                last_status_changed_at: deploy_ts.to_rfc3339(),
                snooze_until: None,
                block_reason: None,
            });
        }
    }

    if let Some(last_scan) = latest_site_scan {
        let recent_scans = db
            .get_scan_history_for_project(project_id, url, 6)
            .map_err(|error| {
                format!("could not load scan history for maintenance issues: {error}")
            })?;
        if let Some(correlation) =
            crate::core::event_correlations::find_correlations(&deploy_events, &recent_scans)
                .into_iter()
                .find(|correlation| {
                    correlation.correlation_type == "deploy_to_regression"
                        && correlation.target_timestamp.as_deref()
                            == Some(last_scan.timestamp.as_str())
                })
        {
            let high_confidence = correlation.confidence == "high";
            items.push(ProjectWorkItem {
                stable_key: format!("maintenance:{}:deploy-regression", url.trim_end_matches('/')),
                project_id,
                environment_url: Some(url.to_string()),
                kind: WorkItemKind::Web,
                status: WorkItemStatus::Regressed,
                severity: Some(if high_confidence { Severity::High } else { Severity::Medium }),
                title: if high_confidence {
                    "Latest deploy lines up with a new regression".to_string()
                } else {
                    "A recent deploy may explain the score drop".to_string()
                },
                summary: format!(
                    "{} Review the deploy and compare it with the current Web Scan before you rely on this release.",
                    correlation.description
                ),
                category: Some("deploys".to_string()),
                domain: None,
                package_name: None,
                target: build_work_target(
                    "deploys",
                    project_id,
                    Some(url),
                    None,
                    None,
                    None,
                    "deploy-regression",
                    None,
                ),
                first_seen_at: correlation.source_timestamp.clone(),
                last_seen_at: correlation
                    .target_timestamp
                    .unwrap_or_else(|| correlation.source_timestamp.clone()),
                last_verified_at: None,
                last_status_changed_at: correlation.source_timestamp,
                snooze_until: None,
                block_reason: None,
            });
        }
    }

    if let Some(summary) = code_scan_summary {
        let parsed = chrono::DateTime::parse_from_rfc3339(&summary.checked_at)
            .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
            .map_err(|error| format!("latest Code Scan has an invalid timestamp: {error}"))?;
        let age = chrono::Utc::now().signed_duration_since(parsed);
        if age > chrono::Duration::days(7) {
            items.push(ProjectWorkItem {
                    stable_key: format!(
                        "maintenance:{}:stale-code-scan",
                        url.trim_end_matches('/')
                    ),
                    project_id,
                    environment_url: Some(url.to_string()),
                    kind: WorkItemKind::Code,
                    status: WorkItemStatus::New,
                    severity: Some(Severity::Medium),
                    title: "Code Scan is getting stale".to_string(),
                    summary: format!(
                        "The last Code Scan ran {} days ago. Run it again after the recent code changes.",
                        age.num_days()
                    ),
                    category: Some("maintenance".to_string()),
                    domain: summary.top_domain.map(|value| value.to_string()),
                    package_name: None,
                    target: build_work_target(
                        "issues",
                        project_id,
                        Some(url),
                        None,
                        summary
                            .top_domain
                            .as_ref()
                            .map(crate::core::code_scan::CodeScanDomain::as_str),
                        None,
                        "stale-code-scan",
                        Some("code"),
                    ),
                    first_seen_at: now.clone(),
                    last_seen_at: now.clone(),
                    last_verified_at: None,
                    last_status_changed_at: now.clone(),
                    snooze_until: None,
                    block_reason: None,
            });
        }
    }

    if let Some(search_regression) = &monitoring.search_regression {
        items.push(ProjectWorkItem {
            stable_key: format!("maintenance:{}:search-regression", url.trim_end_matches('/')),
            project_id,
            environment_url: Some(url.to_string()),
            kind: WorkItemKind::Web,
            status: WorkItemStatus::Regressed,
            severity: Some(Severity::High),
            title: format!("Search clicks down {}%", search_regression.delta_pct.abs()),
            summary: format!("{} is trending down over the last week. Open Search & SEO to see what changed before the drop gets worse.", search_regression.source),
            category: Some("search".to_string()),
            domain: None,
            package_name: None,
            target: build_work_target("search-console", project_id, Some(url), search_regression.item_id.as_deref(), search_regression.focus.as_deref(), None, "search-regression", Some("site")),
            first_seen_at: now.clone(),
            last_seen_at: now.clone(),
            last_verified_at: None,
            last_status_changed_at: now.clone(),
            snooze_until: None,
            block_reason: None,
        });
    }

    if monitoring.integration_failure_count > 0 || monitoring.stale_integration_count > 0 {
        items.push(ProjectWorkItem {
            stable_key: format!("maintenance:{}:integrations", url.trim_end_matches('/')),
            project_id,
            environment_url: Some(url.to_string()),
            kind: WorkItemKind::Update,
            status: if monitoring.integration_failure_count > 0 {
                WorkItemStatus::Regressed
            } else {
                WorkItemStatus::New
            },
            severity: Some(if monitoring.integration_failure_count > 0 {
                Severity::High
            } else {
                Severity::Medium
            }),
            title: "Connected services need a refresh".to_string(),
            summary: if monitoring.integration_failure_count > 0 {
                format!(
                    "SiteCMD is missing fresh data from {} failed integration sync{} and {} stale signal{}.",
                    monitoring.integration_failure_count,
                    if monitoring.integration_failure_count == 1 {
                        ""
                    } else {
                        "s"
                    },
                    monitoring.stale_integration_count,
                    if monitoring.stale_integration_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            } else {
                format!(
                    "SiteCMD has not heard from {} integration signal{} in a while. Refresh them so the project view stays current.",
                    monitoring.stale_integration_count,
                    if monitoring.stale_integration_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            },
            category: Some("integrations".to_string()),
            domain: None,
            package_name: None,
            target: build_work_target(
                "integrations",
                project_id,
                Some(url),
                None,
                None,
                None,
                "integration-signals",
                None,
            ),
            first_seen_at: now.clone(),
            last_seen_at: now.clone(),
            last_verified_at: None,
            last_status_changed_at: now.clone(),
            snooze_until: None,
            block_reason: None,
        });
    }

    items.extend(build_watch_file_maintenance_items(
        db,
        project_id,
        environment_url,
        latest_site_scan,
        updates_refreshed_at,
    )?);

    Ok(items)
}

#[cfg(test)]
mod tests;
