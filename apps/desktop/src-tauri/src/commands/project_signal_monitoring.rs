use tauri::AppHandle;

use crate::db::{Database, ProjectMonitoringSignals, SearchRegressionSignal};

pub(crate) fn integration_type_name(config: &crate::integrations::IntegrationConfig) -> String {
    serde_json::to_string(&config.integration_type)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

/// Read enabled integration names from saved configuration without live fetches.
pub(crate) fn enabled_integration_names(db: &Database, project_id: i64) -> Vec<String> {
    db.get_integrations(project_id)
        .unwrap_or_default()
        .iter()
        .filter(|config| config.enabled)
        .map(integration_type_name)
        .collect()
}

fn is_monitored_integration_type(integration_type: &crate::integrations::IntegrationType) -> bool {
    matches!(
        integration_type,
        crate::integrations::IntegrationType::Plausible
            | crate::integrations::IntegrationType::GoogleAnalytics
            | crate::integrations::IntegrationType::Cloudflare
            | crate::integrations::IntegrationType::UptimeRobot
            | crate::integrations::IntegrationType::GoogleSearchConsole
            | crate::integrations::IntegrationType::BingWebmaster
    )
}

pub(crate) fn take_monitored_integrations(enabled_integrations: &[String]) -> Vec<String> {
    enabled_integrations
        .iter()
        .filter_map(|integration_type| {
            let parsed = integration_type
                .parse::<crate::integrations::IntegrationType>()
                .ok()?;
            if !is_monitored_integration_type(&parsed) {
                return None;
            }
            Some(integration_type.clone())
        })
        .collect()
}

fn get_integration_signal(analytics: &serde_json::Value, integration_type: &str) -> (bool, bool) {
    match integration_type {
        "plausible" => (
            analytics.get("plausible").is_some(),
            analytics.get("plausible_error").is_some(),
        ),
        "googleanalytics" => (
            analytics.get("google_analytics").is_some(),
            analytics.get("google_analytics_error").is_some(),
        ),
        "cloudflare" => (
            analytics.get("cloudflare").is_some(),
            analytics.get("cloudflare_error").is_some(),
        ),
        "uptimerobot" => (
            analytics.get("uptimerobot").is_some(),
            analytics.get("uptimerobot_error").is_some(),
        ),
        "googlesearchconsole" => (
            analytics.get("search_console").is_some(),
            analytics.get("search_console_error").is_some(),
        ),
        "bingwebmaster" => (
            analytics.get("bing").is_some(),
            analytics.get("bing_error").is_some(),
        ),
        _ => (false, false),
    }
}

fn detect_click_drop(points: &[serde_json::Value]) -> Option<i32> {
    if points.len() < 14 {
        return None;
    }

    let previous: i64 = points
        .iter()
        .skip(points.len().saturating_sub(14))
        .take(7)
        .filter_map(|point| point.get("clicks")?.as_i64())
        .sum();
    let recent: i64 = points
        .iter()
        .skip(points.len().saturating_sub(7))
        .filter_map(|point| point.get("clicks")?.as_i64())
        .sum();

    if previous < 20 {
        return None;
    }

    let delta_pct = (((recent - previous) as f64 / previous as f64) * 100.0).round() as i32;
    (delta_pct <= -25).then_some(delta_pct)
}

fn matches_seo_focus(check_id: &str, title: &str, focus: &str) -> bool {
    let haystack = format!("{} {}", check_id, title).to_lowercase();
    let patterns: &[&str] = match focus {
        "seo.noindex" => &["noindex", "indexability"],
        "seo.robots" => &["robots", "robots_txt"],
        "seo.sitemap" => &["sitemap"],
        "seo.canonical" => &["canonical"],
        "seo.descriptions" => &["meta_description", "meta-description", "description"],
        "seo.titles" => &["title"],
        "seo.structured_data" => &["structured", "schema", "json_ld", "json-ld"],
        _ => &[focus],
    };
    patterns
        .iter()
        .any(|pattern| haystack.contains(&pattern.to_lowercase()))
}

fn matches_security_focus(check_id: &str, title: &str, focus: &str) -> bool {
    let haystack = format!("{} {}", check_id, title).to_lowercase();
    let patterns: &[&str] = match focus {
        "sec.https" => &["mixed_content", "https_enforcement", "https"],
        "sec.ssl_expiry" => &["ssl.validity", "ssl"],
        "sec.headers" => &["headers.csp", "csp", "content security policy"],
        "sec.hsts" => &["headers.hsts", "hsts"],
        "sec.exposed_files" => &["exposed", "dotfile", ".env", ".git"],
        _ => &[focus],
    };
    patterns
        .iter()
        .any(|pattern| haystack.contains(&pattern.to_lowercase()))
}

pub(crate) fn infer_security_target(
    db: &Database,
    project_id: i64,
    url: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let history = db.get_scan_history_for_project(project_id, url, 1)?;
    let Some(latest_scan) = history.first() else {
        return Ok((None, None));
    };
    let Some(detail) = db.get_scan_detail(latest_scan.id)? else {
        return Ok((None, None));
    };

    let mut failing_security_issues: Vec<_> = detail
        .issues
        .iter()
        .filter(|issue| {
            issue.category == crate::checks::ScanCategory::Security
                && issue.status != crate::checks::CheckStatus::Pass
        })
        .collect();

    let ordered_focuses = [
        "sec.ssl_expiry",
        "sec.headers",
        "sec.hsts",
        "sec.exposed_files",
        "sec.https",
    ];
    for focus in ordered_focuses {
        if let Some(issue) = failing_security_issues
            .iter()
            .find(|issue| matches_security_focus(&issue.check_id, &issue.title, focus))
        {
            return Ok((Some(focus.to_string()), Some(issue.check_id.clone())));
        }
    }

    failing_security_issues.sort_by_key(|issue| issue.severity.sort_rank());

    Ok((
        None,
        failing_security_issues
            .first()
            .map(|issue| issue.check_id.clone()),
    ))
}

fn infer_search_target(
    db: &Database,
    project_id: i64,
    url: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let history = db.get_scan_history_for_project(project_id, url, 1)?;
    let Some(latest_scan) = history.first() else {
        return Ok((None, None));
    };
    let Some(detail) = db.get_scan_detail(latest_scan.id)? else {
        return Ok((None, None));
    };

    let failing_seo_issues: Vec<_> = detail
        .issues
        .iter()
        .filter(|issue| {
            issue.category == crate::checks::ScanCategory::Seo
                && issue.status != crate::checks::CheckStatus::Pass
        })
        .collect();

    let ordered_focuses = [
        "seo.noindex",
        "seo.robots",
        "seo.sitemap",
        "seo.canonical",
        "seo.descriptions",
        "seo.titles",
        "seo.structured_data",
    ];
    for focus in ordered_focuses {
        if let Some(issue) = failing_seo_issues
            .iter()
            .find(|issue| matches_seo_focus(&issue.check_id, &issue.title, focus))
        {
            return Ok((Some(focus.to_string()), Some(issue.check_id.clone())));
        }
    }

    Ok((
        None,
        failing_seo_issues
            .first()
            .map(|issue| issue.check_id.clone()),
    ))
}

pub(crate) async fn build_project_monitoring_signals(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Result<ProjectMonitoringSignals, String> {
    let configs = db.get_integrations(project_id)?;

    let enabled_integrations: Vec<String> = configs
        .iter()
        .filter(|config| config.enabled)
        .map(integration_type_name)
        .collect();
    let relevant_integrations = take_monitored_integrations(&enabled_integrations);

    if relevant_integrations.is_empty() {
        return Ok(ProjectMonitoringSignals {
            enabled_integrations,
            ..ProjectMonitoringSignals::default()
        });
    }

    let analytics =
        super::integrations::fetch_analytics_internal(app, db, project_id, "28d", environment_url)
            .await
            .unwrap_or_else(|_| serde_json::json!({}));

    let mut integration_failure_count = 0;
    let mut stale_integration_count = 0;
    for integration_type in &relevant_integrations {
        let (has_data, has_error) = get_integration_signal(&analytics, integration_type);
        if has_error {
            integration_failure_count += 1;
        } else if !has_data {
            stale_integration_count += 1;
        }
    }

    let mut candidates = Vec::new();
    if let Some(points) = analytics
        .get("search_console")
        .and_then(|value| value.get("daily"))
        .and_then(|value| value.as_array())
    {
        if let Some(delta_pct) = detect_click_drop(points) {
            candidates.push(SearchRegressionSignal {
                source: "Google Search Console".to_string(),
                delta_pct,
                focus: None,
                item_id: None,
            });
        }
    }
    if let Some(points) = analytics
        .get("bing")
        .and_then(|value| value.get("daily_stats"))
        .and_then(|value| value.as_array())
    {
        if let Some(delta_pct) = detect_click_drop(points) {
            candidates.push(SearchRegressionSignal {
                source: "Bing Webmaster".to_string(),
                delta_pct,
                focus: None,
                item_id: None,
            });
        }
    }

    let mut search_regression = candidates
        .into_iter()
        .min_by_key(|candidate| candidate.delta_pct);
    if let (Some(url), Some(signal)) = (environment_url, search_regression.as_mut()) {
        let (focus, item_id) = infer_search_target(db, project_id, url).unwrap_or((None, None));
        signal.focus = focus;
        signal.item_id = item_id;
    }

    Ok(ProjectMonitoringSignals {
        enabled_integrations,
        integration_failure_count,
        stale_integration_count,
        search_regression,
    })
}
