use super::super::{ReportData, ReportIssue};
use super::utils::html_escape;
use crate::checks::Severity;

fn combined_recommendation_issues(data: &ReportData) -> Vec<ReportIssue> {
    let mut items = data.top_issues.clone();
    if let Some(code_scan) = data.code_scan.as_ref() {
        items.extend(code_scan.top_issues.iter().map(|issue| ReportIssue {
            title: issue.title.clone(),
            category: format!("Code Scan · {}", issue.category),
            severity: issue.severity,
            description: issue.description.clone(),
        }));
    }
    items
}

pub(super) fn render_analytics_section(data: &ReportData, pc: &str, visitor_svg: &str) -> String {
    if !data.sections.analytics {
        return String::new();
    }
    let Some(a) = data.analytics.as_ref() else {
        return String::new();
    };

    let top_pages_rows: String = a
        .top_pages
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, p)| {
            format!(
                r#"<tr>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;color:#6b7280">{}</td>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px">{}</td>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;text-align:right;font-variant-numeric:tabular-nums">{}</td>
            </tr>"#,
                i + 1,
                html_escape(&p.page),
                p.visitors,
            )
        })
        .collect();

    let top_sources_rows: String = a
        .top_sources
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, s)| {
            format!(
                r#"<tr>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;color:#6b7280">{}</td>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px">{}</td>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;text-align:right;font-variant-numeric:tabular-nums">{}</td>
            </tr>"#,
                i + 1,
                html_escape(&s.source),
                s.visitors,
            )
        })
        .collect();

    // Delta helper
    let fmt_count_delta = |current: u64, prev: Option<u64>| -> String {
        match prev {
            Some(pv) if pv > 0 => {
                let pct = ((current as f64 - pv as f64) / pv as f64 * 100.0) as i32;
                let (arrow, color) = if pct >= 0 {
                    ("↑", "#10b981")
                } else {
                    ("↓", "#ef4444")
                };
                format!(
                    r#"<div style="font-size:11px;color:{color};margin-top:2px;font-weight:600">{arrow} {pct}% vs prior period</div>"#
                )
            }
            _ => String::new(),
        }
    };
    let fmt_float_delta = |current: f64,
                           prev: Option<f64>,
                           unit: &str,
                           lower_is_better: bool|
     -> String {
        match prev {
            Some(pv) if pv > 0.0 => {
                let diff = current - pv;
                if diff.abs() < 0.5 {
                    return String::new();
                }
                let improved = if lower_is_better {
                    diff < 0.0
                } else {
                    diff > 0.0
                };
                let (arrow, color) = if improved {
                    ("↑", "#10b981")
                } else {
                    ("↓", "#ef4444")
                };
                format!(
                    r#"<div style="font-size:11px;color:{color};margin-top:2px;font-weight:600">{arrow} {:.0}{unit} vs prior</div>"#,
                    diff.abs()
                )
            }
            _ => String::new(),
        }
    };

    let visitors_delta = fmt_count_delta(a.visitors, a.prev_visitors);
    let pageviews_delta = fmt_count_delta(a.pageviews, a.prev_pageviews);
    let bounce_delta = fmt_float_delta(a.bounce_rate, a.prev_bounce_rate, "pp", true);
    let duration_delta = fmt_float_delta(a.visit_duration, a.prev_visit_duration, "s", false);

    format!(
        r##"
    <div style="page-break-before:always"></div>
    <h2 style="font-size:20px;margin:32px 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">Analytics Overview</h2>

    <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:24px">
      <div style="background:#f9fafb;border-radius:8px;padding:16px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;margin-bottom:4px">Visitors</div>
        <div style="font-size:24px;font-weight:700;font-variant-numeric:tabular-nums">{visitors}</div>
        {visitors_delta}
      </div>
      <div style="background:#f9fafb;border-radius:8px;padding:16px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;margin-bottom:4px">Pageviews</div>
        <div style="font-size:24px;font-weight:700;font-variant-numeric:tabular-nums">{pageviews}</div>
        {pageviews_delta}
      </div>
      <div style="background:#f9fafb;border-radius:8px;padding:16px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;margin-bottom:4px">Bounce Rate</div>
        <div style="font-size:24px;font-weight:700;font-variant-numeric:tabular-nums">{bounce:.0}%</div>
        {bounce_delta}
      </div>
      <div style="background:#f9fafb;border-radius:8px;padding:16px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;margin-bottom:4px">Avg Duration</div>
        <div style="font-size:24px;font-weight:700;font-variant-numeric:tabular-nums">{duration:.0}s</div>
        {duration_delta}
      </div>
    </div>

    <div style="margin-bottom:24px">{visitor_svg}</div>

    <div style="display:grid;grid-template-columns:1fr 1fr;gap:24px">
      <div>
        <h3 style="font-size:14px;font-weight:600;margin-bottom:8px">Top Pages</h3>
        <table style="width:100%;border-collapse:collapse">
          <thead><tr style="background:#f9fafb">
            <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">#</th>
            <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Page</th>
            <th style="padding:6px 12px;text-align:right;font-size:10px;text-transform:uppercase;color:#6b7280">Visitors</th>
          </tr></thead>
          <tbody>{top_pages_rows}</tbody>
        </table>
      </div>
      <div>
        <h3 style="font-size:14px;font-weight:600;margin-bottom:8px">Traffic Sources</h3>
        <table style="width:100%;border-collapse:collapse">
          <thead><tr style="background:#f9fafb">
            <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">#</th>
            <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Source</th>
            <th style="padding:6px 12px;text-align:right;font-size:10px;text-transform:uppercase;color:#6b7280">Visitors</th>
          </tr></thead>
          <tbody>{top_sources_rows}</tbody>
        </table>
      </div>
    </div>
"##,
        visitors = a.visitors,
        pageviews = a.pageviews,
        bounce = a.bounce_rate,
        duration = a.visit_duration,
    )
}

