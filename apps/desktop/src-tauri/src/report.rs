//! Self-contained HTML report generation and data aggregation.

use std::sync::Arc;

use crate::checks::Severity;
use crate::db::Database;
use crate::scoring::calculator::compute_current_score;

mod code_scan_summary;
mod deploys;
mod fetch_plan;
mod html;
mod integration_summaries;
mod types;

#[cfg(feature = "desktop")]
use code_scan_summary::build_code_scan_report_summary;
#[cfg(feature = "desktop")]
use deploys::build_deploys_summary;
#[cfg(test)]
use fetch_plan::required_report_integration_types;
#[cfg(feature = "desktop")]
use fetch_plan::{
    load_report_integrations, should_include_report_code_scan, should_load_detailed_web_issues,
};
pub use html::render_html;
#[cfg(test)]
pub(crate) use html::sanitize_logo_data_url;
#[cfg(feature = "desktop")]
use integration_summaries::{fetch_analytics_summary, fetch_uptime_summary};
pub use types::*;

fn format_report_timestamp(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| {
            dt.format("%B %d, %Y at %l:%M %p")
                .to_string()
                .trim()
                .to_string()
        })
        .unwrap_or_else(|_| value.to_string())
}

#[cfg(feature = "desktop")]
/// Build report data for a given project and period
#[tracing::instrument(skip(app, db, branding, sections, site_url), fields(project_id, period_days, report_title = %report_title))]
pub async fn aggregate_report(
    app: &tauri::AppHandle,
    db: &Arc<Database>,
    project_id: i64,
    site_url: &str,
    period_days: u32,
    branding: ReportBranding,
    report_title: String,
    sections: SectionConfig,
) -> Result<ReportData, String> {
    let now = chrono::Utc::now();
    let period_start = now - chrono::Duration::days(period_days as i64);
    let period_label = match period_days {
        7 => "Last 7 Days".into(),
        30 => "Last 30 Days".into(),
        90 => "Last Quarter".into(),
        _ => format!("Last {} Days", period_days),
    };

    let project = db
        .get_projects()?
        .into_iter()
        .find(|p| p.id == project_id)
        .ok_or("Project not found")?;

    let trend_points = db.get_score_trend_for_project(project_id, site_url, 50)?;

    let period_scans: Vec<_> = trend_points
        .iter()
        .filter(|p| p.timestamp >= period_start.format("%Y-%m-%dT%H:%M:%S").to_string())
        .collect();

    let previous_scans: Vec<_> = trend_points
        .iter()
        .filter(|p| {
            let prev_start = period_start - chrono::Duration::days(period_days as i64);
            p.timestamp >= prev_start.format("%Y-%m-%dT%H:%M:%S").to_string()
                && p.timestamp < period_start.format("%Y-%m-%dT%H:%M:%S").to_string()
        })
        .collect();

    let current_score = period_scans.last().map(|s| s.overall).unwrap_or(0);
    let previous_score = previous_scans.last().map(|s| s.overall);
    let trend = match previous_score {
        Some(prev) if current_score > prev => "up",
        Some(prev) if current_score < prev => "down",
        _ => "stable",
    };

    let latest_scan = db.get_scan_history_for_project(project_id, site_url, 1)?;
    let mut issues_critical = latest_scan
        .first()
        .map(|scan| scan.issues_critical)
        .unwrap_or(0);
    let mut issues_high = latest_scan
        .first()
        .map(|scan| scan.issues_high)
        .unwrap_or(0);
    let mut issues_medium = latest_scan
        .first()
        .map(|scan| scan.issues_medium)
        .unwrap_or(0);
    let mut issues_low = latest_scan.first().map(|scan| scan.issues_low).unwrap_or(0);
    let mut top_issues = Vec::new();

    if should_load_detailed_web_issues(&sections) {
        if let Some(latest) = latest_scan.first() {
            if let Ok(Some(detail)) = db.get_scan_detail(latest.id) {
                issues_critical = 0;
                issues_high = 0;
                issues_medium = 0;
                issues_low = 0;
                for issue in &detail.issues {
                    match issue.severity {
                        Severity::Critical => issues_critical += 1,
                        Severity::High => issues_high += 1,
                        Severity::Medium => issues_medium += 1,
                        Severity::Low => issues_low += 1,
                    }
                    if matches!(issue.severity, Severity::Critical | Severity::High)
                        && top_issues.len() < 10
                    {
                        top_issues.push(ReportIssue {
                            title: issue.title.clone(),
                            category: format!("{:?}", issue.category),
                            severity: issue.severity,
                            description: issue.description.clone(),
                        });
                    }
                }
            }
        }
    }

    let issues_total = latest_scan
        .first()
        .map(|scan| scan.issues_total)
        .unwrap_or(issues_critical + issues_high + issues_medium + issues_low);

    let now_ms = now.timestamp_millis();
    let site_score_groups = db.get_work_items_grouped(project_id, Some(site_url), now_ms)?;
    let site_score_snapshot = compute_current_score(&site_score_groups, now_ms);

    let categories = if let Some(latest) = period_scans.last() {
        let prev = previous_scans.last();
        vec![
            CategorySummary {
                name: "Security".into(),
                score: latest.security.unwrap_or(0),
                previous_score: prev.and_then(|p| p.security),
                issue_count: 0,
            },
            CategorySummary {
                name: "Performance".into(),
                score: latest.performance.unwrap_or(0),
                previous_score: prev.and_then(|p| p.performance),
                issue_count: 0,
            },
            CategorySummary {
                name: "SEO".into(),
                score: latest.seo.unwrap_or(0),
                previous_score: prev.and_then(|p| p.seo),
                issue_count: 0,
            },
            CategorySummary {
                name: "Accessibility".into(),
                score: latest.accessibility.unwrap_or(0),
                previous_score: prev.and_then(|p| p.accessibility),
                issue_count: 0,
            },
            CategorySummary {
                name: "Compliance".into(),
                score: latest.compliance.unwrap_or(0),
                previous_score: prev.and_then(|p| p.compliance),
                issue_count: 0,
            },
            CategorySummary {
                name: "Config".into(),
                score: latest.config.unwrap_or(0),
                previous_score: prev.and_then(|p| p.config),
                issue_count: 0,
            },
        ]
    } else {
        Vec::new()
    };

    let score_points: Vec<ScorePoint> = period_scans
        .iter()
        .map(|s| ScorePoint {
            date: s.timestamp.clone(),
            score: s.overall,
        })
        .collect();

    let code_scan = if should_include_report_code_scan(&sections) {
        build_code_scan_report_summary(db, project_id, site_url)?
    } else {
        None
    };

    let configs = load_report_integrations(app, db, project_id, &sections)?;
    let (analytics, uptime) = tokio::join!(
        fetch_analytics_summary(&configs, &sections, period_days),
        fetch_uptime_summary(&configs, &sections),
    );
    let deploys = build_deploys_summary(db, project_id, period_start, now, &sections)?;

    // Resolved count: compare issue IDs between oldest and newest scan in period
    let resolved_count = if period_scans.len() >= 2 {
        // Rough estimate: difference in issue count if it went down
        let first_issues = period_scans.first().map(|s| s.issues).unwrap_or(0);
        let last_issues = period_scans.last().map(|s| s.issues).unwrap_or(0);
        first_issues.saturating_sub(last_issues)
    } else {
        0
    };

    Ok(ReportData {
        site_url: site_url.to_string(),
        project_name: project.name.clone(),
        report_title: if report_title.is_empty() {
            "Site & Code Report".into()
        } else {
            report_title
        },
        sections,
        period_label,
        period_start: period_start.format("%B %d, %Y").to_string(),
        period_end: now.format("%B %d, %Y").to_string(),
        generated_at: now.format("%Y-%m-%d %H:%M").to_string(),
        site_score: SiteScoreSummary {
            current_score: site_score_snapshot.overall.round().clamp(0.0, 100.0) as u32,
            issues_total: (site_score_snapshot.critical_count
                + site_score_snapshot.high_count
                + site_score_snapshot.medium_count
                + site_score_snapshot.low_count) as u32,
            issues_critical: site_score_snapshot.critical_count as u32,
            issues_high: site_score_snapshot.high_count as u32,
            issues_medium: site_score_snapshot.medium_count as u32,
            issues_low: site_score_snapshot.low_count as u32,
        },
        health: HealthSummary {
            current_score,
            previous_score,
            trend: trend.to_string(),
            trend_points: score_points,
            issues_total,
            issues_critical,
            issues_high,
            issues_medium,
            issues_low,
        },
        categories,
        top_issues,
        resolved_count,
        latest_scan_date: period_scans.last().map(|s| {
            // Format timestamp nicely: "March 28, 2026 at 2:15 PM"
            chrono::NaiveDateTime::parse_from_str(
                &s.timestamp[..19.min(s.timestamp.len())],
                "%Y-%m-%dT%H:%M:%S",
            )
            .map(|dt| {
                dt.format("%B %d, %Y at %l:%M %p")
                    .to_string()
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|_| s.timestamp[..10.min(s.timestamp.len())].to_string())
        }),
        code_scan,
        analytics,
        uptime,
        deploys,
        branding,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::IntegrationType;

    fn sample_report_data() -> ReportData {
        ReportData {
            site_url: "https://example.com".to_string(),
            project_name: "Example".to_string(),
            report_title: "Example Report".to_string(),
            sections: SectionConfig::default(),
            period_label: "Last 30 Days".to_string(),
            period_start: "March 01, 2026".to_string(),
            period_end: "March 31, 2026".to_string(),
            generated_at: "2026-03-31 09:00".to_string(),
            site_score: SiteScoreSummary {
                current_score: 77,
                issues_total: 5,
                issues_critical: 1,
                issues_high: 2,
                issues_medium: 1,
                issues_low: 1,
            },
            health: HealthSummary {
                current_score: 82,
                previous_score: Some(79),
                trend: "up".to_string(),
                trend_points: vec![
                    ScorePoint {
                        date: "2026-03-01T00:00:00Z".to_string(),
                        score: 79,
                    },
                    ScorePoint {
                        date: "2026-03-31T00:00:00Z".to_string(),
                        score: 82,
                    },
                ],
                issues_total: 4,
                issues_critical: 1,
                issues_high: 1,
                issues_medium: 1,
                issues_low: 1,
            },
            categories: vec![],
            top_issues: vec![],
            resolved_count: 1,
            latest_scan_date: Some("March 31, 2026".to_string()),
            code_scan: None,
            analytics: None,
            uptime: None,
            deploys: None,
            branding: ReportBranding::default(),
        }
    }

    #[test]
    fn sanitize_logo_data_url_rejects_svg_and_accepts_png() {
        assert_eq!(
            sanitize_logo_data_url("data:image/png;base64,QUJDRA=="),
            Some("data:image/png;base64,QUJDRA==".to_string())
        );
        assert_eq!(
            sanitize_logo_data_url("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="),
            None
        );
    }

    #[test]
    fn render_html_uses_inline_logo_data_and_ignores_logo_path() {
        let mut data = sample_report_data();
        data.branding.logo_path = Some("/Users/dev/secret.png".to_string());
        data.branding.logo_data_url = Some("data:image/png;base64,QUJDRA==".to_string());

        let html = render_html(&data);

        assert!(html.contains("data:image/png;base64,QUJDRA=="));
        assert!(!html.contains("/Users/dev/secret.png"));
    }

    #[test]
    fn render_html_escapes_frontend_supplied_category_and_period_strings() {
        let mut data = sample_report_data();
        data.period_start = r#"<img src=x onerror=alert("period")>"#.to_string();
        data.period_end = r#"<script>alert("end")</script>"#.to_string();
        data.categories = vec![CategorySummary {
            name: r#"<svg onload=alert("cat")>"#.to_string(),
            score: 82,
            previous_score: Some(80),
            issue_count: 0,
        }];

        let html = render_html(&data);

        assert!(!html.contains(r#"<img src=x onerror=alert("period")>"#));
        assert!(!html.contains(r#"<script>alert("end")</script>"#));
        assert!(!html.contains(r#"<svg onload=alert("cat")>"#));
        assert!(html.contains("&lt;img src=x onerror=alert(&quot;period&quot;)&gt;"));
        assert!(html.contains("&lt;script&gt;alert(&quot;end&quot;)&lt;/script&gt;"));
        assert!(html.contains("&lt;svg onload=alert(&quot;cat&quot;)&gt;"));
    }

    #[test]
    fn report_fetch_plan_skips_code_scan_when_section_is_disabled() {
        let sections = SectionConfig {
            code_scan: false,
            ..SectionConfig::default()
        };

        assert!(!should_include_report_code_scan(&sections));
    }

    #[test]
    fn report_fetch_plan_only_requires_enabled_integrations() {
        let sections = SectionConfig {
            analytics: false,
            uptime: true,
            ..SectionConfig::default()
        };

        assert_eq!(
            required_report_integration_types(&sections),
            vec![IntegrationType::UptimeRobot]
        );
    }

    #[test]
    fn report_fetch_plan_skips_detailed_web_issues_when_issue_sections_are_disabled() {
        let sections = SectionConfig {
            top_issues: false,
            recommendations: false,
            ..SectionConfig::default()
        };

        assert!(!should_load_detailed_web_issues(&sections));
    }
}
