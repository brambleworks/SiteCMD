//! Self-contained HTML report renderer.

use super::{format_report_timestamp, ReportData};
use crate::checks::Severity;

mod sections;
mod supplemental_sections;
mod utils;

use sections::{render_code_scan_section, render_issues_section};
use supplemental_sections::{
    render_analytics_section, render_deploys_section, render_recommendations_section,
    render_uptime_section,
};
pub(crate) use utils::sanitize_logo_data_url;
use utils::{
    html_escape, sanitize_css_color, score_color, severity_color, svg_score_trend,
    svg_visitor_chart,
};

/// Render a self-contained, printable HTML report with inline CSS.
#[tracing::instrument(skip(data))]
pub fn render_html(data: &ReportData) -> String {
    let pc = &sanitize_css_color(&data.branding.primary_color);
    let company = html_escape(&data.branding.company_name);
    let footer_text = html_escape(&data.branding.footer_text);

    let logo_html = data
        .branding
        .logo_data_url
        .as_deref()
        .and_then(sanitize_logo_data_url)
        .map(|logo_data_url| {
            format!(
                r#"<img src="{logo_data_url}" alt="{company}" style="max-height:48px;max-width:200px;margin-bottom:12px">"#
            )
        })
        .unwrap_or_default();

    let trend_svg = svg_score_trend(&data.health.trend_points, pc, 600, 80);
    let visitor_svg = data
        .analytics
        .as_ref()
        .map(|a| svg_visitor_chart(&a.daily_visitors, pc, 600, 100))
        .unwrap_or_default();

    let scan_date_line = data
        .latest_scan_date
        .as_ref()
        .map(|d| {
            format!(
                r#"<div style="font-size:10px;color:#9ca3af;margin-top:4px">as of {}</div>"#,
                html_escape(d)
            )
        })
        .unwrap_or_default();

    let mut changes: Vec<String> = Vec::new();
    if let Some(prev) = data.health.previous_score {
        let d = data.health.current_score as i32 - prev as i32;
        if d != 0 {
            let (arrow, color, word) = if d > 0 {
                ("↑", "#10b981", "increase")
            } else {
                ("↓", "#ef4444", "decrease")
            };
            changes.push(format!(
                r#"<span style="color:{color};font-weight:600">{arrow} {}</span> Web Scan score {word}"#,
                d.abs()
            ));
        }
    }
    for cat in &data.categories {
        if let Some(prev) = cat.previous_score {
            let d = cat.score as i32 - prev as i32;
            if d != 0 {
                let (arrow, color, word) = if d > 0 {
                    ("↑", "#10b981", "increase")
                } else {
                    ("↓", "#ef4444", "decrease")
                };
                changes.push(format!(
                    r#"<span style="color:{color};font-weight:600">{arrow} {}</span> {} {word}"#,
                    d.abs(),
                    html_escape(&cat.name)
                ));
            }
        }
    }
    if let Some(code_scan) = data.code_scan.as_ref() {
        if let Some(prev) = code_scan.previous_score {
            let d = code_scan.current_score as i32 - prev as i32;
            if d != 0 {
                let (arrow, color, word) = if d > 0 {
                    ("↑", "#10b981", "increase")
                } else {
                    ("↓", "#ef4444", "decrease")
                };
                changes.push(format!(
                    r#"<span style="color:{color};font-weight:600">{arrow} {}</span> Code Scan {word}"#,
                    d.abs()
                ));
            }
        }
    }
    let score_changes = if changes.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div style="display:flex;flex-wrap:wrap;gap:8px 16px;margin-bottom:24px;padding:12px 16px;background:#f9fafb;border-radius:8px;font-size:12px">
              <span style="font-weight:600;color:#6b7280;margin-right:4px">Changes this period:</span>
              {}
            </div>"#,
            changes.join(r#" <span style="color:#d1d5db">·</span> "#)
        )
    };

    let code_scan_snapshot = data
        .code_scan
        .as_ref()
        .map(|code_scan| {
            let checked_at = format_report_timestamp(&code_scan.checked_at);
            let trend_text = match code_scan.previous_score {
                Some(previous) if code_scan.current_score > previous => {
                    format!("Linked-project score improved by {} from the previous run.", code_scan.current_score - previous)
                }
                Some(previous) if code_scan.current_score < previous => {
                    format!("Linked-project score dropped by {} from the previous run.", previous - code_scan.current_score)
                }
                Some(_) => "Linked-project score held steady from the previous run.".to_string(),
                None => "This is the first Code Scan in the selected report window.".to_string(),
            };
            let domain_trend_text = code_scan
                .domain_trend
                .as_ref()
                .map(|label| format!(" {}", label))
                .unwrap_or_default();

            format!(
                r#"
    <div style="margin-bottom:28px;padding:16px 18px;border:1px solid #dbeafe;background:#f8fbff;border-radius:12px">
      <div style="display:flex;justify-content:space-between;gap:20px;align-items:flex-start;flex-wrap:wrap">
        <div style="flex:1;min-width:280px">
          <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#2563eb;font-weight:700;margin-bottom:6px">Linked Code Scan</div>
          <p style="font-size:13px;color:#374151;line-height:1.6;margin:0">
            Latest linked-project findings produced a <strong>{score}/100</strong> diagnostic score with <strong>{critical}</strong> critical and <strong>{high}</strong> high code issues across <strong>{issue_count}</strong> findings.
            Leading domain: <strong>{top_domain}</strong>. Checked {checked_at}{framework}. {trend_text}{domain_trend_text}
          </p>
        </div>
        <div style="display:grid;grid-template-columns:repeat(4,minmax(68px,1fr));gap:8px;min-width:280px">
          <div style="background:#ffffff;border:1px solid #dbeafe;border-radius:8px;padding:10px;text-align:center">
            <div style="font-size:18px;font-weight:700;color:{score_color};font-variant-numeric:tabular-nums">{score}</div>
            <div style="font-size:9px;color:#6b7280;text-transform:uppercase;letter-spacing:0.05em">Diagnostic Score</div>
          </div>
          <div style="background:#ffffff;border:1px solid #fee2e2;border-radius:8px;padding:10px;text-align:center">
            <div style="font-size:18px;font-weight:700;color:{crit_color};font-variant-numeric:tabular-nums">{critical}</div>
            <div style="font-size:9px;color:#6b7280;text-transform:uppercase;letter-spacing:0.05em">Critical</div>
          </div>
          <div style="background:#ffffff;border:1px solid #fed7aa;border-radius:8px;padding:10px;text-align:center">
            <div style="font-size:18px;font-weight:700;color:{high_color};font-variant-numeric:tabular-nums">{high}</div>
            <div style="font-size:9px;color:#6b7280;text-transform:uppercase;letter-spacing:0.05em">High</div>
          </div>
          <div style="background:#ffffff;border:1px solid #e5e7eb;border-radius:8px;padding:10px;text-align:center">
            <div style="font-size:12px;font-weight:700;color:#111827;line-height:1.2">{top_domain}</div>
            <div style="font-size:9px;color:#6b7280;text-transform:uppercase;letter-spacing:0.05em">Leading Domain</div>
          </div>
        </div>
      </div>
    </div>"#,
                score = code_scan.current_score,
                score_color = score_color(code_scan.current_score),
                crit_color = severity_color(Severity::Critical),
                high_color = severity_color(Severity::High),
                critical = code_scan.critical_count,
                high = code_scan.high_count,
                issue_count = code_scan.issue_count,
                top_domain = html_escape(
                    &code_scan
                        .top_domain
                        .clone()
                        .unwrap_or_else(|| "Code Scan".to_string()),
                ),
                checked_at = html_escape(&checked_at),
                framework = code_scan
                    .framework
                    .as_ref()
                    .map(|framework| format!(" using {}", html_escape(framework)))
                    .unwrap_or_default(),
                trend_text = html_escape(&trend_text),
                domain_trend_text = html_escape(&domain_trend_text),
            )
        })
        .unwrap_or_default();

    let category_rows: String = data.categories.iter().map(|cat| {
        let delta = cat.previous_score.map(|prev| {
            let d = cat.score as i32 - prev as i32;
            if d > 0 { format!(r#"<span style="color:#10b981;font-size:12px"> ↑{d}</span>"#) }
            else if d < 0 { format!(r#"<span style="color:#ef4444;font-size:12px"> ↓{}</span>"#, d.abs()) }
            else { String::new() }
        }).unwrap_or_default();

        format!(r#"
        <div style="margin-bottom:12px">
          <div style="display:flex;justify-content:space-between;margin-bottom:4px">
            <span style="font-size:13px;font-weight:600">{name}</span>
            <span style="font-size:13px;font-weight:700;color:{color}">{score}/100{delta}</span>
          </div>
          <div style="background:#e5e7eb;border-radius:4px;height:8px;overflow:hidden">
            <div style="background:{color};height:100%;width:{score}%;border-radius:4px;transition:width 0.3s"></div>
          </div>
        </div>"#,
            name = html_escape(&cat.name), score = cat.score, color = score_color(cat.score), delta = delta,
        )
    }).collect();

    // Build executive summary section (conditional)
    let executive_section = if data.sections.executive_summary {
        format!(
            r#"
    <h2 style="font-size:20px;margin:0 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">Executive Summary</h2>
    <div style="display:flex;align-items:center;gap:24px;margin-bottom:16px">
      <div style="text-align:center;min-width:120px">
        <div style="font-size:56px;font-weight:800;color:{score_color};line-height:1;font-variant-numeric:tabular-nums">{score}</div>
        <div style="font-size:12px;color:#6b7280;margin-top:4px">SiteCMD Score</div>
        {scan_date_line}
      </div>
      <div style="flex:1">{trend_svg}</div>
    </div>
    {score_changes}
    <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:32px">
      <div style="background:#fef2f2;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:{crit_color};font-weight:600">Critical</div>
        <div style="font-size:22px;font-weight:700;color:{crit_color};font-variant-numeric:tabular-nums">{critical}</div>
      </div>
      <div style="background:#fff7ed;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:{high_color};font-weight:600">High</div>
        <div style="font-size:22px;font-weight:700;color:{high_color};font-variant-numeric:tabular-nums">{high}</div>
      </div>
      <div style="background:#fffbeb;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:{med_color};font-weight:600">Medium</div>
        <div style="font-size:22px;font-weight:700;color:{med_color};font-variant-numeric:tabular-nums">{medium}</div>
      </div>
      <div style="background:#f0fdf4;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#10b981;font-weight:600">Resolved</div>
        <div style="font-size:22px;font-weight:700;color:#10b981;font-variant-numeric:tabular-nums">{resolved}</div>
      </div>
    </div>
    {code_scan_snapshot}"#,
            score = data.site_score.current_score,
            score_color = score_color(data.site_score.current_score),
            crit_color = severity_color(Severity::Critical),
            high_color = severity_color(Severity::High),
            med_color = severity_color(Severity::Medium),
            scan_date_line = scan_date_line,
            score_changes = score_changes,
            critical = data.site_score.issues_critical,
            high = data.site_score.issues_high,
            medium = data.site_score.issues_medium,
            resolved = data.resolved_count,
            code_scan_snapshot = code_scan_snapshot,
        )
    } else {
        String::new()
    };

    // Build category section (conditional)
    let category_section = if data.sections.category_breakdown {
        format!(
            r#"
    <h2 style="font-size:20px;margin:0 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">Web Scan Breakdown</h2>
    <div style="margin-bottom:32px">
      {category_rows}
    </div>"#
        )
    } else {
        String::new()
    };

    let code_scan_section = render_code_scan_section(data, pc);

    let analytics_section = render_analytics_section(data, pc, &visitor_svg);
    let uptime_section = render_uptime_section(data, pc);
    let deploys_section = render_deploys_section(data, pc);
    let recommendations_section = render_recommendations_section(data, pc);

    let attribution = if data.branding.hide_attribution {
        String::new()
    } else {
        r#"<div style="font-size:9px;color:#9ca3af;margin-top:2px">Generated by SiteCMD</div>"#
            .to_string()
    };

    let client_line = data.branding.client_name.as_ref()
        .filter(|n| !n.is_empty())
        .map(|name| format!(
            r#"<div style="font-size:11px;color:#6b7280;margin-bottom:4px;font-weight:500">Confidential - prepared for {}</div>"#,
            html_escape(name)
        ))
        .unwrap_or_default();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{report_title} - {site_url}</title>
<style>
  @page {{
    margin: 20mm 15mm 25mm 15mm;
    @bottom-center {{
      content: "Page " counter(page) " of " counter(pages);
      font-size: 9px;
      color: #9ca3af;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
    }}
  }}
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; color: #111827; line-height: 1.5; }}
  @media print {{ .no-print {{ display: none !important; }} }}
</style>
</head>
<body>
  <div style="max-width:800px;margin:0 auto;padding:40px 24px">

    <!-- Cover -->
    <div style="text-align:center;padding:60px 0 40px;border-bottom:3px solid {pc};margin-bottom:32px">
      {logo_html}
      <h1 style="font-size:28px;font-weight:800;color:#111827;margin-bottom:8px">{report_title}</h1>
      <p style="font-size:16px;color:#6b7280;margin-bottom:4px">{site_url}</p>
      <p style="font-size:13px;color:#9ca3af">{period_start} - {period_end}</p>
      <p style="font-size:11px;color:#9ca3af;margin-top:8px">{company}</p>
    </div>

    {executive_section}

    {category_section}

    {code_scan_section}

    {issues_section}

    {recommendations_section}

    {analytics_section}

    {uptime_section}

    {deploys_section}

    <!-- Footer -->
    <div style="margin-top:48px;padding-top:16px;border-top:1px solid #e5e7eb;text-align:center">
      {client_line}
      <div style="font-size:10px;color:#9ca3af">{footer_text}</div>
      {attribution}
    </div>
  </div>
</body>
</html>"##,
        site_url = html_escape(&data.site_url),
        report_title = html_escape(&data.report_title),
        period_start = html_escape(&data.period_start),
        period_end = html_escape(&data.period_end),
        executive_section = executive_section,
        category_section = category_section,
        code_scan_section = code_scan_section,
        issues_section = render_issues_section(data, pc),
        recommendations_section = recommendations_section,
    )
}