pub(super) fn render_uptime_section(data: &ReportData, pc: &str) -> String {
    if !data.sections.uptime {
        return String::new();
    }
    let Some(u) = data.uptime.as_ref() else {
        return String::new();
    };

    format!(
        r#"
    <div style="page-break-before:always"></div>
    <h2 style="font-size:20px;margin:32px 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">Uptime & Performance</h2>
    <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-bottom:24px">
      <div style="background:#f9fafb;border-radius:8px;padding:16px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;margin-bottom:4px">Uptime</div>
        <div style="font-size:28px;font-weight:700;color:{uptime_color}">{uptime:.2}%</div>
      </div>
      <div style="background:#f9fafb;border-radius:8px;padding:16px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;margin-bottom:4px">Incidents</div>
        <div style="font-size:28px;font-weight:700">{incidents}</div>
      </div>
      <div style="background:#f9fafb;border-radius:8px;padding:16px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;margin-bottom:4px">Avg Response</div>
        <div style="font-size:28px;font-weight:700">{response}ms</div>
      </div>
    </div>"#,
        uptime = u.uptime_pct,
        uptime_color = if u.uptime_pct >= 99.9 {
            "#10b981"
        } else if u.uptime_pct >= 99.0 {
            "#f59e0b"
        } else {
            "#ef4444"
        },
        incidents = u.incidents,
        response = u.avg_response_ms,
    )
}

pub(super) fn render_deploys_section(data: &ReportData, pc: &str) -> String {
    if !data.sections.deploys {
        return String::new();
    }
    let Some(d) = data.deploys.as_ref() else {
        return String::new();
    };

    let deploy_rows: String = d
        .recent
        .iter()
        .map(|dep| {
            format!(
                r#"<tr>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;font-variant-numeric:tabular-nums;white-space:nowrap">{date}</td>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px">{msg}</td>
              <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;color:#6b7280">{author}</td>
            </tr>"#,
                date = &dep.date[..10.min(dep.date.len())],
                msg = html_escape(&dep.message),
                author = html_escape(&dep.author),
            )
        })
        .collect();

    format!(
        r#"
    <div style="page-break-before:always"></div>
    <h2 style="font-size:20px;margin:32px 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">Deployment Log</h2>
    <p style="font-size:13px;color:#6b7280;margin-bottom:16px"><strong>{count}</strong> deploys during this period</p>
    <table style="width:100%;border-collapse:collapse">
      <thead><tr style="background:#f9fafb">
        <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Date</th>
        <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Commit</th>
        <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Author</th>
      </tr></thead>
      <tbody>{deploy_rows}</tbody>
    </table>"#,
        count = d.count,
    )
}

pub(super) fn render_recommendations_section(data: &ReportData, pc: &str) -> String {
    let recommendation_issues = combined_recommendation_issues(data);
    if !data.sections.recommendations || recommendation_issues.is_empty() {
        return String::new();
    }

    // Group issues by priority tier
    let fix_now: Vec<_> = recommendation_issues
        .iter()
        .filter(|i| matches!(i.severity, Severity::Critical | Severity::High))
        .collect();
    let should_fix: Vec<_> = recommendation_issues
        .iter()
        .filter(|i| i.severity == Severity::Medium)
        .collect();
    let consider: Vec<_> = recommendation_issues
        .iter()
        .filter(|i| i.severity == Severity::Low)
        .collect();

    let effort_tag = |severity: Severity| -> (&str, &str) {
        match severity {
            Severity::Critical => ("High", "#ef4444"),
            Severity::High => ("Medium", "#f97316"),
            Severity::Medium => ("Low", "#f59e0b"),
            Severity::Low => ("Low", "#6b7280"),
        }
    };

    let render_group = |title: &str, color: &str, items: &[&ReportIssue]| -> String {
        if items.is_empty() {
            return String::new();
        }
        let rows: String = items
            .iter()
            .map(|issue| {
                let (effort, effort_color) = effort_tag(issue.severity);
                format!(
                    r#"<tr>
                  <td style="padding:8px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;font-weight:500">{title}</td>
                  <td style="padding:8px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;color:#6b7280">{category}</td>
                  <td style="padding:8px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;text-align:center">
                    <span style="color:{effort_color};font-weight:600;font-size:10px;text-transform:uppercase">{effort}</span>
                  </td>
                </tr>"#,
                    title = html_escape(&issue.title),
                    category = html_escape(&issue.category),
                )
            })
            .collect();
        format!(
            r#"
            <tr><td colspan="3" style="padding:10px 12px 6px;font-size:11px;font-weight:700;color:{color};text-transform:uppercase;letter-spacing:0.05em;border-bottom:1px solid #e5e7eb">{title} ({count})</td></tr>
            {rows}"#,
            count = items.len(),
        )
    };

    let groups = [
        render_group("Fix Now", "#ef4444", &fix_now),
        render_group("Should Fix", "#f97316", &should_fix),
        render_group("Consider Fixing", "#6b7280", &consider),
    ]
    .join("");

    format!(
        r#"
    <div style="page-break-before:always"></div>
    <h2 style="font-size:20px;margin:32px 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">Recommendations</h2>
    <p style="font-size:13px;color:#6b7280;margin-bottom:16px">Prioritized action items from live Web Scans and the linked Code Scan. Items are grouped by urgency with estimated fix effort.</p>
    <table style="width:100%;border-collapse:collapse;margin-bottom:32px">
      <thead><tr style="background:#f9fafb">
        <th style="padding:8px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Issue</th>
        <th style="padding:8px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Category</th>
        <th style="padding:8px 12px;text-align:center;font-size:10px;text-transform:uppercase;color:#6b7280">Effort</th>
      </tr></thead>
      <tbody>{groups}</tbody>
    </table>"#
    )
}
